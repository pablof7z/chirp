use std::ffi::{c_char, c_void, CString};
use std::ptr;
use std::sync::Arc;

use nmp_core::__ffi_internal::{
    dispatch_capability, CapabilityCallback, CapabilityCallbackRegistration, LifecycleObserverFn,
    LifecycleObserverRegistration, DEFAULT_EMIT_HZ, DEFAULT_VISIBLE_LIMIT,
};
use nmp_core::actor::{ActionLedgerCommand, ActorCommand, InterestsCommand};
use nmp_native_runtime::{
    dispatch_action_bytes_typed, empty_debug_info_json, NmpApp, NmpConfigStatus,
};

use super::{app_ref, c_string_nonempty, c_string_opt, into_c_string, json_error};

pub type NmpUpdateCallback = extern "C" fn(*mut c_void, *const u8, usize);
pub type NmpActionResultObserver = extern "C" fn(*const c_char);

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
        let app = Box::from_raw(app);
        app.shutdown();
    }
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_set_update_callback(
    app: *mut NmpApp,
    context: *mut c_void,
    callback: Option<NmpUpdateCallback>,
) {
    let Some(app) = app_ref(app) else { return };
    let listener = callback.map(|cb| {
        let context = context as usize;
        Arc::new(move |bytes: &[u8]| {
            cb(context as *mut c_void, bytes.as_ptr(), bytes.len());
        }) as nmp_native_runtime::UpdateListener
    });
    app.set_update_listener(listener);
}

#[no_mangle]
pub extern "C" fn nmp_app_set_storage_path(app: *mut NmpApp, path: *const c_char) -> u32 {
    let Some(app) = app_ref(app) else {
        return NmpConfigStatus::NullApp.code();
    };
    app.set_storage_path(c_string_opt(path)).code()
}

#[no_mangle]
pub extern "C" fn nmp_app_start(app: *mut NmpApp, visible_limit: u32, emit_hz: u32) {
    let Some(app) = app_ref(app) else { return };
    app.start_runtime(clamp_visible_limit(visible_limit), clamp_emit_hz(emit_hz));
}

#[no_mangle]
pub extern "C" fn nmp_app_configure(app: *mut NmpApp, visible_limit: u32, emit_hz: u32) {
    let Some(app) = app_ref(app) else { return };
    app.configure_runtime(clamp_visible_limit(visible_limit), clamp_emit_hz(emit_hz));
}

