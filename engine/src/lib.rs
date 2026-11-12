mod audio;
mod file;
mod queue;
mod recording;
#[cfg(test)]
mod soak;

use recording::{Command, Engine};
use std::{
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
};

fn json_string(value: impl serde::Serialize) -> *mut c_char {
    let json = serde_json::to_string(&value)
        .unwrap_or_else(|_| "{\"error\":\"Could not encode engine response.\"}".into());
    CString::new(json).unwrap().into_raw()
}
fn guarded(operation: impl FnOnce() -> serde_json::Value) -> *mut c_char {
    match catch_unwind(AssertUnwindSafe(|| {
        objc2::rc::autoreleasepool(|_| operation())
    })) {
        Ok(value) => json_string(value),
        Err(_) => json_string(
            serde_json::json!({"error":"The recording engine encountered an unexpected error. Stop recording and relaunch Senses."}),
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn senses_engine_create() -> *mut Engine {
    catch_unwind(|| Box::into_raw(Box::new(Engine::default()))).unwrap_or(std::ptr::null_mut())
}
/// # Safety
/// `engine` is a live handle returned by create, exclusively owned by this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn senses_engine_destroy(engine: *mut Engine) {
    if !engine.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(engine) })));
    }
}
/// # Safety
/// The handle is live and exclusively borrowed; JSON is a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn senses_command(engine: *mut Engine, json: *const c_char) -> *mut c_char {
    guarded(|| {
        if engine.is_null() || json.is_null() {
            return serde_json::json!({"error":"Recording engine unavailable."});
        }
        let result = unsafe { CStr::from_ptr(json) }
            .to_str()
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<Command>(text).map_err(|e| e.to_string()))
            .and_then(|command| unsafe { &mut *engine }.command(command));
        match result {
            Ok(()) => serde_json::json!({"ok":true}),
            Err(error) => serde_json::json!({"error":error}),
        }
    })
}
/// # Safety
/// The handle is live and exclusively borrowed for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn senses_snapshot(engine: *mut Engine) -> *mut c_char {
    guarded(|| {
        if engine.is_null() {
            return serde_json::json!({"error":"Recording engine unavailable."});
        }
        serde_json::to_value(unsafe { &mut *engine }.snapshot()).unwrap()
    })
}
#[unsafe(no_mangle)]
pub extern "C" fn senses_sources() -> *mut c_char {
    guarded(|| serde_json::to_value(audio::sources().unwrap_or_default()).unwrap())
}
/// # Safety
/// `value` was returned by this library and has not already been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn senses_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}
