use crate::recording::Shared;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_core_audio::*;
use objc2_core_audio_types::*;
use objc2_core_foundation::{CFDictionary, CFRetained, CFString};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use serde::Serialize;
use std::{
    ffi::c_void,
    mem::size_of,
    ptr::{self, NonNull},
    sync::{Arc, atomic::Ordering},
};

pub fn check(code: i32, operation: &str) -> Result<(), String> {
    if code == 0 {
        return Ok(());
    }
    let chars: String = code
        .to_be_bytes()
        .iter()
        .map(|b| {
            if b.is_ascii_graphic() {
                *b as char
            } else {
                '.'
            }
        })
        .collect();
    Err(format!("{operation} failed ({code}, '{chars}')."))
}

fn address(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

pub trait PropertyValue: Copy {
    fn initial() -> Self;
}
impl PropertyValue for u32 {
    fn initial() -> Self {
        0
    }
}
impl PropertyValue for i32 {
    fn initial() -> Self {
        0
    }
}
impl PropertyValue for f64 {
    fn initial() -> Self {
        0.0
    }
}
impl PropertyValue for *mut CFString {
    fn initial() -> Self {
        ptr::null_mut()
    }
}
impl PropertyValue for AudioStreamBasicDescription {
    fn initial() -> Self {
        crate::file::float_format(0.0)
    }
}

pub fn property<T: PropertyValue>(object: u32, selector: u32) -> Result<T, String> {
    let mut value = T::initial();
    let mut len = size_of::<T>() as u32;
    unsafe {
        check(
            AudioObjectGetPropertyData(
                object,
                NonNull::from(&mut address(selector)),
                0,
                ptr::null(),
                NonNull::from(&mut len),
                NonNull::from(&mut value).cast(),
            ),
            "Read audio property",
        )?;
    }
    if len != size_of::<T>() as u32 {
        return Err("Audio property had an unexpected size.".into());
    }
    Ok(value)
}

pub fn string_property(object: u32, selector: u32) -> Result<String, String> {
    let pointer: *mut CFString = property(object, selector)?;
    let pointer = NonNull::new(pointer).ok_or("Audio property returned a null string.")?;
    // Core Audio transfers a +1 CFString reference for these properties.
    Ok(unsafe { CFRetained::from_raw(pointer) }.to_string())
}

fn process_ids() -> Result<Vec<u32>, String> {
    let mut addr = address(kAudioHardwarePropertyProcessObjectList);
    let mut len = 0;
    unsafe {
        check(
            AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject as u32,
                NonNull::from(&mut addr),
                0,
                ptr::null(),
                NonNull::from(&mut len),
            ),
            "List audio processes",
        )?;
        if len == 0 {
            return Ok(Vec::new());
        }
        let mut ids = vec![0u32; len as usize / 4];
        check(
            AudioObjectGetPropertyData(
                kAudioObjectSystemObject as u32,
                NonNull::from(&mut addr),
                0,
                ptr::null(),
                NonNull::from(&mut len),
                NonNull::new(ids.as_mut_ptr()).unwrap().cast(),
            ),
            "Read audio processes",
        )?;
        ids.truncate(len as usize / 4);
        Ok(ids)
    }
}

#[derive(Serialize)]
pub struct Process {
    pub pid: i32,
    pub bundle_id: String,
}
pub fn sources() -> Result<Vec<Process>, String> {
    Ok(process_ids()?
        .into_iter()
        .filter_map(|id| {
            let pid = property(id, kAudioProcessPropertyPID).ok()?;
            let bundle_id = string_property(id, kAudioProcessPropertyBundleID).unwrap_or_default();
            Some(Process { pid, bundle_id })
        })
        .collect())
}

pub fn default_output() -> Result<u32, String> {
    let id = property(
        kAudioObjectSystemObject as u32,
        kAudioHardwarePropertyDefaultOutputDevice,
    )?;
    if id == 0 {
        Err("No system audio output is available.".into())
    } else {
        Ok(id)
    }
}
pub fn output_name() -> String {
    default_output()
        .and_then(|id| string_property(id, kAudioObjectPropertyName))
        .unwrap_or_else(|_| "System output unavailable".into())
}

