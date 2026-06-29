//! NIP-29 group-chat and group-discovery FFI registration entry points.
//!
//! Extracted from `ffi/register.rs` to keep each file under the 500-LOC cap
//! (AGENTS.md). Exported from `ffi/mod.rs` alongside the rest of the
//! `pub extern "C"` surface.
//!
//! ## Thin shell over the `nmp-native-runtime` composition root (#2088)
//!
//! These symbols hold ZERO logic: they parse C strings and delegate to the
//! hydrating `NmpApp::open_group_*` / `close_group_*` methods that live in
//! `nmp-native-runtime` (`group_feed.rs`). The composition — register the projection
//! muted, route ingest through `open_observed_interest_pinned` for read-cache
//! replay, record a teardown handle — lives there, not here, because it must
//! name `NmpApp` (the FFI host type `nmp-nip29` may not name, D0).

use std::collections::HashMap;
use std::ffi::c_char;
use std::sync::{Mutex, OnceLock};

use nmp_native_runtime::{
    Nip29GroupDiscoveryHandle, Nip29GroupDiscoverySession, Nip29GroupEventsHandle,
    Nip29GroupEventsSession, NmpApp,
};
use nmp_nip29::group_id::GroupId;
use serde::Deserialize;

use super::helpers::c_string_opt;

static GROUP_EVENTS_HANDLES: OnceLock<Mutex<HashMap<usize, Nip29GroupEventsHandle>>> =
    OnceLock::new();

fn group_events_handles() -> &'static Mutex<HashMap<usize, Nip29GroupEventsHandle>> {
    GROUP_EVENTS_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Chirp-owned opaque C handle for a NIP-29 group-discovery session.
///
/// NMP's current typed read-session handle intentionally carries no app
/// pointer. Chirp's existing C ABI closes discovery with the handle only, so
/// this wrapper records the owning app pointer as FFI delivery bookkeeping and
/// delegates teardown back to `NmpApp`.
pub struct GroupFeedHandle {
    app: *mut NmpApp,
    handle: Nip29GroupDiscoveryHandle,
}

/// The C-ABI request shape for [`nmp_app_chirp_register_group_events`].
///
/// NIP-29 owns only the `["h", local_id]` routing (issue #2187); the CONSUMER
/// declares both the group AND which kinds it wants. `kinds` is **required**:
/// a missing field is a deserialize error (rejected), so an old `GroupId`-only
/// payload cannot silently widen into a broad all-kinds read. An **empty**
/// `kinds` array means "all h-tagged group events".
#[derive(Deserialize)]
struct GroupEventsRequest {
    /// The target group `{host_relay_url, local_id}`.
    group: GroupId,
    /// The consumer's kind selection. Empty = all; missing = invalid.
    kinds: Vec<u32>,
}

/// Open a NIP-29 group-events read view for one group + kind set into `app`.
///
/// This is **pure consumption** — the read-side of a group screen. It
/// constructs a `GroupEventsProjection` scoped to the supplied group and kind
/// set, and routes its ingest through the hydrating observed-interest door
/// ([`NmpApp::open_group_events`]): a screen opened AFTER the group's events
/// were already cached now catches up on the cached tail (#2088), then tails
/// live. Its snapshot surfaces under `"nmp.nip29.group_events"` (`NGEV`).
///
/// `request_json` is a JSON object naming the target group AND the kinds the
/// consumer wants (a chat view passes `[9, 11]`):
///
/// ```json
/// {"group":{"host_relay_url":"wss://groups.example.com","local_id":"room"},"kinds":[9,11]}
/// ```
///
/// **Empty `kinds` = all h-tagged group events. A missing `kinds` field is
/// invalid** and rejected (prevents an old `GroupId`-only payload silently
/// becoming a broad read).
///
/// D6 — fire-and-forget. A null `app`, a null/invalid-UTF-8 `request_json`, or
/// a JSON shape that does not deserialize to a [`GroupEventsRequest`] (including
/// a missing `kinds`) degrades to a silent return — nothing is registered and
/// no error crosses the FFI.
///
/// SCOPE — singleton: a subsequent call replaces the prior group-events view
/// (the prior hydrating session is closed first, leak-free). Because the view
/// now holds a relay interest, the companion
/// [`nmp_app_chirp_unregister_group_events`] tears it down when the screen is
/// dismissed.
///
/// `app` MUST outlive the call (it is only borrowed for its duration).
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_chirp_register_group_events(
    app: *mut NmpApp,
    request_json: *const c_char,
) {
    if app.is_null() {
        return;
    }
    // SAFETY: caller guarantees `app` is a valid pointer from `nmp_app_new`,
    // live for the duration of this call. The borrow is not held past return.
    let app_ref = unsafe { &*app };

    // Reject silently on a missing or malformed request — D6. The JSON must
    // deserialize to `{ group: GroupId, kinds: [u32] }`; a missing `kinds`
    // field is a deserialize error and is rejected.
    let Some(raw) = c_string_opt(request_json) else {
        return;
    };
    let Ok(request) = serde_json::from_str::<GroupEventsRequest>(&raw) else {
        return;
    };

    let Ok(mut handles) = group_events_handles().lock() else {
        return;
    };
    let app_key = app as usize;
    if let Some(handle) = handles.remove(&app_key) {
        app_ref.close_nip29_group_events_session(handle);
    }
    let handle = app_ref.open_nip29_group_events_session(Nip29GroupEventsSession::new(
        request.group,
        request.kinds,
    ));
    handles.insert(app_key, handle);
}

