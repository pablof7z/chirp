//! Chirp-owned C ABI wrappers over `nmp-native-runtime`.
//!
//! Chirp links one aggregate Rust archive from native shells. With the generic
//! FFI bundle deleted upstream, this app crate owns the C symbols its shells
//! still call while delegating runtime behavior to
//! [`nmp_native_runtime::NmpApp`].

use std::ffi::{c_char, c_void, CString};
use std::sync::Arc;

use nmp_native_runtime::{dispatch_action_bytes_typed, NmpApp, NmpConfigStatus};
use zeroize::Zeroizing;

use super::helpers::c_string_opt;

mod nip21;
mod refs;
pub use nip21::*;
pub use refs::*;

type NmpUpdateCallback = extern "C" fn(*mut c_void, *const u8, usize);
type NmpCapabilityCallback = extern "C" fn(*mut c_void, *const c_char) -> *mut c_char;

#[no_mangle]
pub extern "C" fn nmp_app_new() -> *mut NmpApp {
    Box::into_raw(Box::new(nmp_native_runtime::new_app()))
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_free(app: *mut NmpApp) {
    if app.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(app));
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_start(app: *mut NmpApp, visible_limit: u32, emit_hz: u32) {
    if let Some(app) = app_ref(app) {
        app.start_runtime(visible_limit as usize, emit_hz);
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_configure(app: *mut NmpApp, visible_limit: u32, emit_hz: u32) {
    if let Some(app) = app_ref(app) {
        app.configure_runtime(visible_limit as usize, emit_hz);
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_stop(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.stop_runtime();
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_reset(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.reset_runtime();
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_is_alive(app: *mut NmpApp) -> u8 {
    app_ref(app).is_some_and(NmpApp::is_alive) as u8
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_set_update_callback(
    app: *mut NmpApp,
    context: *mut c_void,
    callback: Option<NmpUpdateCallback>,
) {
    let Some(app) = app_ref(app) else { return };
    match callback {
        Some(callback) => {
            let context = context as usize;
            app.set_update_listener(Some(Arc::new(move |bytes: &[u8]| {
                callback(context as *mut c_void, bytes.as_ptr(), bytes.len());
            })));
        }
        None => app.set_update_listener(None),
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_set_capability_callback(
    app: *mut NmpApp,
    context: *mut c_void,
    callback: Option<NmpCapabilityCallback>,
) {
    let Some(app) = app_ref(app) else { return };
    let registration =
        callback.map(
            |callback| nmp_core::__ffi_internal::CapabilityCallbackRegistration {
                context: context as usize,
                callback,
            },
        );
    app.capability_callback_slot()
        .set_registration(registration);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_set_storage_path(app: *mut NmpApp, path: *const c_char) -> u32 {
    let Some(app) = app_ref(app) else {
        return NmpConfigStatus::NullApp.code();
    };
    app.set_storage_path(c_string_opt(path)).code()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_declare_incremental_apply(app: *mut NmpApp) -> i32 {
    let Some(app) = app_ref(app) else { return -1 };
    match app.declare_incremental_apply() {
        Ok(()) => 0,
        Err(nmp_core::substrate::IncrementalApplyError::AlreadyStarted) => 1,
        Err(nmp_core::substrate::IncrementalApplyError::RegistryUnavailable) => 2,
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_dispatch_action_bytes(
    app: *mut NmpApp,
    ptr: *const u8,
    len: usize,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return into_raw_string(r#"{"error":"null_app"}"#.to_string());
    };
    if ptr.is_null() {
        return into_raw_string(r#"{"error":"null_bytes"}"#.to_string());
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let outcome = dispatch_action_bytes_typed(app, bytes);
    let json = match (outcome.correlation_id, outcome.error) {
        (_, Some(error)) => serde_json::json!({ "error": error }).to_string(),
        (Some(correlation_id), None) => {
            serde_json::json!({ "correlation_id": correlation_id }).to_string()
        }
        (None, None) => serde_json::json!({ "error": "missing dispatch outcome" }).to_string(),
    };
    into_raw_string(json)
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_signin_nsec(app: *mut NmpApp, secret: *const c_char, make_active: u8) {
    let (Some(app), Some(secret)) = (app_ref(app), c_string_opt(secret)) else {
        return;
    };
    app.add_signer(
        nmp_core::SignerSource::LocalNsec(Zeroizing::new(secret)),
        make_active != 0,
    );
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_register_agent_nsec(app: *mut NmpApp, secret: *const c_char) {
    let (Some(app), Some(secret)) = (app_ref(app), c_string_opt(secret)) else {
        return;
    };
    app.add_signer(
        nmp_core::SignerSource::AppManagedLocalNsec(Zeroizing::new(secret)),
        false,
    );
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_signin_bunker(app: *mut NmpApp, uri: *const c_char, make_active: u8) {
    let (Some(app), Some(uri)) = (app_ref(app), c_string_opt(uri)) else {
        return;
    };
    app.add_signer(nmp_core::SignerSource::BunkerUri(uri), make_active != 0);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_switch_active(app: *mut NmpApp, identity_id: *const c_char) {
    let (Some(app), Some(identity_id)) = (app_ref(app), c_string_opt(identity_id)) else {
        return;
    };
    app.switch_active(identity_id);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_remove_account(app: *mut NmpApp, identity_id: *const c_char) {
    let (Some(app), Some(identity_id)) = (app_ref(app), c_string_opt(identity_id)) else {
        return;
    };
    app.remove_account(identity_id);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_add_relay(app: *mut NmpApp, url: *const c_char, role: *const c_char) {
    let (Some(app), Some(url)) = (app_ref(app), c_string_opt(url)) else {
        return;
    };
    let role = c_string_opt(role).unwrap_or_else(|| "both".to_string());
    app.add_relay(url, role);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_remove_relay(app: *mut NmpApp, url: *const c_char) {
    let (Some(app), Some(url)) = (app_ref(app), c_string_opt(url)) else {
        return;
    };
    app.remove_relay(url);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_retry_publish(app: *mut NmpApp, handle: *const c_char) {
    let (Some(app), Some(handle)) = (app_ref(app), c_string_opt(handle)) else {
        return;
    };
    app.retry_publish(handle);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_cancel_action(app: *mut NmpApp, correlation_id: *const c_char) {
    let (Some(app), Some(correlation_id)) = (app_ref(app), c_string_opt(correlation_id)) else {
        return;
    };
    app.cancel_publish(correlation_id);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_ack_action_stage(app: *mut NmpApp, correlation_id: *const c_char) {
    let (Some(app), Some(correlation_id)) = (app_ref(app), c_string_opt(correlation_id)) else {
        return;
    };
    if correlation_id.is_empty() {
        return;
    }
    app.send_cmd(nmp_core::actor::ActorCommand::ActionLedger(
        nmp_core::actor::ActionLedgerCommand::Ack(correlation_id),
    ));
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_load_older_feed(app: *mut NmpApp, feed_key: *const c_char) {
    let (Some(app), Some(feed_key)) = (app_ref(app), c_string_opt(feed_key)) else {
        return;
    };
    let _ = app.load_older_feed(&feed_key);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_lifecycle_foreground(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.lifecycle_foreground();
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_lifecycle_background(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.lifecycle_background();
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_signer_broker_init(app: *mut NmpApp) -> u32 {
    if app.is_null() {
        return NmpConfigStatus::NullApp.code();
    }
    unsafe { &*app }.init_signer_broker().code()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_cancel_bunker_handshake(app: *mut NmpApp) {
    if app.is_null() {
        return;
    }
    unsafe { &*app }.cancel_bunker_handshake();
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_nostrconnect_uri(
    app: *mut NmpApp,
    callback_scheme: *const c_char,
) -> *mut c_char {
    if app.is_null() {
        return std::ptr::null_mut();
    }
    let callback = c_string_opt(callback_scheme);
    let Some(uri) = (unsafe { &*app }).nostrconnect_uri(callback.as_deref()) else {
        return std::ptr::null_mut();
    };
    CString::new(uri)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_open_uri(app: *mut NmpApp, uri: *const c_char) {
    let (Some(app), Some(uri)) = (app_ref(app), c_string_opt(uri)) else {
        return;
    };
    app.open_uri(uri);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_dispatch_capability(
    app: *mut NmpApp,
    request_json: *const c_char,
) -> *mut c_char {
    let (Some(app), Some(request_json)) = (app_ref(app), c_string_opt(request_json)) else {
        return std::ptr::null_mut();
    };
    into_raw_string(nmp_core::__ffi_internal::dispatch_capability(
        &app.capability_callback_slot(),
        &request_json,
    ))
}

#[cfg(feature = "android-ffi")]
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_external_signer_init(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.init_external_signer();
    }
}

#[cfg(feature = "android-ffi")]
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_signin_nip55(app: *mut NmpApp, signer_package: *const c_char) {
    if let Some(app) = app_ref(app) {
        app.signin_nip55(c_string_opt(signer_package));
    }
}

#[cfg(feature = "android-ffi")]
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_deliver_external_signer_response(
    app: *mut NmpApp,
    response_json: *const c_char,
) {
    let (Some(app), Some(response_json)) = (app_ref(app), c_string_opt(response_json)) else {
        return;
    };
    app.deliver_external_signer_response(&response_json);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: every caller-facing string this crate returns is allocated with
    // `CString::into_raw`; reclaim it exactly once here.
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[must_use]
pub(super) fn into_raw_string(value: String) -> *mut c_char {
    CString::new(value)
        .unwrap_or_else(|_| CString::new("").unwrap_or_default())
        .into_raw()
}

#[must_use]
pub(super) fn app_ref<'a>(app: *mut NmpApp) -> Option<&'a NmpApp> {
    if app.is_null() {
        None
    } else {
        Some(unsafe { &*app })
    }
}