fn dict(entries: &[(&str, &AnyObject)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: Vec<_> = entries.iter().map(|(k, _)| NSString::from_str(k)).collect();
    let key_refs: Vec<_> = keys.iter().map(|k| &**k).collect();
    let values: Vec<_> = entries.iter().map(|(_, v)| *v).collect();
    NSDictionary::from_slices(&key_refs, &values)
}

pub struct Capture {
    tap: u32,
    device: u32,
    input_stream: u32,
    io: AudioDeviceIOProcID,
    pub output: u32,
    pub rate: f64,
    description: Retained<CATapDescription>,
    shared: Arc<Shared>,
    started: bool,
}

impl Capture {
    pub fn new(
        all: bool,
        pids: &[i32],
        bundles: &[String],
        shared: Arc<Shared>,
    ) -> Result<Self, String> {
        unsafe {
            let description = CATapDescription::new();
            description.setName(&NSString::from_str("Senses capture"));
            description.setPrivate(true);
            description.setMixdown(true);
            description.setMono(false);
            description.setMuteBehavior(CATapMuteBehavior::Unmuted);
            description.setExclusive(all);
            // Restore explicit app bundle IDs, but refresh shared helper PIDs
            // ourselves: automatic PID restoration by bundle ID could attach
            // another app's instance of a shared WebKit helper.
            description.setProcessRestoreEnabled(false);
            let mut capture = Self {
                tap: 0,
                device: 0,
                input_stream: 0,
                io: None,
                output: default_output()?,
                rate: 0.0,
                description,
                shared,
                started: false,
            };
            capture.set_membership(all, pids, bundles)?;
            check(
                AudioHardwareCreateProcessTap(Some(&capture.description), &mut capture.tap),
                "Create audio capture (allow Senses in System Settings → Privacy & Security → Screen & System Audio Recording)",
            )?;
            let format: AudioStreamBasicDescription =
                property(capture.tap, kAudioTapPropertyFormat)?;
            if format.mFormatID != kAudioFormatLinearPCM
                || format.mBitsPerChannel != 32
                || format.mFormatFlags & kAudioFormatFlagIsFloat == 0
                || format.mChannelsPerFrame != 2
                || !format.mSampleRate.is_finite()
                || format.mSampleRate <= 0.0
            {
                return Err(
                    "The output device did not provide supported stereo float audio.".into(),
                );
            }
            let output_uid = NSString::from_str(&string_property(
                capture.output,
                kAudioDevicePropertyDeviceUID,
            )?);
            let tap_uid = capture.description.UUID().UUIDString();
            let uid = NSString::from_str(&format!("local.senses.capture.{}", tap_uid));
            let name = NSString::from_str("Senses Private Capture");
            let yes = NSNumber::new_bool(true);
            // Match Apple's aggregate recipe: real output clock, tap input, drift compensation.
            let sub = dict(&[("uid", &output_uid)]);
            let tap = dict(&[("uid", &tap_uid), ("drift", &yes)]);
            let subs = NSArray::from_slice(&[&*sub]);
            let taps = NSArray::from_slice(&[&*tap]);
            let config = dict(&[
                ("name", &name),
                ("uid", &uid),
                ("private", &yes),
                ("master", &output_uid),
                ("subdevices", &subs),
                ("taps", &taps),
                ("tapautostart", &yes),
            ]);
            // NSDictionary and CFDictionary are toll-free bridged.
            let cf =
                &*((&*config as *const NSDictionary<NSString, AnyObject>).cast::<CFDictionary>());
            check(
                AudioHardwareCreateAggregateDevice(cf, NonNull::from(&mut capture.device)),
                "Create private recording device",
            )?;
            check(
                AudioDeviceCreateIOProcID(
                    capture.device,
                    Some(callback),
                    Arc::as_ptr(&capture.shared) as *mut c_void,
                    NonNull::from(&mut capture.io),
                ),
                "Prepare audio callback",
            )?;
            capture.disable_physical_inputs()?;
            Ok(capture)
        }
    }
    pub fn start(&mut self) -> Result<(), String> {
        unsafe {
            check(
                AudioDeviceStart(self.device, self.io),
                "Start audio capture",
            )?;
        }
        self.started = true;
        Ok(())
    }
    pub fn alive(&self) -> bool {
        property::<u32>(self.device, kAudioDevicePropertyDeviceIsAlive).unwrap_or(0) != 0
    }
    pub fn format_changed(&self) -> bool {
        property::<f64>(self.device, kAudioDevicePropertyNominalSampleRate)
            .map(|rate| rate != self.rate)
            .unwrap_or(false)
    }
    fn disable_physical_inputs(&mut self) -> Result<(), String> {
        // An external output may have microphone streams. Disable every
        // physical input, leaving only the appended tap stream enabled.
        let mut addr = address(kAudioDevicePropertyStreams);
        addr.mScope = kAudioObjectPropertyScopeInput;
        let mut bytes = 0u32;
        unsafe {
            check(
                AudioObjectGetPropertyDataSize(
                    self.device,
                    NonNull::from(&mut addr),
                    0,
                    ptr::null(),
                    NonNull::from(&mut bytes),
                ),
                "Inspect capture streams",
            )?;
            let count = bytes as usize / size_of::<u32>();
            if count == 0 || count > 64 {
                return Err("Unexpected number of recording streams.".into());
            }
            let mut streams = vec![0u32; count];
            check(
                AudioObjectGetPropertyData(
                    self.device,
                    NonNull::from(&mut addr),
                    0,
                    ptr::null(),
                    NonNull::from(&mut bytes),
                    NonNull::new(streams.as_mut_ptr()).unwrap().cast(),
                ),
                "Read capture streams",
            )?;
            self.input_stream = streams[count - 1];
            // The aggregate resamples the tap to its own clock. A Bluetooth
            // master can be 44.1 kHz while both CATap and the virtual stream
            // report 48 kHz. IOProc frame timing follows the aggregate clock.
            let delivered: AudioStreamBasicDescription =
                property(self.input_stream, kAudioStreamPropertyVirtualFormat)?;
            if delivered.mFormatID != kAudioFormatLinearPCM
                || delivered.mBitsPerChannel != 32
                || delivered.mFormatFlags & kAudioFormatFlagIsFloat == 0
                || delivered.mChannelsPerFrame != 2
                || !delivered.mSampleRate.is_finite()
                || delivered.mSampleRate <= 0.0
            {
                return Err("Unsupported recording stream format.".into());
            }
            self.rate = property(self.device, kAudioDevicePropertyNominalSampleRate)?;
            if !self.rate.is_finite() || self.rate <= 0.0 {
                return Err("Invalid recording clock rate.".into());
            }
            #[repr(C)]
            struct Usage {
                io: AudioDeviceIOProcID,
                count: u32,
                enabled: [u32; 64],
            }
            let mut usage = Usage {
                io: self.io,
                count: count as u32,
                enabled: [0; 64],
            };
            usage.enabled[count - 1] = 1;
            addr.mSelector = kAudioDevicePropertyIOProcStreamUsage;
            let size =
                size_of::<AudioDeviceIOProcID>() + size_of::<u32>() + count * size_of::<u32>();
            check(
                AudioObjectSetPropertyData(
                    self.device,
                    NonNull::from(&mut addr),
                    0,
                    ptr::null(),
                    size as u32,
                    NonNull::from(&mut usage).cast(),
                ),
                "Disable microphone streams",
            )?;
        }
        Ok(())
    }
    pub fn set_membership(
        &mut self,
        all: bool,
        pids: &[i32],
        bundles: &[String],
    ) -> Result<(), String> {
        let own = std::process::id() as i32;
        let ids: Vec<_> = process_ids()?
            .into_iter()
            .filter(|id| {
                let pid = property::<i32>(*id, kAudioProcessPropertyPID).unwrap_or(0);
                if all {
                    pid == own
                } else {
                    pid != own && pids.contains(&pid)
                }
            })
            .map(NSNumber::new_u32)
            .collect();
        let nsids = NSArray::from_retained_slice(&ids);
        let strings: Vec<_> = if all {
            vec![NSString::from_str("local.senses.recorder")]
        } else {
            bundles
                .iter()
                .filter(|b| !b.starts_with("pid:") && *b != "local.senses.recorder")
                .map(|b| NSString::from_str(b))
                .collect()
        };
        let nsbundles = NSArray::from_retained_slice(&strings);
        unsafe {
            self.description.setProcesses(&nsids);
            self.description.setBundleIDs(&nsbundles);
            if self.tap != 0 {
                let mut ptr = Retained::as_ptr(&self.description);
                check(
                    AudioObjectSetPropertyData(
                        self.tap,
                        NonNull::from(&mut address(kAudioTapPropertyDescription)),
                        0,
                        ptr::null(),
                        size_of::<*const CATapDescription>() as u32,
                        NonNull::from(&mut ptr).cast(),
                    ),
                    "Update selected apps",
                )?;
            }
        }
        Ok(())
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        unsafe {
            if self.started {
                AudioDeviceStop(self.device, self.io);
            }
            if self.io.is_some() {
                AudioDeviceDestroyIOProcID(self.device, self.io);
            }
            if self.device != 0 {
                AudioHardwareDestroyAggregateDevice(self.device);
            }
            if self.tap != 0 {
                AudioHardwareDestroyProcessTap(self.tap);
            }
        }
    }
}

unsafe extern "C-unwind" fn callback(
    _: u32,
    _: NonNull<AudioTimeStamp>,
    input: NonNull<AudioBufferList>,
    _: NonNull<AudioTimeStamp>,
    output: NonNull<AudioBufferList>,
    _: NonNull<AudioTimeStamp>,
    context: *mut c_void,
) -> i32 {
    // No allocations, locks, Objective-C calls, disk I/O, or UI work here.
    let shared = unsafe { &*context.cast::<Shared>() };
    let out = unsafe { output.as_ref() };
    if out.mNumberBuffers <= 64 {
        for buffer in unsafe {
            std::slice::from_raw_parts(out.mBuffers.as_ptr(), out.mNumberBuffers as usize)
        } {
            if !buffer.mData.is_null() {
                unsafe {
                    ptr::write_bytes(buffer.mData.cast::<u8>(), 0, buffer.mDataByteSize as usize);
                }
            }
        }
    }
    shared.callbacks.fetch_add(1, Ordering::Relaxed);
    if shared.paused.load(Ordering::Relaxed) || shared.stop.load(Ordering::Relaxed) {
        return 0;
    }
    let list = unsafe { input.as_ref() };
    let count = list.mNumberBuffers as usize;
    if count == 0 {
        return 0;
    }
    if count > 16 {
        shared.fail(2);
        return 0;
    }
    let buffers = unsafe { std::slice::from_raw_parts(list.mBuffers.as_ptr(), count) };
    // The physical output can also expose inputs. The tap is the last input
    // stream in the aggregate; take its final two channels, never microphones.
    let last = &buffers[count - 1];
    let (left, right, interleaved) = if last.mNumberChannels == 2 {
        (last, last, true)
    } else if count >= 2 && last.mNumberChannels == 1 && buffers[count - 2].mNumberChannels == 1 {
        (&buffers[count - 2], last, false)
    } else {
        shared.fail(2);
        return 0;
    };
    let divisor = if interleaved { 8 } else { 4 };
    let frames = left.mDataByteSize as usize / divisor;
    if frames == 0 {
        return 0;
    }
    if frames > crate::queue::MAX_FRAMES || right.mDataByteSize < left.mDataByteSize {
        shared.fail(2);
        return 0;
    }
    let ok = unsafe {
        shared.queue.push(frames, |samples| {
            if left.mData.is_null() || right.mData.is_null() {
                samples.fill(0.0);
                return;
            }
            if interleaved {
                ptr::copy_nonoverlapping(
                    left.mData.cast::<f32>(),
                    samples.as_mut_ptr(),
                    frames * 2,
                );
            } else {
                let l = left.mData.cast::<f32>();
                let r = right.mData.cast::<f32>();
                for i in 0..frames {
                    samples[i * 2] = *l.add(i);
                    samples[i * 2 + 1] = *r.add(i);
                }
            }
        })
    };
    if !ok {
        shared.dropped.fetch_add(frames as u64, Ordering::Relaxed);
        shared.fail(1);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe fn invoke(
        shared: &Shared,
        input: NonNull<AudioBufferList>,
        output: NonNull<AudioBufferList>,
    ) {
        let mut timestamp: AudioTimeStamp = unsafe { std::mem::zeroed() };
        unsafe {
            callback(
                0,
                NonNull::from(&mut timestamp),
                input,
                NonNull::from(&mut timestamp),
                output,
                NonNull::from(&mut timestamp),
                (shared as *const Shared).cast_mut().cast(),
            );
        }
    }
    #[test]
    fn planar_input_is_interleaved_without_replaying_to_output() {
        let shared = Shared::new(false);
        let mut left = [0.1f32, 0.3];
        let mut right = [-0.2f32, -0.4];
        let mut speakers = [1.0f32; 4];
        #[repr(C)]
        struct TwoBuffers {
            count: u32,
            buffers: [AudioBuffer; 2],
        }
        let mut input = TwoBuffers {
            count: 2,
            buffers: [
                AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: 8,
                    mData: left.as_mut_ptr().cast(),
                },
                AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: 8,
                    mData: right.as_mut_ptr().cast(),
                },
            ],
        };
        let mut output = AudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [AudioBuffer {
                mNumberChannels: 2,
                mDataByteSize: 16,
                mData: speakers.as_mut_ptr().cast(),
            }],
        };
        unsafe {
            invoke(
                &shared,
                NonNull::from(&mut input).cast(),
                NonNull::from(&mut output),
            );
            assert!(
                shared
                    .queue
                    .pop(|b| assert_eq!(&b.samples[..4], &[0.1, -0.2, 0.3, -0.4]))
            );
        }
        assert_eq!(speakers, [0.0; 4]);
    }
    #[test]
    fn callback_stops_at_first_overflow_so_the_file_has_no_hidden_gap() {
        let shared = Shared::new(false);
        unsafe { while shared.queue.push(1, |s| s.fill(0.1)) {} }
        let mut samples = [0.2f32, 0.3];
        let mut input = AudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [AudioBuffer {
                mNumberChannels: 2,
                mDataByteSize: 8,
                mData: samples.as_mut_ptr().cast(),
            }],
        };
        let mut output = AudioBufferList {
            mNumberBuffers: 0,
            mBuffers: [AudioBuffer {
                mNumberChannels: 0,
                mDataByteSize: 0,
                mData: ptr::null_mut(),
            }],
        };
        unsafe {
            invoke(
                &shared,
                NonNull::from(&mut input),
                NonNull::from(&mut output),
            );
        }
        assert_eq!(shared.fault.load(Ordering::Acquire), 1);
        assert!(shared.stop.load(Ordering::Acquire));
        assert_eq!(shared.dropped.load(Ordering::Relaxed), 1);
        unsafe {
            while shared.queue.pop(|b| assert_eq!(b.samples[0], 0.1)) {}
            invoke(
                &shared,
                NonNull::from(&mut input),
                NonNull::from(&mut output),
            );
            assert!(
                !shared
                    .queue
                    .pop(|_| panic!("capture must remain stopped after overflow"))
            );
        }
    }
    #[test]
    fn callback_preserves_stereo_and_omits_paused_time() {
        let shared = Shared::new(false);
        let mut samples = [0.1f32, -0.2, 0.3, -0.4];
        let mut input = AudioBufferList {
            mNumberBuffers: 1,
            mBuffers: [AudioBuffer {
                mNumberChannels: 2,
                mDataByteSize: 16,
                mData: samples.as_mut_ptr().cast(),
            }],
        };
        let mut output = AudioBufferList {
            mNumberBuffers: 0,
            mBuffers: [AudioBuffer {
                mNumberChannels: 0,
                mDataByteSize: 0,
                mData: ptr::null_mut(),
            }],
        };
        let mut timestamp: AudioTimeStamp = unsafe { std::mem::zeroed() };
        let mut invoke = || unsafe {
            callback(
                0,
                NonNull::from(&mut timestamp),
                NonNull::from(&mut input),
                NonNull::from(&mut timestamp),
                NonNull::from(&mut output),
                NonNull::from(&mut timestamp),
                (&shared as *const Shared).cast_mut().cast(),
            );
        };
        invoke();
        unsafe {
            assert!(
                shared
                    .queue
                    .pop(|b| assert_eq!(&b.samples[..4], &[0.1, -0.2, 0.3, -0.4]))
            );
        }
        shared.paused.store(true, Ordering::Release);
        invoke();
        unsafe {
            assert!(
                !shared
                    .queue
                    .pop(|_| panic!("paused frames must not enter the file"))
            );
        }
        shared.paused.store(false, Ordering::Release);
        invoke();
        unsafe {
            assert!(shared.queue.pop(|b| assert_eq!(b.frames, 2)));
        }
        shared.stop.store(true, Ordering::Release);
        invoke();
        unsafe {
            assert!(
                !shared
                    .queue
                    .pop(|_| panic!("stopped frames must not enter the file"))
            );
        }
        assert_eq!(shared.dropped.load(Ordering::Relaxed), 0);
    }

    #[test]
    #[ignore = "Read-only hardware format probe; does not start audio I/O"]
    fn probe_delivered_capture_format() {
        objc2::rc::autoreleasepool(|_| {
            let capture = Capture::new(false, &[], &[], Arc::new(Shared::new(false))).unwrap();
            let tap: AudioStreamBasicDescription =
                property(capture.tap, kAudioTapPropertyFormat).unwrap();
            let device_rate: f64 =
                property(capture.device, kAudioDevicePropertyNominalSampleRate).unwrap();
            eprintln!(
                "tap={} Hz; delivered={} Hz; aggregate={} Hz",
                tap.mSampleRate, capture.rate, device_rate
            );
            assert_eq!(
                capture.rate, device_rate,
                "writer must use the aggregate clock's delivered sample rate"
            );
            assert!(!capture.format_changed());
        });
    }
}
