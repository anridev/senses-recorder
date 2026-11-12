import AudioToolbox
import Foundation

/// A bounded-memory full decode check for the silent endurance fixture.
/// Usage: verify-audio file.wav file_part002.wav file.m4a
@main
enum VerifyAudio {
    static func check(_ status: OSStatus, _ operation: String) {
        guard status == noErr else { fatalError("\(operation): OSStatus \(status)") }
    }

    static func main() {
        guard CommandLine.arguments.count > 1 else { fatalError("Provide finalized soak audio paths") }
        var results: [[String: Any]] = []
        for path in CommandLine.arguments.dropFirst() {
            var file: ExtAudioFileRef?
            check(ExtAudioFileOpenURL(URL(fileURLWithPath: path) as CFURL, &file), "Open \(path)")
            guard let file else { fatalError("No audio file") }
            var stored = AudioStreamBasicDescription()
            var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.size)
            check(ExtAudioFileGetProperty(file, kExtAudioFileProperty_FileDataFormat, &size, &stored), "Read format")
            precondition(stored.mSampleRate == 48000 && stored.mChannelsPerFrame == 2, "Unexpected format")
            if path.hasSuffix(".wav") { precondition(stored.mBitsPerChannel == 24, "Expected PCM24") }
            var format = AudioStreamBasicDescription(mSampleRate: 48000,
                mFormatID: kAudioFormatLinearPCM, mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
                mBytesPerPacket: 8, mFramesPerPacket: 1, mBytesPerFrame: 8, mChannelsPerFrame: 2,
                mBitsPerChannel: 32, mReserved: 0)
            check(ExtAudioFileSetProperty(file, kExtAudioFileProperty_ClientDataFormat, size, &format), "Set decoder format")
            var samples = [Float](repeating: 0, count: 8192 * 2)
            var total: UInt64 = 0
            var energy: Double = 0
            var stereoSumEnergy: Double = 0
            var nonfinite = 0
            while true {
                let read: UInt32 = samples.withUnsafeMutableBytes { storage in
                    var buffers = AudioBufferList(mNumberBuffers: 1, mBuffers: AudioBuffer(
                        mNumberChannels: 2, mDataByteSize: UInt32(storage.count), mData: storage.baseAddress))
                    var frames: UInt32 = 8192
                    check(ExtAudioFileRead(file, &frames, &buffers), "Decode \(path)")
                    return frames
                }
                if read == 0 { break }
                for frame in 0..<Int(read) {
                    let left = Double(samples[frame * 2])
                    let right = Double(samples[frame * 2 + 1])
                    if !left.isFinite || !right.isFinite { nonfinite += 1 }
                    energy += left * left + right * right
                    stereoSumEnergy += (left + right) * (left + right)
                }
                total += UInt64(read)
            }
            check(ExtAudioFileDispose(file), "Close decoder")
            let rms = sqrt(energy / Double(max(1, total * 2)))
            let stereoError = sqrt(stereoSumEnergy / Double(max(1, total)))
            precondition(nonfinite == 0, "Nonfinite audio samples")
            precondition(rms > 0.06 && rms < 0.08, "Unexpected fixture amplitude: \(rms)")
            precondition(stereoError < 0.005, "Stereo fixture was not preserved: \(stereoError)")
            results.append(["file": URL(fileURLWithPath: path).lastPathComponent,
                "decoded_frames": total, "sample_rate": 48000, "channels": 2,
                "rms": rms, "stereo_error_rms": stereoError, "nonfinite_frames": nonfinite])
        }
        let data = try! JSONSerialization.data(withJSONObject: results, options: [.prettyPrinted, .sortedKeys])
        print(String(decoding: data, as: UTF8.self))
    }
}