/// Tear down the NIP-29 group-events read view opened by
/// [`nmp_app_chirp_register_group_events`].
///
/// Detaches the relay interest, revokes the observer, and removes the
/// `"nmp.nip29.group_events"` typed snapshot projection so no stale event log is
/// emitted after the screen is dismissed. Idempotent — closing an unopened
/// view is a harmless no-op. D6 — a null `app` is a silent no-op.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_chirp_unregister_group_events(app: *mut NmpApp) {
    if app.is_null() {
        return;
    }
    // SAFETY: caller guarantees `app` is a valid pointer from `nmp_app_new`.
    let app_ref = unsafe { &*app };
    let Ok(mut handles) = group_events_handles().lock() else {
        return;
    };
    if let Some(handle) = handles.remove(&(app as usize)) {
        app_ref.close_nip29_group_events_session(handle);
    }
}

/// Open a NIP-29 group-discovery session for one host relay.
///
/// This is the **read side** of the NIP-29 group-discovery flow. It constructs
/// a `DiscoveredGroupsProjection` scoped to the supplied relay URL and routes
/// its ingest through the hydrating observed-interest door
/// ([`NmpApp::open_group_discovery`]): a discover screen opened AFTER the
/// relay's kind:39000/39001/39002 catalog was cached catches it up (#2088),
/// then tails live. Its snapshot surfaces under
/// `"nmp.nip29.discovered_groups"` (`NDGS`).
///
/// Returns a heap-allocated opaque [`GroupFeedHandle`] the caller MUST free via
/// `nmp_app_chirp_close_group_discovery`. A null `app`, null/non-UTF-8 /
/// empty `host_relay_url` returns NULL (D6).
///
/// `app` MUST outlive the handle. Call `nmp_app_chirp_close_group_discovery`
/// before `nmp_app_free`.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_chirp_open_group_discovery(
    app: *mut NmpApp,
    host_relay_url: *const c_char,
) -> *mut GroupFeedHandle {
    if app.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: caller guarantees `app` is a valid pointer from `nmp_app_new`,
    // live for the duration of this call and the returned handle.
    let app_ref = unsafe { &*app };

    let Some(relay_url) = c_string_opt(host_relay_url).filter(|s| !s.is_empty()) else {
        return std::ptr::null_mut();
    };

    let handle =
        app_ref.open_nip29_group_discovery_session(Nip29GroupDiscoverySession::new(relay_url));
    Box::into_raw(Box::new(GroupFeedHandle { app, handle }))
}

/// Close a NIP-29 group-discovery session opened by
/// `nmp_app_chirp_open_group_discovery`.
///
/// Detaches the relay interest, revokes the observer, and removes the
/// `"nmp.nip29.discovered_groups"` typed snapshot projection so no stale group
/// catalog is emitted after the discover screen is dismissed. The handle memory
/// is reclaimed; the pointer MUST NOT be used after this call.
///
/// D6 — a null `handle` is a silent no-op.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_chirp_close_group_discovery(handle: *mut GroupFeedHandle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: `handle` is a valid pointer returned by
    // `nmp_app_chirp_open_group_discovery` and must not be used after this
    // call. `Box::from_raw` takes ownership; the app it references must still
    // be alive per the documented contract.
    let handle = unsafe { Box::from_raw(handle) };
    if handle.app.is_null() {
        return;
    }
    // SAFETY: `handle.app` is the app pointer supplied to
    // `nmp_app_chirp_open_group_discovery`; callers must close the handle
    // before freeing that app.
    let app_ref = unsafe { &*handle.app };
    app_ref.close_nip29_group_discovery_session(handle.handle);
}
