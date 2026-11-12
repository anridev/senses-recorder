# Validation

Test host: Apple silicon, macOS 26.3; Xcode 26.6; Rust 1.99.1.

## Automated

- Short suite: 11 passed; the hardware format probe and four-hour endurance test are opt-in.
- Swift source membership: 12 checks cover helper ownership, app isolation, combined selection, and stale-process replacement after relaunch.
- Clippy: all targets pass with warnings treated as errors.
- Release build and strict code-signature verification pass.
- Synthetic WAV and AAC round trips: stereo separation and 44.1→48 kHz resampling verified by frequency analysis; WAV is 24-bit PCM.
- WAV rotation with a reduced test threshold: decoded parts contain exactly the original frame count.
- Accelerated four-hour engine test: passed in 73.7 seconds, with exactly 691,200,000 frames in each format. WAV rotated into two parts; M4A stayed in one. Peak resident memory reached 27.4 MiB. Raw report: `build/soak-report.json`.
- Four-hour real-time engine test: passed in 14,400.1 seconds. Each format contains exactly 691,200,000 frames at 48 kHz stereo, with zero dropped frames. Peak resident memory was 27.8 MiB (16.9 MiB at the first minute). WAV produced two parts; M4A produced one. Raw report: `build/soak-realtime-report.json`. This tests the production queue and writer path using silent synthetic input, not four hours of hardware capture.
- Independent full-file decoding: all four hours of M4A and both WAV parts decoded successfully using bounded memory, with exact frame counts, finite samples, and the expected stereo signal. The first WAV part is 3,758,018,560 bytes, below the 3.5 GiB limit. Raw report: `build/soak-full-decode-report.json`. Temporary endurance audio was removed after verification.

## Live app checks

- Native app launches; window, source selection, format/folder controls, level meters, and Recent recordings are visible and usable.
- Chrome capture produces a playable stereo M4A; pause holds the timer steady, resume continues, and stop finalizes the file.
- Repeated recording sessions produce valid M4A and 24-bit WAV files.
- All-system capture produces a valid WAV. Test tone analysis exposed a Bluetooth clock mismatch (44.1 kHz device clock versus 48 kHz tap metadata); the engine now uses the aggregate device clock. The silent hardware probe confirms the corrected rate.
- After that fix, another live capture saved 49.006 seconds of 48 kHz, 24-bit stereo WAV, with the recording timer advancing at normal speed.
- The separate-process test tone appears as a sustained signal in all-system capture and is absent as a sustained signal in Chrome-only capture. These tests used the same physical output; no source playback was rerouted.
- The cleaned app picker and saved destination/format settings were checked in the final UI. The app was restored to M4A and `~/Music/Senses` after testing.
- Normal output remains the selected WH-1000XM5 headphones. No default-output switching or captured-audio replay is performed by the recorder.
- The user made recordings in the completed app and confirmed correct sound and playback speed.

## Manual checks still requiring hardware or user interaction

- Permission denial followed by granting access and retrying (successful permission/capture is verified; deliberately revoking access was not performed).
- Switching between speakers, USB audio, and Bluetooth while recording, including unplugging hardware.
- Explicit system sleep, audio-service restart, and a genuinely full destination volume.
- Browser helper restart and automatic app reattachment across every browser family.
- Two independently playing apps captured together in one recording (selection merging is covered by unit checks).
- Closing the window and quitting during an active recording (idle quit/relaunch is verified).
- Subjective check for unchanged live speaker/headphone playback while recording (recorded sound and speed are confirmed by the user).

Automated overflow and invalid-destination tests exercise failure paths without filling the user's disk or restarting system audio services.

## Logo update — November 10, 2026

- Version 0.1.1 adds the shared vector S-waveform mark to the app icon, sidebar, header, and idle menu-bar icon. The recording indicator remains visible during capture.
- Generated icons were visually checked at small sizes and at 256 pixels. SVG and transparent PNG exports are in `design/`.
- Release build and strict signature verification passed. The installed `/Applications/Senses.app` icon matches the generated ICNS, and the bundle reports version 0.1.1.

## Recorder redesign — November 12, 2026

- The macOS release build passed strict code-signature verification. The short suite passed all 11 Rust tests, Clippy, and all 12 Swift source-membership checks.
- A separate local preview rendered the production SwiftUI views with sample selections. Safari appeared by name and icon beside the transport and in Recording Options. With Safari and Music selected, both names and icons appeared in the options at regular and narrow window widths.
- The dark appearance and All System Audio state were visually checked. The README image shows the idle recorder with example apps. This pass used synthetic audio for codec verification.
