use crate::{
    audio::{self, Capture},
    file::{AudioFile, WAV_LIMIT},
    queue::Queue,
};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize)]
pub struct Selection {
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub pids: Vec<i32>,
    #[serde(default)]
    pub bundles: Vec<String>,
}
#[derive(Clone, Deserialize)]
pub struct Config {
    pub directory: PathBuf,
    pub stem: String,
    pub format: String,
    #[serde(flatten)]
    pub selection: Selection,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Start {
        #[serde(flatten)]
        config: Config,
    },
    Pause,
    Resume,
    Stop,
    Interrupt {
        reason: String,
    },
    UpdateSources {
        #[serde(flatten)]
        selection: Selection,
    },
}

#[derive(Default)]
pub struct WriterState {
    pub files: Vec<String>,
    pub path: Option<String>,
    pub error: Option<String>,
    pub part: u32,
}

pub struct Shared {
    pub queue: Queue,
    pub paused: AtomicBool,
    pub stop: AtomicBool,
    pub fault: AtomicU32,
    pub dropped: AtomicU64,
    pub callbacks: AtomicU64,
    pub frames: AtomicU64,
    pub left: AtomicU32,
    pub right: AtomicU32,
    pub nonzero: AtomicBool,
    pub writer: Mutex<WriterState>,
}
impl Shared {
    pub fn new(paused: bool) -> Self {
        Self {
            queue: Queue::new(),
            paused: AtomicBool::new(paused),
            stop: AtomicBool::new(false),
            fault: AtomicU32::new(0),
            dropped: AtomicU64::new(0),
            callbacks: AtomicU64::new(0),
            frames: AtomicU64::new(0),
            left: AtomicU32::new(0),
            right: AtomicU32::new(0),
            nonzero: AtomicBool::new(false),
            writer: Mutex::new(WriterState::default()),
        }
    }
    pub fn fail(&self, code: u32) {
        self.fault.store(code, Ordering::Release);
        self.stop.store(true, Ordering::Release);
    }
}

