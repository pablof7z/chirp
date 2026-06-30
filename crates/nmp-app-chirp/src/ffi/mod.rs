//! Chirp per-app FFI surface.
//!
//! `extern "C"` symbols Swift links against:
//!
//! - [`nmp_app_chirp_register`] — wire the Chirp modular projections and return
//!   an opaque handle for later snapshots / unregister.
//! - [`nmp_app_open_feed`] / [`nmp_app_close_feed`] — the single typed feed
//!   session doorway for home, author, thread, and other declared feeds.
//! - [`nmp_app_chirp_register_group_events`] /
//!   [`nmp_app_chirp_unregister_group_events`] — open/close a NIP-29 group-chat
//!   read view (`"nmp.nip29.group_events"`, `NGEV`). The view is now a HYDRATING
//!   observed interest (#2088): a screen opened after the group's events were
//!   cached catches up on the cached tail. Singleton; `register` replaces a
//!   prior view, `unregister` tears it down on screen dismissal.
//! - [`nmp_app_chirp_open_group_discovery`] /
//!   [`nmp_app_chirp_close_group_discovery`] — open/close lifecycle for the
//!   NIP-29 group-discovery session (`"nmp.nip29.discovered_groups"`, `NDGS`),
//!   also hydrating (#2088). Returns a heap-owned `GroupFeedHandle` that MUST be
//!   freed via `close`; `close` detaches the interest, unregisters the observer,
//!   and removes the snapshot projection.
//! - [`nmp_app_chirp_register_dm_inbox`] — host entry point for the NIP-17 DM
//!   runtime. `nmp_app_chirp_register` wires it eagerly: a kind:1059
//!   `IngestParser` (slot `"nip17.dm_inbox"`, fires on live ingest and
//!   cache-served replay), a `"nmp.nip17.dm_inbox"` snapshot projection, and a
//!   Rust-owned controller for the active gift-wrap interest + kind:10050
//!   relay-list publish.
//! - [`nmp_app_chirp_unregister`] — free the handle. Idempotent. (The engine /
//!   follow-set observer registrations are torn down by `nmp_app_free`.)
//! - [`nmp_app_chirp_seed_default_relays`] /
//!   [`nmp_app_chirp_seed_relays_from_json`] — Chirp relay-bootstrap seeding.
//!   Wraps `nmp_chirp_config::chirp_default_relay_bootstrap` so the relay
//!   default set has ONE source of truth and the Swift/Kotlin shells never
//!   hardcode relay URLs (D7). iOS analogue of the Android
//!   `nmp-chirp-android-ffi::relay_seeding` glue.
//!
//! ## Doctrine
//!
//! * **D0** — `nmp-core` never depends on `nmp-nip01`; this crate is the
//!   composition point. ADR-0009 (kernel boundary).
//! * **D6** — every entry point is fire-and-forget. Null pointers, missing
//!   strings, serialization failures, and poisoned mutexes all degrade
//!   silently rather than raising across the FFI.
//! * **No business logic in Swift** — Swift takes the JSON string, decodes
//!   the `RootFeedSnapshot` (`[{ card, attribution }]`), and renders. All
//!   root-indexing / attribution happens here / in `nmp-feed` + `nmp-nip01`.
//!
//! ## Module layout
//!
//! This module is split across several sub-modules to keep each file under
//! the V-09 500-LOC hand-authored ceiling. The split is purely organizational —
//! every `pub extern "C"` symbol Swift links against is re-exported below.

mod actions;
mod create_account;
mod declared_projections;
mod feed;
mod group;
mod handle;
mod helpers;
mod register;
mod relay_seeding;
mod runtime_ffi;
mod snapshot;
mod tag_feed;
mod typed_actions;

#[cfg(test)]
mod tests;

pub use create_account::nmp_app_chirp_create_new_account;
pub use declared_projections::nmp_app_chirp_declare_consumed_projections;
pub use nmp_native_runtime::{NmpApp, NmpConfigStatus};
// #1740 step 7 — the ONE public app-facing feed doorway (typed params in,
// opaque handle out). Replaces per-feed-type opens with a single generic entry.
pub use feed::{nmp_app_close_feed, nmp_app_open_feed};
pub use group::{
    nmp_app_chirp_close_group_discovery, nmp_app_chirp_open_group_discovery,
    nmp_app_chirp_register_group_events, nmp_app_chirp_unregister_group_events, GroupFeedHandle,
};
pub use handle::ChirpHandle;
pub use register::{
    nmp_app_chirp_register, nmp_app_chirp_register_dm_inbox, nmp_app_chirp_register_follow_list,
    NmpRegisterStatus,
};
pub use relay_seeding::{nmp_app_chirp_seed_default_relays, nmp_app_chirp_seed_relays_from_json};
pub use runtime_ffi::*;
pub use snapshot::nmp_app_chirp_unregister;
pub use tag_feed::{nmp_app_chirp_close_tag_feed, nmp_app_chirp_open_tag_feed};
pub use typed_actions::nmp_app_chirp_dispatch_action_bytes;
