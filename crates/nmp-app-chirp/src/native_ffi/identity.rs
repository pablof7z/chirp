use std::collections::HashMap;
use std::ffi::c_char;

use nmp_core::SignerSource;
use nmp_native_runtime::NmpApp;
use zeroize::Zeroizing;

use super::{app_ref, c_string_nonempty, c_string_opt, into_c_string};

#[no_mangle]
pub extern "C" fn nmp_app_signin_nsec(app: *mut NmpApp, secret: *const c_char, make_active: u8) {
    let Some(app) = app_ref(app) else { return };
    if let Some(secret) = c_string_nonempty(secret) {
        app.add_signer(
            SignerSource::LocalNsec(Zeroizing::new(secret)),
            make_active != 0,
        );
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_register_agent_nsec(app: *mut NmpApp, secret: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(secret) = c_string_nonempty(secret) {
        app.add_signer(
            SignerSource::AppManagedLocalNsec(Zeroizing::new(secret)),
            false,
        );
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_signin_bunker(app: *mut NmpApp, uri: *const c_char, make_active: u8) {
    let Some(app) = app_ref(app) else { return };
    if let Some(uri) = c_string_nonempty(uri) {
        app.add_signer(SignerSource::BunkerUri(uri), make_active != 0);
    }
}

#[cfg(feature = "android-ffi")]
#[no_mangle]
pub extern "C" fn nmp_external_signer_init(app: *mut NmpApp) {
    let Some(app) = app_ref(app) else { return };
    app.init_external_signer();
}

#[cfg(feature = "android-ffi")]
#[no_mangle]
pub extern "C" fn nmp_app_signin_nip55(app: *mut NmpApp, signer_package: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    app.signin_nip55(c_string_opt(signer_package));
}

#[cfg(feature = "android-ffi")]
#[no_mangle]
pub extern "C" fn nmp_app_deliver_external_signer_response(
    app: *mut NmpApp,
    response_json: *const c_char,
) {
    let Some(app) = app_ref(app) else { return };
    if let Some(response_json) = c_string_nonempty(response_json) {
        app.deliver_external_signer_response(&response_json);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_create_new_account(
    app: *mut NmpApp,
    profile_json: *const c_char,
    relays_json: *const c_char,
    mls: bool,
    make_active: u8,
) {
    let Some(app) = app_ref(app) else { return };
    let profile = parse_profile(profile_json).unwrap_or_default();
    let relays = parse_relays(relays_json).unwrap_or_default();
    app.create_account(profile, relays, Vec::new(), mls, make_active != 0);
}

#[no_mangle]
pub extern "C" fn nmp_app_switch_active(app: *mut NmpApp, identity_id: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(identity_id) = c_string_nonempty(identity_id) {
        app.switch_active(identity_id);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_remove_account(app: *mut NmpApp, identity_id: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(identity_id) = c_string_nonempty(identity_id) {
        app.remove_account(identity_id);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_add_relay(app: *mut NmpApp, url: *const c_char, role: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let (Some(url), Some(role)) = (c_string_nonempty(url), c_string_nonempty(role)) {
        app.add_relay(url, role);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_remove_relay(app: *mut NmpApp, url: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(url) = c_string_nonempty(url) {
        app.remove_relay(url);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_retry_publish(app: *mut NmpApp, handle: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(handle) = c_string_nonempty(handle) {
        app.retry_publish(handle);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_cancel_action(app: *mut NmpApp, correlation_id: *const c_char) {
    let Some(app) = app_ref(app) else { return };
    if let Some(correlation_id) = c_string_nonempty(correlation_id) {
        app.cancel_publish(correlation_id);
    }
}

#[no_mangle]
pub extern "C" fn nmp_app_sign_event_for_return(
    app: *mut NmpApp,
    account_pubkey_hex: *const c_char,
    unsigned_json: *const c_char,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return into_c_string("");
    };
    let Some(unsigned_json) = c_string_nonempty(unsigned_json) else {
        return into_c_string("");
    };
    let correlation_id = crate::mint_correlation_id();
    app.sign_event_for_return(
        c_string_opt(account_pubkey_hex).unwrap_or_default(),
        unsigned_json,
        correlation_id.clone(),
    );
    into_c_string(correlation_id)
}

fn parse_profile(ptr: *const c_char) -> Option<HashMap<String, String>> {
    let json = c_string_opt(ptr)?;
    serde_json::from_str(&json).ok()
}

fn parse_relays(ptr: *const c_char) -> Option<Vec<(String, String)>> {
    let json = c_string_opt(ptr)?;
    serde_json::from_str(&json).ok()
}