pub(crate) fn writer(
    config: Config,
    rate: f64,
    mut part: u32,
    shared: Arc<Shared>,
    ready: std::sync::mpsc::SyncSender<Result<(), String>>,
) {
    let result = AudioFile::create(&config.directory, &config.stem, &config.format, rate, part);
    let file = match result {
        Ok(file) => file,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    {
        let mut state = shared.writer.lock().unwrap();
        state.path = Some(file.partial_path.display().to_string());
        state.part = part;
    }
    if ready.send(Ok(())).is_err() {
        return;
    }
    let mut error = None;
    let mut current = Some(file);
    loop {
        let mut failed = false;
        let popped = unsafe {
            shared.queue.pop(|block| {
                if current
                    .as_ref()
                    .unwrap()
                    .needs_rotation(block.frames, WAV_LIMIT)
                {
                    match current.take().unwrap().finish() {
                        Ok(path) => shared
                            .writer
                            .lock()
                            .unwrap()
                            .files
                            .push(path.display().to_string()),
                        Err(e) => {
                            error = Some(e);
                            failed = true;
                            return;
                        }
                    }
                    part += 1;
                    match AudioFile::create(
                        &config.directory,
                        &config.stem,
                        &config.format,
                        rate,
                        part,
                    ) {
                        Ok(next) => {
                            let mut state = shared.writer.lock().unwrap();
                            state.path = Some(next.partial_path.display().to_string());
                            state.part = part;
                            current = Some(next);
                        }
                        Err(e) => {
                            error = Some(e);
                            failed = true;
                            return;
                        }
                    }
                }
                if let Err(e) = current
                    .as_mut()
                    .unwrap()
                    .write(&block.samples[..block.frames * 2])
                {
                    error = Some(e);
                    failed = true;
                    return;
                }
                let mut peaks = [0.0f32; 2];
                for frame in block.samples[..block.frames * 2].as_chunks::<2>().0 {
                    peaks[0] = peaks[0].max(frame[0].abs());
                    peaks[1] = peaks[1].max(frame[1].abs());
                }
                if peaks[0] > 0.00001 || peaks[1] > 0.00001 {
                    shared.nonzero.store(true, Ordering::Relaxed);
                }
                for (atomic, peak) in [(&shared.left, peaks[0]), (&shared.right, peaks[1])] {
                    let old = f32::from_bits(atomic.load(Ordering::Relaxed));
                    atomic.store(peak.max(old * 0.85).to_bits(), Ordering::Relaxed);
                }
                shared
                    .frames
                    .fetch_add(block.frames as u64, Ordering::Relaxed);
            })
        };
        if failed {
            shared.stop.store(true, Ordering::Release);
            break;
        }
        if !popped {
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
    }
    if let Some(last) = current.take() {
        match last.finish() {
            Ok(path) => {
                let mut state = shared.writer.lock().unwrap();
                state.files.push(path.display().to_string());
                state.path = None;
            }
            Err(e) => {
                if error.is_none() {
                    error = Some(e);
                }
            }
        }
    }
    shared.writer.lock().unwrap().error = error;
    shared.stop.store(true, Ordering::Release);
}

struct Session {
    capture: Option<Capture>,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
    rate: f64,
    started: Instant,
    last_callback: Instant,
    callbacks: u64,
}
impl Session {
    fn start(config: &Config, part: u32, paused: bool) -> Result<Self, String> {
        let shared = Arc::new(Shared::new(paused));
        let capture = Capture::new(
            config.selection.all,
            &config.selection.pids,
            &config.selection.bundles,
            shared.clone(),
        )?;
        let rate = capture.rate;
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        let state = shared.clone();
        let options = config.clone();
        let worker = thread::Builder::new()
            .name("senses-file-writer".into())
            .spawn(move || {
                // AudioToolbox and Foundation may autorelease objects on this thread.
                objc2::rc::autoreleasepool(|_| writer(options, rate, part, state, send));
            })
            .map_err(|e| format!("Cannot start file writer: {e}"))?;
        let mut session = Self {
            capture: Some(capture),
            shared,
            worker: Some(worker),
            rate,
            started: Instant::now(),
            last_callback: Instant::now(),
            callbacks: 0,
        };
        receive
            .recv()
            .map_err(|_| "Audio file writer stopped unexpectedly.".to_string())??;
        session.capture.as_mut().unwrap().start()?;
        Ok(session)
    }
    fn seconds(&self) -> f64 {
        self.shared.frames.load(Ordering::Relaxed) as f64 / self.rate
    }
    fn stop(&mut self) {
        // Stop and unregister callbacks before signaling the consumer to drain.
        self.capture.take();
        self.shared.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            self.shared
                .writer
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .error =
                Some("The file writer failed. Any unfinished file has been retained.".into());
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Default, Clone, Copy, PartialEq, Debug)]
enum Mode {
    #[default]
    Idle,
    Recording,
    Paused,
}
impl Mode {
    fn pause(self) -> Result<Self, String> {
        if self == Self::Recording {
            Ok(Self::Paused)
        } else {
            Err("Only an active recording can be paused.".into())
        }
    }
    fn resume(self) -> Result<Self, String> {
        if self == Self::Paused {
            Ok(Self::Recording)
        } else {
            Err("Only a paused recording can be resumed.".into())
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Paused => "paused",
        }
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    state: &'static str,
    seconds: f64,
    left: f32,
    right: f32,
    dropped_frames: u64,
    error: Option<String>,
    notice: Option<String>,
    path: Option<String>,
    files: Vec<String>,
    output_name: String,
}

#[derive(Default)]
pub struct Engine {
    session: Option<Session>,
    config: Option<Config>,
    mode: Mode,
    seconds: f64,
    dropped: u64,
    files: Vec<String>,
    error: Option<String>,
    notice: Option<String>,
    next_part: u32,
}
impl Engine {
    pub fn command(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Start { config } => {
                if self.session.is_some() {
                    return Err("A recording is already running.".into());
                }
                if !config.selection.all
                    && config.selection.bundles.is_empty()
                    && config.selection.pids.is_empty()
                {
                    return Err("Select at least one app.".into());
                }
                self.error = None;
                self.notice = None;
                self.seconds = 0.0;
                self.dropped = 0;
                self.files.clear();
                self.next_part = 1;
                let session = Session::start(&config, 1, false)?;
                self.session = Some(session);
                self.config = Some(config);
                self.mode = Mode::Recording;
            }
            Command::Pause => {
                self.mode = self.mode.pause()?;
                self.session
                    .as_ref()
                    .unwrap()
                    .shared
                    .paused
                    .store(true, Ordering::Release);
            }
            Command::Resume => {
                self.mode = self.mode.resume()?;
                self.session
                    .as_ref()
                    .unwrap()
                    .shared
                    .paused
                    .store(false, Ordering::Release);
            }
            Command::Stop => {
                self.stop();
            }
            Command::Interrupt { reason } => {
                self.stop();
                self.notice = Some(reason);
            }
            Command::UpdateSources { selection } => {
                if let (Some(session), Some(config)) = (&mut self.session, &mut self.config) {
                    // Capture mode is fixed until Stop. Only process membership can change.
                    if selection.all != config.selection.all {
                        return Err("Stop recording before changing capture mode.".into());
                    }
                    session.capture.as_mut().unwrap().set_membership(
                        selection.all,
                        &selection.pids,
                        &selection.bundles,
                    )?;
                    config.selection = selection;
                }
            }
        }
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.stop();
            self.seconds += session.seconds();
            self.dropped += session.shared.dropped.load(Ordering::Relaxed);
            let state = session
                .shared
                .writer
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            self.files.extend(state.files.clone());
            self.next_part = state.part + 1;
            if let Some(error) = &state.error {
                self.error = Some(error.clone());
            }
        }
        self.mode = Mode::Idle;
    }
    fn maintain(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        let failure = match session.shared.fault.load(Ordering::Acquire) {
            1 => Some("Recording stopped because disk encoding could not keep up. Available audio was saved.".into()),
            2 => Some("Recording stopped because the audio device changed to an unsupported buffer format.".into()),
            _ => None,
        };
        if let Some(error) = failure {
            self.stop();
            if self.error.is_none() {
                self.error = Some(error);
            }
            return;
        }
        if session.shared.stop.load(Ordering::Acquire) {
            self.stop();
            return;
        }
        let output = match audio::default_output() {
            Ok(id) => id,
            Err(error) => {
                self.stop();
                self.error = Some(error);
                return;
            }
        };
        if session.capture.as_ref().unwrap().output != output
            || session.capture.as_ref().unwrap().format_changed()
        {
            let paused = self.mode == Mode::Paused;
            self.stop();
            if self.error.is_some() {
                return;
            }
            match Session::start(self.config.as_ref().unwrap(), self.next_part, paused) {
                Ok(session) => {
                    self.session = Some(session);
                    self.mode = if paused {
                        Mode::Paused
                    } else {
                        Mode::Recording
                    };
                    self.notice = Some("System output changed. Recording continues in a new part; the switch may leave a brief gap.".into());
                }
                Err(error) => self.error = Some(error),
            }
            return;
        }
        let callbacks = session.shared.callbacks.load(Ordering::Relaxed);
        if callbacks != session.callbacks {
            session.callbacks = callbacks;
            session.last_callback = Instant::now();
        }
        if !session.capture.as_ref().unwrap().alive()
            || session.last_callback.elapsed() > Duration::from_secs(10)
        {
            self.stop();
            self.error = Some("Audio capture stopped responding. Check system audio permission and the output device, then start a new recording.".into());
            return;
        }
        if session.started.elapsed() > Duration::from_secs(8)
            && !session.shared.nonzero.load(Ordering::Relaxed)
            && self.notice.is_none()
        {
            self.notice = Some("No audible signal yet. Play audio in a selected app. If the meters stay empty, allow Senses in System Settings → Privacy & Security → Screen & System Audio Recording, then retry.".into());
        } else if session.shared.nonzero.load(Ordering::Relaxed)
            && self
                .notice
                .as_ref()
                .is_some_and(|n| n.starts_with("No audible"))
        {
            self.notice = None;
        }
    }
    pub fn snapshot(&mut self) -> Snapshot {
        self.maintain();
        let mut snapshot = Snapshot {
            state: self.mode.name(),
            seconds: self.seconds,
            left: 0.0,
            right: 0.0,
            dropped_frames: self.dropped,
            error: self.error.clone(),
            notice: self.notice.clone(),
            path: None,
            files: self.files.clone(),
            output_name: audio::output_name(),
        };
        if let Some(session) = &self.session {
            snapshot.seconds += session.seconds();
            snapshot.dropped_frames += session.shared.dropped.load(Ordering::Relaxed);
            if self.mode == Mode::Recording {
                snapshot.left = f32::from_bits(session.shared.left.load(Ordering::Relaxed));
                snapshot.right = f32::from_bits(session.shared.right.load(Ordering::Relaxed));
            }
            let state = session.shared.writer.lock().unwrap();
            snapshot.files.extend(state.files.clone());
            snapshot.path = state.path.clone();
        }
        snapshot
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_reject_invalid_commands() {
        assert!(Mode::Idle.pause().is_err());
        assert!(Mode::Idle.resume().is_err());
        assert_eq!(
            Mode::Recording.pause().unwrap().resume().unwrap(),
            Mode::Recording
        );
        assert!(Mode::Paused.pause().is_err());
        assert!(Mode::Recording.resume().is_err());
    }
    #[test]
    fn rejects_empty_selection_before_touching_audio_hardware() {
        let mut engine = Engine::default();
        let command: Command = serde_json::from_str(
            r#"{"command":"start","directory":"/tmp","stem":"test","format":"wav","all":false}"#,
        )
        .unwrap();
        assert!(engine.command(command).unwrap_err().contains("Select"));
    }
}
