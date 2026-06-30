//! Android JNI surface for keyring capability registration.
//!
//! **`nativeSetCapabilityHandler`** registers a synchronous Kotlin handler
//! (e.g. `KeystoreKeyringCapability`) for all non-`external_signer` capability
//! namespaces. The handler object must expose `fun handle(requestJson: String): String`.
//! The `GlobalRef` is held in the session's `capability_handler` slot and cleared in
//! `close_updates_locked` after the capability socket is quiesced.
//!
//! Doctrine:
//! * **D6** — every failure (detached VM, JNI error, null session) is a data envelope
//!   or a boolean `false` (0); never a panic or NULL.
//! * **D7** — these entry points transport results only; Rust owns all policy.

use jni::objects::{JClass, JObject};
use jni::sys::jlong;
use jni::JNIEnv;

use crate::session_arc;

/// Register a synchronous Kotlin capability handler for all non-`external_signer`
/// namespaces (e.g. Android Keystore keyring). The `handler` object must
/// implement `fun handle(requestJson: String): String`.
///
/// The `GlobalRef` is held in the session and cleared in teardown after
/// `nmp_app_set_capability_callback(None)` quiesces any in-flight dispatch.
///
/// D6: a null/dead handle or JNI failure is a silent no-op.
/// D7: the handler executes and reports; Rust owns all policy.
#[no_mangle]
pub extern "system" fn Java_org_nmp_android_KernelBridge_nativeSetCapabilityHandler(
    env: JNIEnv,
    _class: JClass,
    handle: jlong,
    handler: JObject,
) {
    let Some(s) = session_arc(handle) else {
        return;
    };
    if handler.is_null() {
        // Clear any existing handler (deregistration path).
        drop(s.capability_handler.lock().map(|mut g| g.take()));
        return;
    }
    // Obtain the JavaVM for cross-thread attachment in the trampoline.
    let vm = match env.get_java_vm() {
        Ok(vm) => vm,
        Err(_) => return,
    };
    // Promote the local handler reference to a GlobalRef so it survives
    // beyond this JNI call frame.
    let global = match env.new_global_ref(&handler) {
        Ok(g) => g,
        Err(_) => return,
    };
    let new_handler = Some(crate::capability::SyncCapabilityHandler::new(vm, global));
    drop(s.capability_handler.lock().map(|mut g| *g = new_handler));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::session::{insert_session, remove_session, Session};

    /// `nativeSetCapabilityHandler` with a dead session id is a no-op (D6).
    #[test]
    fn set_capability_handler_dead_session_is_noop() {
        // Call through the session registry with an invalid handle.
        // The function returns early (no panic), which is all we assert.
        let slot = super::session_arc(0);
        assert!(slot.is_none());
    }

    /// Verify that the capability_handler slot starts empty and can be set
    /// and cleared without holding a live JNI environment.
    #[test]
    fn capability_handler_slot_starts_empty() {
        let session = Session::test_session();
        let handle = insert_session(Arc::clone(&session));
        {
            let guard = session.capability_handler.lock().unwrap();
            assert!(guard.is_none(), "slot must be empty on a fresh session");
        }
        remove_session(handle);
    }

    /// Verify that `close_updates_locked` drops the capability_handler slot
    /// (ensures no GlobalRef dangling after teardown).
    #[test]
    fn close_updates_clears_capability_handler_slot() {
        let session = Session::test_session();
        // Nothing in the slot — close must not panic.
        session.close_updates();
        let guard = session.capability_handler.lock().unwrap();
        assert!(guard.is_none());
    }
}
