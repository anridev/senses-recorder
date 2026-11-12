use crate::audio::check;
use objc2_audio_toolbox::*;
use objc2_core_audio_types::*;
use objc2_core_foundation::CFURL;
use std::{
    fs::{self, OpenOptions},
    mem::size_of,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::{self, NonNull},
};

pub const WAV_LIMIT: u64 = 3_758_096_384; // 3.5 GiB, well below RIFF's 32-bit limit.

pub fn float_format(rate: f64) -> AudioStreamBasicDescription {
    AudioStreamBasicDescription {
        mSampleRate: rate,
        mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8,
        mFramesPerPacket: 1,
        mBytesPerFrame: 8,
        mChannelsPerFrame: 2,
        mBitsPerChannel: 32,
        mReserved: 0,
    }
}

pub fn file_url(path: &Path) -> Result<objc2_core_foundation::CFRetained<CFURL>, String> {
    let bytes = path.as_os_str().as_bytes();
    unsafe {
        CFURL::from_file_system_representation(None, bytes.as_ptr(), bytes.len() as isize, false)
    }
    .ok_or_else(|| "Invalid recording path.".into())
}

/// A reservation prevents two sessions from choosing the same final name.
/// The `.partial` file is retained on all unsuccessful closes.
pub struct AudioFile {
    handle: ExtAudioFileRef,
    pub final_path: PathBuf,
    pub partial_path: PathBuf,
    reservation: PathBuf,
    pub input_frames: u64,
    rate: f64,
    wav: bool,
}

impl AudioFile {
    pub fn create(
        directory: &Path,
        stem: &str,
        format: &str,
        rate: f64,
        part: u32,
    ) -> Result<Self, String> {
        if !matches!(format, "wav" | "m4a") {
            return Err("Choose WAV or M4A.".into());
        }
        if stem.is_empty() || Path::new(stem).file_name().and_then(|s| s.to_str()) != Some(stem) {
            return Err("Invalid recording filename.".into());
        }
        fs::create_dir_all(directory)
            .map_err(|e| format!("Cannot create recording folder: {e}"))?;
        let base = if part <= 1 {
            stem.to_string()
        } else {
            format!("{stem}_part{part:03}")
        };
        let (final_path, partial_path, reservation) = reserve_name(directory, &base, format)?;
        let mut file = Self {
            handle: ptr::null_mut(),
            final_path,
            partial_path,
            reservation,
            input_frames: 0,
            rate,
            wav: format == "wav",
        };
        let url = file_url(&file.partial_path)?;
        let mut destination = if file.wav {
            AudioStreamBasicDescription {
                mSampleRate: 48000.0,
                mFormatID: kAudioFormatLinearPCM,
                mFormatFlags: kAudioFormatFlagIsSignedInteger | kAudioFormatFlagIsPacked,
                mBytesPerPacket: 6,
                mFramesPerPacket: 1,
                mBytesPerFrame: 6,
                mChannelsPerFrame: 2,
                mBitsPerChannel: 24,
                mReserved: 0,
            }
        } else {
            AudioStreamBasicDescription {
                mSampleRate: 48000.0,
                mFormatID: kAudioFormatMPEG4AAC,
                mChannelsPerFrame: 2,
                mFormatFlags: 0,
                mBytesPerPacket: 0,
                mFramesPerPacket: 0,
                mBytesPerFrame: 0,
                mBitsPerChannel: 0,
                mReserved: 0,
            }
        };
        unsafe {
            if !file.wav {
                let mut size = size_of::<AudioStreamBasicDescription>() as u32;
                check(
                    AudioFormatGetProperty(
                        kAudioFormatProperty_FormatInfo,
                        0,
                        ptr::null(),
                        &mut size,
                        (&mut destination as *mut AudioStreamBasicDescription).cast(),
                    ),
                    "Configure AAC",
                )?;
            }
            check(
                ExtAudioFileCreateWithURL(
                    &url,
                    if file.wav {
                        kAudioFileWAVEType
                    } else {
                        kAudioFileM4AType
                    },
                    NonNull::from(&mut destination),
                    ptr::null(),
                    0,
                    NonNull::from(&mut file.handle),
                ),
                "Create recording file",
            )?;
            let mut input = float_format(rate);
            check(
                ExtAudioFileSetProperty(
                    file.handle,
                    kExtAudioFileProperty_ClientDataFormat,
                    size_of::<AudioStreamBasicDescription>() as u32,
                    NonNull::from(&mut input).cast(),
                ),
                "Set capture format",
            )?;
            if !file.wav {
                let mut converter: AudioConverterRef = ptr::null_mut();
                let mut size = size_of::<AudioConverterRef>() as u32;
                check(
                    ExtAudioFileGetProperty(
                        file.handle,
                        kExtAudioFileProperty_AudioConverter,
                        NonNull::from(&mut size),
                        NonNull::from(&mut converter).cast(),
                    ),
                    "Access AAC encoder",
                )?;
                let mut bitrate = 256_000u32;
                check(
                    AudioConverterSetProperty(
                        converter,
                        kAudioConverterEncodeBitRate,
                        4,
                        NonNull::from(&mut bitrate).cast(),
                    ),
                    "Set AAC bitrate",
                )?;
                // Notify ExtAudioFile that the owned converter's configuration changed.
                let mut configuration: *const std::ffi::c_void = ptr::null();
                check(
                    ExtAudioFileSetProperty(
                        file.handle,
                        kExtAudioFileProperty_ConverterConfig,
                        size_of::<*const std::ffi::c_void>() as u32,
                        NonNull::from(&mut configuration).cast(),
                    ),
                    "Apply AAC settings",
                )?;
            }
        }
        Ok(file)
    }
    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if !samples.len().is_multiple_of(2) {
            return Err("Stereo data must contain complete frames.".into());
        }
        let frames = samples.len() / 2;
        let mut buffer = AudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [AudioBuffer {
                mNumberChannels: 2,
                mDataByteSize: std::mem::size_of_val(samples) as u32,
                mData: samples.as_ptr() as *mut _,
            }],
        };
        unsafe {
            check(
                ExtAudioFileWrite(self.handle, frames as u32, NonNull::from(&mut buffer)),
                "Write audio (check free disk space)",
            )?;
        }
        self.input_frames += frames as u64;
        Ok(())
    }
    pub fn needs_rotation(&self, next_frames: usize, limit: u64) -> bool {
        self.wav
            && ((self.input_frames + next_frames as u64) as f64 * 48000.0 / self.rate * 6.0
                + 65536.0)
                >= limit as f64
            && self.input_frames > 0
    }
    pub fn finish(mut self) -> Result<PathBuf, String> {
        let handle = std::mem::replace(&mut self.handle, ptr::null_mut());
        unsafe {
            check(
                ExtAudioFileDispose(handle),
                "Finalize recording (unfinished file retained)",
            )?;
        }
        // macOS exclusive rename is atomic, does not overwrite, and also works
        // on external volumes that do not support hard links.
        let from = std::ffi::CString::new(self.partial_path.as_os_str().as_bytes())
            .map_err(|e| e.to_string())?;
        let to = std::ffi::CString::new(self.final_path.as_os_str().as_bytes())
            .map_err(|e| e.to_string())?;
        if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } != 0 {
            return Err(format!(
                "Cannot finalize filename; audio remains at {}: {}",
                self.partial_path.display(),
                std::io::Error::last_os_error()
            ));
        }
        Ok(self.final_path.clone())
    }
}
impl Drop for AudioFile {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                ExtAudioFileDispose(self.handle);
            }
        }
        let _ = fs::remove_file(&self.reservation);
    }
}

