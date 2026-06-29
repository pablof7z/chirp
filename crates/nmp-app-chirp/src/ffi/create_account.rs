//! Chirp-owned account-creation FFI wrapper.
//!
//! The generic `nmp_app_create_new_account` (in `nmp-native-runtime`) auto-follows nobody:
//! which accounts a fresh account follows is operator/product policy, and NMP no
//! longer hardcodes any default follow set (#1493). Chirp owns that policy in
//! `nmp_chirp_config::chirp_default_follows`, and this wrapper threads it into
//! the create-account command via the shared
//! `nmp_native_runtime::create_new_account_with_initial_follows` helper.
//!
//! This is the exact Rust-owned pattern the relay bootstrap already uses
//! (`nmp_app_chirp_seed_default_relays` wraps `chirp_default_relay_bootstrap`):
//! the seed follows never transit the thin Swift/Kotlin shell — the shell calls
//! this Chirp symbol with the SAME `(profile_json, relays_json, mls,
//! make_active)` arguments it passed to the generic symbol, and the follows are
//! injected here in Rust.
//!
//! D6: fire-and-forget. A null app or undecodable JSON degrades to a `false`
//! return (the underlying helper surfaces a toast) rather than raising across
//! the FFI.

use std::collections::HashMap;
use std::ffi::c_char;

use nmp_native_runtime::NmpApp;

use super::helpers::c_string_opt;

/// Create a new Chirp account, auto-following Chirp's product seed set
/// (`nmp_chirp_config::chirp_default_follows`).
///
/// Drop-in replacement for `nmp_app_create_new_account` on the Chirp path —
/// same arguments, but the fresh account is seeded with Chirp's default follows
/// (kind:3 contacts prepopulate + cold-start publish) instead of an empty set.
///
/// Returns `true` when the create-account command was dispatched, `false` on a
/// null app or undecodable `profile_json` / `relays_json`.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_chirp_create_new_account(
    app: *mut NmpApp,
    profile_json: *const c_char,
    relays_json: *const c_char,
    mls: bool,
    make_active: u8,
) -> bool {
    if app.is_null() {
        return false;
    }
    // SAFETY: caller guarantees `app` is a valid pointer from `nmp_app_new`,
    // live for the duration of this call. The borrow is not held past return.
    let app_ref = unsafe { &*app };
    let Some(profile_json) = c_string_opt(profile_json) else {
        return false;
    };
    let Some(relays_json) = c_string_opt(relays_json) else {
        return false;
    };

    let profile: HashMap<String, String> = match serde_json::from_str(&profile_json) {
        Ok(profile) => profile,
        Err(_) => {
            app_ref.show_toast("Failed to decode profile JSON".to_string());
            return false;
        }
    };
    let relays: Vec<(String, String)> = match serde_json::from_str(&relays_json) {
        Ok(relays) => relays,
        Err(_) => {
            app_ref.show_toast("Failed to decode relays JSON".to_string());
            return false;
        }
    };
    let follows = nmp_chirp_config::chirp_default_follows()
        .iter()
        .map(|p| (*p).to_string())
        .collect::<Vec<String>>();
    app_ref.set_pending_mls_autopublish(mls);
    app_ref.create_account(profile, relays, follows, mls, make_active != 0);
    true
}
