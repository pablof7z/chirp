use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::{Mutex, OnceLock};

use nmp_native_runtime::{parse_search_request, Nip50SearchHandle, Nip50SearchSession, NmpApp};

use super::{app_ref, c_string_nonempty};

static SEARCH_HANDLES: OnceLock<Mutex<HashMap<(usize, String), Nip50SearchHandle>>> =
    OnceLock::new();

#[no_mangle]
pub extern "C" fn nmp_app_search_open(
    app: *mut NmpApp,
    request_json: *const c_char,
    session_id: *const c_char,
) {
    let Some(app_ref) = app_ref(app) else { return };
    let (Some(request_json), Some(session_id)) = (
        c_string_nonempty(request_json),
        c_string_nonempty(session_id),
    ) else {
        return;
    };
    let Some(request) = parse_search_request(&request_json) else {
        return;
    };
    let handle = app_ref.open_search_session(Nip50SearchSession::new(request, &session_id));
    if let Ok(mut handles) = handles().lock() {
        handles.insert((app as usize, session_id), handle);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_search_close(app: *mut NmpApp, session_id: *const c_char) {
    let Some(app_ref) = app_ref(app) else { return };
    let Some(session_id) = c_string_nonempty(session_id) else {
        return;
    };
    let removed = handles()
        .lock()
        .ok()
        .and_then(|mut handles| handles.remove(&(app as usize, session_id.clone())));
    let handle = removed.unwrap_or_else(|| Nip50SearchHandle::for_key(session_id));
    app_ref.close_search_session(&handle);
}

#[no_mangle]
pub extern "C" fn nmp_app_search_snapshot(
    app: *mut NmpApp,
    session_id: *const c_char,
    out_buf: *mut u8,
    cap: usize,
) -> i32 {
    let Some(app_ref) = app_ref(app) else {
        return 0;
    };
    let Some(session_id) = c_string_nonempty(session_id) else {
        return 0;
    };
    let handle = handles()
        .lock()
        .ok()
        .and_then(|handles| handles.get(&(app as usize, session_id.clone())).cloned())
        .unwrap_or_else(|| Nip50SearchHandle::for_key(session_id));
    let Some(bytes) = app_ref.search_session_snapshot_bytes(&handle) else {
        return 0;
    };
    if !out_buf.is_null() && cap >= bytes.len() {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, bytes.len());
        }
    }
    i32::try_from(bytes.len()).unwrap_or(i32::MAX)
}

fn handles() -> &'static Mutex<HashMap<(usize, String), Nip50SearchHandle>> {
    SEARCH_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}
