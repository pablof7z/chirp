//! Chirp-owned native delivery glue over the current `nmp-native-runtime` API.
//!
//! NMP no longer exports the reusable raw C symbol set. These functions keep
//! Chirp's existing shells building while forwarding directly to public NMP
//! Rust methods and typed command seams.

use std::ffi::{c_char, CStr, CString};

use nmp_native_runtime::NmpApp;

mod app;
mod identity;
mod marmot;
mod mirror;
mod refs;
mod search;
mod stateless;

pub use app::*;
pub use identity::*;
pub use marmot::*;
pub use mirror::*;
pub use refs::*;
pub use search::*;
pub use stateless::*;

pub(crate) fn app_ref<'a>(app: *mut NmpApp) -> Option<&'a NmpApp> {
    if app.is_null() {
        None
    } else {
        Some(unsafe { &*app })
    }
}

pub(crate) fn const_app_ref<'a>(app: *const NmpApp) -> Option<&'a NmpApp> {
    if app.is_null() {
        None
    } else {
        Some(unsafe { &*app })
    }
}

pub(crate) fn c_string_opt(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .map(str::to_string)
}

pub(crate) fn c_string_nonempty(ptr: *const c_char) -> Option<String> {
    c_string_opt(ptr).and_then(|s| (!s.is_empty()).then_some(s))
}

pub(crate) fn into_c_string(text: impl Into<String>) -> *mut c_char {
    let text = text.into().replace('\0', "");
    CString::new(text)
        .unwrap_or_else(|_| CString::new("{}").expect("literal has no nul"))
        .into_raw()
}

pub(crate) fn json_error(message: &str) -> *mut c_char {
    into_c_string(serde_json::json!({ "error": message }).to_string())
}
