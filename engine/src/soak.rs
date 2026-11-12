//! Opt-in, silent integration test. Feeds both production writer queues in real
//! time without opening a microphone, capturing an app, or playing any sound.
use crate::{
    audio::check,
    file::{file_url, float_format},
    queue::MAX_FRAMES,
    recording::{Config, Selection, Shared, writer},
};
use objc2_audio_toolbox::*;
use objc2_core_audio_types::{AudioBuffer, AudioBufferList};
use std::{
    fs,
    mem::size_of,
    path::Path,
    ptr::{self, NonNull},
    sync::{Arc, atomic::Ordering, mpsc},
    thread,
    time::{Duration, Instant},
};

fn inspect(path: &Path) -> u64 {
    unsafe {
        let mut handle = ptr::null_mut();
        check(
            ExtAudioFileOpenURL(&file_url(path).unwrap(), NonNull::from(&mut handle)),
            "Open soak output",
        )
        .unwrap();
        let mut frames = 0i64;
        let mut size = size_of::<i64>() as u32;
        check(
            ExtAudioFileGetProperty(
                handle,
                kExtAudioFileProperty_FileLengthFrames,
                NonNull::from(&mut size),
                NonNull::from(&mut frames).cast(),
            ),
            "Read output frame count",
        )
        .unwrap();
        let mut format = float_format(0.0);
        size = size_of_val(&format) as u32;
        check(
            ExtAudioFileGetProperty(
                handle,
                kExtAudioFileProperty_FileDataFormat,
                NonNull::from(&mut size),
                NonNull::from(&mut format).cast(),
            ),
            "Read output format",
        )
        .unwrap();
        assert_eq!(format.mSampleRate, 48000.0);
        assert_eq!(format.mChannelsPerFrame, 2);
        let mut client = float_format(48000.0);
        check(
            ExtAudioFileSetProperty(
                handle,
                kExtAudioFileProperty_ClientDataFormat,
                size_of_val(&client) as u32,
                NonNull::from(&mut client).cast(),
            ),
            "Configure soak decoder",
        )
        .unwrap();
        let mut samples = vec![0.0f32; MAX_FRAMES * 2];
        let mut decoded = 0u64;
        let mut energy = 0.0f64;
        let mut stereo_error = 0.0f64;
        loop {
            let mut buffer = AudioBufferList {
                mNumberBuffers: 1,
                mBuffers: [AudioBuffer {
                    mNumberChannels: 2,
                    mDataByteSize: (samples.len() * size_of::<f32>()) as u32,
                    mData: samples.as_mut_ptr().cast(),
                }],
            };
            let mut read = MAX_FRAMES as u32;
            check(
                ExtAudioFileRead(handle, NonNull::from(&mut read), NonNull::from(&mut buffer)),
                "Decode complete soak output",
            )
            .unwrap();
            if read == 0 {
                break;
            }
            for frame in samples[..read as usize * 2].as_chunks::<2>().0 {
                let [left, right] = frame.map(f64::from);
                assert!(left.is_finite() && right.is_finite());
                energy += left * left + right * right;
                stereo_error += (left + right).powi(2);
            }
            decoded += read as u64;
        }
        assert_eq!(
            decoded, frames as u64,
            "Decoded length must match file metadata"
        );
        let rms = (energy / (decoded * 2) as f64).sqrt();
        assert!(
            (0.06..0.08).contains(&rms),
            "Unexpected fixture amplitude: {rms}"
        );
        assert!((stereo_error / decoded as f64).sqrt() < 0.005);
        check(ExtAudioFileDispose(handle), "Close soak output").unwrap();
        decoded
    }
}

fn resident_high_water() -> i64 {
    unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage.ru_maxrss
    }
}