#[no_mangle]
pub extern "C" fn nmp_app_stop(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.stop_runtime();
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_reset(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.reset_runtime();
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_is_alive(app: *mut NmpApp) -> u8 {
    app_ref(app).is_some_and(NmpApp::is_alive).into()
}

#[no_mangle]
pub extern "C" fn nmp_app_lifecycle_foreground(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.lifecycle_foreground();
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_lifecycle_background(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.lifecycle_background();
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_set_lifecycle_callback(
    app: *mut NmpApp,
    context: *mut c_void,
    callback: Option<LifecycleObserverFn>,
) {
    let Some(app) = app_ref(app) else { return };
    let registration = callback.map(|callback| LifecycleObserverRegistration {
        context: context as usize,
        callback,
    });
    app.set_lifecycle_observer(registration);
}

#[no_mangle]
pub extern "C" fn nmp_app_set_capability_callback(
    app: *mut NmpApp,
    context: *mut c_void,
    callback: Option<CapabilityCallback>,
) {
    let Some(app) = app_ref(app) else { return };
    let registration = callback.map(|callback| CapabilityCallbackRegistration {
        context: context as usize,
        callback,
    });
    app.capability_callback_slot()
        .set_registration(registration);
}

#[no_mangle]
pub extern "C" fn nmp_app_dispatch_capability(
    app: *mut NmpApp,
    request_json: *const c_char,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return json_error("null_app");
    };
    let Some(request) = c_string_opt(request_json) else {
        return json_error("bad_request");
    };
    into_c_string(dispatch_capability(
        &app.capability_callback_slot(),
        &request,
    ))
}

#[no_mangle]
pub extern "C" fn nmp_app_dispatch_action_bytes(
    app: *mut NmpApp,
    ptr: *const u8,
    len: usize,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return json_error("runtime app is not available");
    };
    if ptr.is_null() {
        return json_error("action dispatch payload is null");
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let outcome = dispatch_action_bytes_typed(app, bytes);
    let value = match (outcome.correlation_id, outcome.error, outcome.code) {
        (Some(correlation_id), None, _) => serde_json::json!({ "correlation_id": correlation_id }),
        (Some(correlation_id), Some(error), None) => {
            serde_json::json!({ "correlation_id": correlation_id, "error": error })
        }
        (Some(correlation_id), Some(error), Some(code)) => {
            serde_json::json!({ "correlation_id": correlation_id, "error": error, "code": code })
        }
        (None, Some(error), None) => serde_json::json!({ "error": error }),
        (None, Some(error), Some(code)) => serde_json::json!({ "error": error, "code": code }),
        (None, None, _) => serde_json::json!({ "error": "action dispatch returned no outcome" }),
    };
    into_c_string(value.to_string())
}

#[no_mangle]
pub extern "C" fn nmp_app_open_interest(
    app: *mut NmpApp,
    filter_json: *const c_char,
    consumer_id: *const c_char,
    scope: u32,
) {
    let Some(app) = app_ref(app) else { return };
    let (Some(filter_json), Some(consumer_id)) = (
        c_string_nonempty(filter_json),
        c_string_nonempty(consumer_id),
    ) else {
        return;
    };
    app.send_cmd(ActorCommand::Interests(InterestsCommand::OpenInterest {
        filter_json,
        consumer_id,
        scope,
    }));
}

#[no_mangle]
pub extern "C" fn nmp_app_close_interest(
    app: *mut NmpApp,
    filter_json: *const c_char,
    consumer_id: *const c_char,
    scope: u32,
) {
    let Some(app) = app_ref(app) else { return };
    let (Some(filter_json), Some(consumer_id)) = (
        c_string_nonempty(filter_json),
        c_string_nonempty(consumer_id),
    ) else {
        return;
    };
    app.send_cmd(ActorCommand::Interests(InterestsCommand::CloseInterest {
        filter_json,
        consumer_id,
        scope,
        relay_pin: None,
    }));
}

#[no_mangle]
pub extern "C" fn nmp_app_load_older_feed(app: *mut NmpApp, feed_key: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(key) = c_string_nonempty(feed_key) {
        let _ = app.load_older_feed(&key);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_ack_action_stage(app: *mut NmpApp, correlation_id: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(correlation_id) = c_string_nonempty(correlation_id) {
        app.send_cmd(ActorCommand::ActionLedger(ActionLedgerCommand::Ack(
            correlation_id,
        )));
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_register_action_result_observer(
    app: *mut NmpApp,
    observer: Option<NmpActionResultObserver>,
) {
    let Some(app) = app_ref(app) else { return };
    match observer {
        Some(observer) => app.register_action_result_observer(move |result| {
            if let Ok(json) = serde_json::to_string(&result) {
                let c = CString::new(json.replace('\0', "")).unwrap_or_default();
                observer(c.as_ptr());
            }
        }),
        None => app.clear_action_result_observer(),
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_declare_consumed_projections(
    app: *mut NmpApp,
    keys: *const *const c_char,
    len: usize,
) {
    let Some(app) = app_ref(app) else { return };
    if keys.is_null() || len == 0 {
        return;
    }
    let raw = unsafe { std::slice::from_raw_parts(keys, len) };
    let keys = raw
        .iter()
        .filter_map(|ptr| c_string_nonempty(*ptr))
        .collect::<Vec<_>>();
    app.declare_consumed_projections(keys);
}

#[no_mangle]
pub extern "C" fn nmp_app_consume_all_builtin_projections(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.consume_all_builtin_projections();
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_declare_incremental_apply(app: *mut NmpApp) -> i32 {
    let Some(app) = app_ref(app) else { return -1 };
    match app.declare_incremental_apply() {
        Ok(()) => 0,
        Err(nmp_core::substrate::IncrementalApplyError::AlreadyStarted) => 1,
        Err(nmp_core::substrate::IncrementalApplyError::RegistryUnavailable) => 2,
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_debug_info(app: *mut NmpApp, domain: i32) -> *mut c_char {
    let value = app_ref(app)
        .map(|app| app.debug_info_json(domain))
        .unwrap_or_else(|| empty_debug_info_json(domain));
    into_c_string(value.to_string())
}

#[no_mangle]
pub extern "C" fn nmp_app_open_uri(app: *mut NmpApp, uri: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(uri) = c_string_nonempty(uri) {
        app.open_uri(uri);
    }
}

#[no_mangle]
pub extern "C" fn nmp_signer_broker_init(app: *mut NmpApp) -> u32 {
    let Some(app) = app_ref(app) else {
        return NmpConfigStatus::NullApp.code();
    };
    app.init_signer_broker().code()
}

#[no_mangle]
pub extern "C" fn nmp_app_cancel_bunker_handshake(app: *mut NmpApp) {
    if let Some(app) = app_ref(app) {
        app.cancel_bunker_handshake();
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_nostrconnect_uri(
    app: *mut NmpApp,
    callback_scheme: *const c_char,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return ptr::null_mut();
    };
    match app.nostrconnect_uri(c_string_opt(callback_scheme).as_deref()) {
        Some(uri) => into_c_string(uri),
        None => ptr::null_mut(),
    }
}

fn clamp_visible_limit(value: u32) -> usize {
    match value {
        0 => DEFAULT_VISIBLE_LIMIT,
        n => (n as usize).clamp(1, 500),
    }
}

fn clamp_emit_hz(value: u32) -> u32 {
    match value {
        0 => DEFAULT_EMIT_HZ,
        n => n.clamp(1, 12),
    }
}
