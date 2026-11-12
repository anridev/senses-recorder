import Foundation

struct AudioProcess: Decodable {
    let pid: Int32
    let bundle_id: String
}

struct RecorderStatus: Decodable {
    var state = "idle"
    var seconds: Double = 0
    var left: Float = 0
    var right: Float = 0
    var dropped_frames: UInt64 = 0
    var error: String?
    var notice: String?
    var path: String?
    var files: [String] = []
    var output_name = "System output"
    var isActive: Bool { ["recording", "paused", "starting", "stopping"].contains(state) }
}

/// Every Rust handle access, including destruction, happens on this queue.
final class EngineBridge: @unchecked Sendable {
    private let queue = DispatchQueue(label: "local.senses.engine", qos: .userInitiated)
    private var handle: OpaquePointer?
    init() { handle = senses_engine_create() }
    private func take(_ pointer: UnsafeMutablePointer<CChar>?) -> Data {
        guard let pointer else { return Data("{}".utf8) }
        defer { senses_string_free(pointer) }
        return Data(String(cString: pointer).utf8)
    }
    func command(_ object: [String: Any], completion: @escaping (String?) -> Void = { _ in }) {
        guard let data = try? JSONSerialization.data(withJSONObject: object),
              let json = String(data: data, encoding: .utf8) else { return }
        queue.async { [self] in
            let data = json.withCString { take(senses_command(handle, $0)) }
            let result = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
            let error = result?["error"] as? String
            DispatchQueue.main.async { completion(error) }
        }
    }
    func poll(includeSources: Bool, completion: @escaping (RecorderStatus?, [AudioProcess]?) -> Void) {
        queue.async { [self] in
            let status = try? JSONDecoder().decode(RecorderStatus.self, from: take(senses_snapshot(handle)))
            let sources = includeSources ? try? JSONDecoder().decode([AudioProcess].self, from: take(senses_sources())) : nil
            DispatchQueue.main.async { completion(status, sources) }
        }
    }
    func finish() {
        queue.sync {
            if let handle {
                senses_engine_destroy(handle)
                self.handle = nil
            }
        }
    }
    deinit { if let handle { senses_engine_destroy(handle) } }
}