#[test]
#[ignore = "Four-hour silent real-time integration test; use scripts/soak.sh"]
fn four_hour_recording() {
    let seconds: u64 = std::env::var("SENSES_SOAK_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(14400);
    let realtime = std::env::var("SENSES_SOAK_FAST").as_deref() != Ok("1");
    let directory = std::env::temp_dir().join(format!("senses-soak-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let mut workers = Vec::new();
    let mut states = Vec::new();
    for format in ["wav", "m4a"] {
        let config = Config {
            directory: directory.clone(),
            stem: format!("soak-{format}"),
            format: format.into(),
            selection: Selection {
                all: false,
                pids: vec![],
                bundles: vec![],
            },
        };
        let shared = Arc::new(Shared::new(false));
        let state = shared.clone();
        let (send, receive) = mpsc::sync_channel(1);
        workers.push(thread::spawn(move || {
            objc2::rc::autoreleasepool(|_| writer(config, 48000.0, 1, state, send))
        }));
        receive.recv().unwrap().unwrap();
        states.push(shared);
    }
    let block: Vec<f32> = (0..MAX_FRAMES)
        .flat_map(|i| {
            let left = (i as f32 * std::f32::consts::TAU * 375.0 / 48000.0).sin() * 0.1;
            [left, -left]
        })
        .collect();
    let total = seconds * 48000;
    let start = Instant::now();
    let mut sent = 0u64;
    let mut memory_samples = Vec::new();
    let mut next_report = 60u64 * 48000;
    while sent < total {
        let frames = MAX_FRAMES.min((total - sent) as usize);
        for shared in &states {
            loop {
                let pushed = unsafe {
                    shared
                        .queue
                        .push(frames, |s| s.copy_from_slice(&block[..frames * 2]))
                };
                if pushed {
                    break;
                }
                assert!(
                    !realtime,
                    "writer queue overflowed during a real-time stream"
                );
                assert!(
                    !shared.stop.load(Ordering::Acquire),
                    "writer stopped unexpectedly"
                );
                thread::sleep(Duration::from_micros(100));
            }
        }
        sent += frames as u64;
        if realtime {
            let deadline = start + Duration::from_secs_f64(sent as f64 / 48000.0);
            if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
                thread::sleep(remaining);
            }
        }
        if sent >= next_report {
            let rss = resident_high_water();
            memory_samples.push(
                serde_json::json!({"audio_seconds": sent as f64 / 48000.0, "peak_rss_bytes": rss}),
            );
            eprintln!(
                "soak: {:.0}/{seconds}s audio; {:.0}s elapsed; peak RSS {:.1} MiB",
                sent as f64 / 48000.0,
                start.elapsed().as_secs_f64(),
                rss as f64 / 1048576.0
            );
            next_report += 60 * 48000;
        }
    }
    for state in &states {
        state.stop.store(true, Ordering::Release);
    }
    for worker in workers {
        worker.join().unwrap();
    }
    let mut outputs = Vec::new();
    for (format, shared) in ["wav", "m4a"].into_iter().zip(states) {
        assert_eq!(shared.frames.load(Ordering::Relaxed), total);
        assert_eq!(shared.dropped.load(Ordering::Relaxed), 0);
        let state = shared.writer.lock().unwrap();
        assert!(state.error.is_none(), "{:?}", state.error);
        let actual: u64 = state.files.iter().map(|p| inspect(Path::new(p))).sum();
        assert!(
            actual.abs_diff(total) <= 2048,
            "{format}: {actual} != {total}"
        );
        if format == "wav" && seconds >= 14400 {
            assert!(
                state.files.len() >= 2,
                "WAV must rotate before RIFF overflow"
            );
        }
        outputs.push(serde_json::json!({"format":format,"parts":state.files.len(),"frames":actual,"dropped_frames":0}));
    }
    let report = serde_json::json!({"audio_seconds":seconds,"realtime":realtime,"elapsed_seconds":start.elapsed().as_secs_f64(),
        "outputs":outputs,"memory_samples":memory_samples,"result":"passed"});
    if let Ok(path) = std::env::var("SENSES_SOAK_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    fs::remove_dir_all(directory).unwrap();
    eprintln!("{}", serde_json::to_string_pretty(&report).unwrap());
}