fn reserve_name(
    directory: &Path,
    base: &str,
    extension: &str,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    for suffix in 0..10_000 {
        let stem = if suffix == 0 {
            base.to_string()
        } else {
            format!("{base}_{suffix}")
        };
        let final_path = directory.join(format!("{stem}.{extension}"));
        let partial = directory.join(format!("{stem}.{extension}.partial"));
        let reservation = directory.join(format!(".{stem}.{extension}.reserve"));
        if final_path.exists() || partial.exists() {
            continue;
        }
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&reservation)
        {
            Ok(_) => return Ok((final_path, partial, reservation)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Cannot write to the recording folder: {e}")),
        }
    }
    Err("Could not choose an unused recording filename.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn temp_dir(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("senses-{label}-{}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn collision_and_partial_files_are_preserved() {
        let dir = temp_dir("collisions");
        let a = AudioFile::create(&dir, "test", "wav", 48000.0, 1).unwrap();
        let b = AudioFile::create(&dir, "test", "wav", 48000.0, 1).unwrap();
        assert_ne!(a.final_path, b.final_path);
        let partial = a.partial_path.clone();
        drop(a);
        drop(b);
        assert!(partial.exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_bad_destinations_and_formats() {
        let dir = temp_dir("validation");
        assert!(AudioFile::create(&dir, "../escape", "wav", 48000.0, 1).is_err());
        assert!(AudioFile::create(&dir, "test", "mp3", 48000.0, 1).is_err());
        let blocker = dir.join("regular-file");
        fs::write(&blocker, "test").unwrap();
        assert!(AudioFile::create(&blocker, "test", "wav", 48000.0, 1).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    fn decode(path: &Path) -> (AudioStreamBasicDescription, Vec<f32>) {
        unsafe {
            let mut handle = ptr::null_mut();
            let url = file_url(path).unwrap();
            check(
                ExtAudioFileOpenURL(&url, NonNull::from(&mut handle)),
                "Open test file",
            )
            .unwrap();
            let mut format = float_format(0.0);
            let mut size = size_of::<AudioStreamBasicDescription>() as u32;
            check(
                ExtAudioFileGetProperty(
                    handle,
                    kExtAudioFileProperty_FileDataFormat,
                    NonNull::from(&mut size),
                    NonNull::from(&mut format).cast(),
                ),
                "Read format",
            )
            .unwrap();
            let mut client = float_format(48000.0);
            check(
                ExtAudioFileSetProperty(
                    handle,
                    kExtAudioFileProperty_ClientDataFormat,
                    size,
                    NonNull::from(&mut client).cast(),
                ),
                "Decode float samples",
            )
            .unwrap();
            let mut all = Vec::new();
            loop {
                let mut samples = vec![0.0f32; 8192];
                let mut buffer = AudioBufferList {
                    mNumberBuffers: 1,
                    mBuffers: [AudioBuffer {
                        mNumberChannels: 2,
                        mDataByteSize: (samples.len() * 4) as u32,
                        mData: samples.as_mut_ptr().cast(),
                    }],
                };
                let mut frames = 4096u32;
                check(
                    ExtAudioFileRead(
                        handle,
                        NonNull::from(&mut frames),
                        NonNull::from(&mut buffer),
                    ),
                    "Decode",
                )
                .unwrap();
                if frames == 0 {
                    break;
                }
                all.extend_from_slice(&samples[..frames as usize * 2]);
            }
            check(ExtAudioFileDispose(handle), "Close test file").unwrap();
            (format, all)
        }
    }
    #[test]
    fn wav_and_aac_roundtrip_resample_and_preserve_stereo() {
        let dir = temp_dir("roundtrip");
        for extension in ["wav", "m4a"] {
            let mut file = AudioFile::create(&dir, "tones", extension, 44100.0, 1).unwrap();
            for start in (0..88200).step_by(512) {
                let end = (start + 512).min(88200);
                let samples: Vec<f32> = (start..end)
                    .flat_map(|i| {
                        [440.0f32, 880.0]
                            .map(|f| (i as f32 / 44100.0 * std::f32::consts::TAU * f).sin() * 0.3)
                    })
                    .collect();
                file.write(&samples).unwrap();
            }
            let path = file.finish().unwrap();
            let (format, samples) = decode(&path);
            assert_eq!(format.mSampleRate, 48000.0);
            assert_eq!(format.mChannelsPerFrame, 2);
            assert!(
                (samples.len() as i64 / 2 - 96000).abs() <= 2048,
                "{} frames",
                samples.len() / 2
            );
            let tone_power = |channel: usize, frequency: f64| -> f64 {
                let (mut real, mut imaginary) = (0.0, 0.0);
                for (i, frame) in samples
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .skip(4096)
                    .take(48000)
                    .enumerate()
                {
                    let phase = i as f64 * std::f64::consts::TAU * frequency / 48000.0;
                    real += frame[channel] as f64 * phase.cos();
                    imaginary += frame[channel] as f64 * phase.sin();
                }
                real.hypot(imaginary) / 48000.0
            };
            assert!(tone_power(0, 440.0) > 0.1);
            assert!(tone_power(1, 880.0) > 0.1);
            assert!(tone_power(0, 880.0) < 0.005);
            assert!(tone_power(1, 440.0) < 0.005);
            if extension == "wav" {
                assert_eq!(format.mBitsPerChannel, 24);
            }
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wav_rotation_preserves_frames_across_parts() {
        let dir = temp_dir("rotation");
        let mut file = AudioFile::create(&dir, "split", "wav", 48000.0, 1).unwrap();
        let mut paths = vec![];
        let mut part = 1;
        for _ in 0..10 {
            if file.needs_rotation(1024, 90000) {
                paths.push(file.finish().unwrap());
                part += 1;
                file = AudioFile::create(&dir, "split", "wav", 48000.0, part).unwrap();
            }
            file.write(&vec![0.2; 2048]).unwrap();
        }
        paths.push(file.finish().unwrap());
        assert!(paths.len() > 1);
        assert_eq!(
            paths.iter().map(|p| decode(p).1.len() / 2).sum::<usize>(),
            10240
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
