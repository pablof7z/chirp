use std::ffi::{c_char, c_void};
use std::ptr;

use nmp_native_runtime::NmpApp;

use super::{app_ref, c_string_nonempty};

const BLOCKED: &str =
    "Marmot is disabled: NMP issue #2495 must restore the current active-registration seam";

#[repr(C)]
pub struct MarmotHandle {
    _private: u8,
}

impl MarmotHandle {
    #[must_use]
    pub fn dispatch(&self, _action: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "ok": false, "error": BLOCKED })
    }

    #[must_use]
    pub fn snapshot_rust(&self) -> serde_json::Value {
        serde_json::json!({ "groups": [], "pending_welcomes": [], "key_package": null, "last_error": BLOCKED })
    }
}

#[no_mangle]
pub extern "C" fn nmp_marmot_register_active(
    _app: *mut NmpApp,
    _db_dir: *const c_char,
    _keyring_service_id: *const c_char,
) -> *mut MarmotHandle {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn nmp_marmot_unregister(handle: *mut MarmotHandle) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_chirp_identity_restore(
    app: *mut NmpApp,
    _db_dir: *const c_char,
    test_nsec: *const c_char,
) -> *mut MarmotHandle {
    if let Some(app) = app_ref(app) {
        if let Some(secret) = c_string_nonempty(test_nsec) {
            app.add_signer(
                nmp_core::SignerSource::LocalNsec(zeroize::Zeroizing::new(secret)),
                true,
            );
        }
    }
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn nmp_app_chirp_identity_sign_in_nsec(
    app: *mut NmpApp,
    secret: *const c_char,
    _db_dir: *const c_char,
) -> *mut MarmotHandle {
    if let Some(app) = app_ref(app) {
        if let Some(secret) = c_string_nonempty(secret) {
            app.add_signer(
                nmp_core::SignerSource::LocalNsec(zeroize::Zeroizing::new(secret)),
                true,
            );
        }
    }
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn nmp_app_chirp_identity_remove_account(
    app: *mut NmpApp,
    identity_id: *const c_char,
) {
    if let Some(app) = app_ref(app) {
        if let Some(identity_id) = c_string_nonempty(identity_id) {
            app.remove_account(identity_id);
        }
    }
}

#[allow(dead_code)]
pub type MarmotProjection = serde_json::Value;
#[allow(dead_code)]
pub type MarmotSnapshot = serde_json::Value;
#[allow(dead_code)]
pub type MarmotGroupRow = serde_json::Value;
#[allow(dead_code)]
pub type MarmotMessageRow = serde_json::Value;
#[allow(dead_code)]
pub type PendingWelcomeRow = serde_json::Value;
#[allow(dead_code)]
pub type KeyPackageStatus = serde_json::Value;

#[allow(dead_code)]
pub extern "C" fn nmp_marmot_blocked_reason(_: *mut c_void) -> *const c_char {
    c"Marmot disabled by NMP issue #2495".as_ptr()
}
