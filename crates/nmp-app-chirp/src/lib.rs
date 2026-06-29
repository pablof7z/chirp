//! `nmp-app-chirp` — Chirp per-app glue.
//!
//! Composes `nmp-core` (the kernel substrate + observed-projection sink slot)
//! with `nmp-nip01` (the NIP-10 modular timeline view) and `nmp-threading`
//! (the agnostic grouping algorithm) to surface Twitter-style stacked-reply
//! modules over the kernel's home timeline. Lives outside `nmp-core` because
//! ADR-0009 forbids `nmp-core -> nmp-nip01` (cycle).
//!
//! ## Wiring
//!
//! The iOS shell links this one aggregate static library for Chirp. Keeping
//! `nmp-native-runtime`, the NIP-46 broker adapter, and the Chirp projection in one Rust
//! archive gives the process exactly one copy of the native C-ABI state.
//!
//! The shell calls [`nmp_signer_broker_init`] once after `nmp_app_new` and
//! checks for `NmpConfigStatus::Ok`, then calls [`ffi::nmp_app_chirp_register`].
//! The projection registration:
//!
//! 1. Builds a reusable `nmp_nip01::ModularTimelineProjection` with the
//!    viewer's pubkey and the default `ModulePolicy`.
//! 2. Opens declared observed projections for the feed's concrete interest
//!    shapes. From that moment on, matching accepted events are replayed and then
//!    delivered through scoped future delivery.
//! 3. Returns an opaque handle the shell keeps for snapshots / unregister.
//!
//! The home feed is registered as the standard `"nmp.feed.home"` projection.
//! Render shells consume it from the normal NMP update stream and report
//! viewport intent through generic feed FFI; cursor math, page size, cap, and
//! quoted-card closure remain in reusable NMP crates.
//!
//! ## Doctrine
//!
//! * **D0** — kernel emits, this crate composes. No business logic in
//!   Swift; the grouping algorithm is in `nmp-threading`.
//! * **D6** — runtime FFI symbols degrade silently on null pointers, lock
//!   poisoning, or serialization failure; init-only config symbols return
//!   explicit status codes for ordering errors.

pub mod action_specs;
// ADR-0064 / Cut-B caller slice (#1756) — the typed byte-doorway dispatch seam
// shared by all three in-repo Chirp Rust shells (this crate's `ChirpClient`,
// chirp-tui, chirp-desktop). Owns the namespace→typed-payload encoder, the
// host correlation-id mint, and the envelope + `nmp_app_dispatch_action_bytes`
// call. The retired JSON `nmp_app_dispatch_action` doorway has no caller here.
pub mod dispatch_bytes;
pub mod ffi;
pub mod native_ffi;
pub mod snapshot_types;
pub mod typed_api;
#[cfg(feature = "wallet")]
mod wallet_runtime;
mod zap_identifier;

pub use ffi::{nmp_app_chirp_register, nmp_app_chirp_unregister, ChirpHandle, NmpRegisterStatus};
// ADR-0053 / Workstream-E4 — Chirp's projection-consumption intent: the single
// C-ABI call every Chirp shell (iOS, Android, tui, desktop) uses to declare it
// (`consume_all` — Chirp is a full client). No hand-maintained key list.
pub use ffi::nmp_app_chirp_declare_consumed_projections;
// #1493 — Chirp-owned create-account wrapper that injects Chirp's product seed
// follows (`nmp_chirp_config::chirp_default_follows`). The generic
// `nmp_app_create_new_account` auto-follows nobody; the Swift/Kotlin shells call
// THIS symbol so the seed pubkeys never transit the thin shell.
pub use ffi::nmp_app_chirp_create_new_account;
// The Rust-native `(namespace, body_json)` action builders backing
// `crate::typed_api::ChirpClient` (chirp-tui / chirp-desktop). Social writes on
// the iOS/Android shells ride the generated `GeneratedActionBuilders` byte
// builders straight to the byte doorway; the `ChirpActionIntent` JSON intent
// lane has been retired (M14-1 PR2 / #2145).
pub use action_specs::{
    follow_spec, publish_note_spec, publish_profile_spec, react_spec, repost_spec, send_dm_spec,
    unfollow_spec, zap_identifier_spec, zap_spec, TypedActionSpec,
};
pub use dispatch_bytes::{dispatch_action_bytes_for, mint_correlation_id, parse_dispatch_envelope};
pub use zap_identifier::{ZapIdentifierInput, ZAP_IDENTIFIER_NAMESPACE};
// The raw `(namespace, body_json)` byte doorway for direct-dispatch sites
// (NIP-29 group ops, #2170). M14-1 / #2145.
pub use ffi::nmp_app_chirp_dispatch_action_bytes;
pub use ffi::{
    nmp_app_chirp_close_group_discovery,
    nmp_app_chirp_close_tag_feed,
    nmp_app_chirp_open_group_discovery,
    nmp_app_chirp_open_tag_feed,
    // #1740 step 7 — the ONE public app-facing feed doorway.
    nmp_app_close_feed,
    nmp_app_open_feed,
    GroupFeedHandle,
};
pub use native_ffi::{
    nmp_app_ack_action_stage, nmp_app_add_relay, nmp_app_cancel_action,
    nmp_app_cancel_bunker_handshake, nmp_app_close_interest, nmp_app_configure,
    nmp_app_consume_all_builtin_projections, nmp_app_create_new_account, nmp_app_debug_info,
    nmp_app_declare_consumed_projections, nmp_app_declare_incremental_apply,
    nmp_app_dispatch_action_bytes, nmp_app_dispatch_capability, nmp_app_encode_profile,
    nmp_app_free, nmp_app_intent_classify, nmp_app_intent_dispatch, nmp_app_is_alive,
    nmp_app_lifecycle_background, nmp_app_lifecycle_foreground, nmp_app_load_older_feed,
    nmp_app_new, nmp_app_nostrconnect_uri, nmp_app_open_interest, nmp_app_open_uri,
    nmp_app_register_action_result_observer, nmp_app_register_agent_nsec,
    nmp_app_release_event_ref, nmp_app_release_profile_ref, nmp_app_release_ref,
    nmp_app_remove_account, nmp_app_remove_relay, nmp_app_reset, nmp_app_resolve_event_embed,
    nmp_app_resolve_event_embed_live, nmp_app_resolve_event_embed_live_with_metadata,
    nmp_app_resolve_event_embed_with_metadata, nmp_app_resolve_profile_card_live,
    nmp_app_resolve_profile_ref, nmp_app_resolve_ref, nmp_app_resolve_ref_with_metadata,
    nmp_app_retry_publish, nmp_app_search_close, nmp_app_search_open, nmp_app_search_snapshot,
    nmp_app_set_capability_callback, nmp_app_set_lifecycle_callback, nmp_app_set_storage_path,
    nmp_app_set_update_callback, nmp_app_sign_event_for_return, nmp_app_signin_bunker,
    nmp_app_signin_nsec, nmp_app_start, nmp_app_stop, nmp_app_switch_active,
    nmp_content_tokenize_text, nmp_free_string, nmp_mirror_free_bytes, nmp_mirror_pull_page,
    nmp_nip21_decode_uri, nmp_signer_broker_init, NmpMirrorBytes,
};
#[cfg(feature = "android-ffi")]
pub use native_ffi::{
    nmp_app_deliver_external_signer_response, nmp_app_signin_nip55, nmp_external_signer_init,
};
pub use nmp_nip01::{
    Nip10ReplyAttribution as ChirpReplyAttribution, TimelineEventCard as ChirpEventCard,
};
pub use snapshot_types::{
    ActionResult, ActionStageRow, InterestRow, ProfileCard, RelayRow, RelayWireSubRow,
    RuntimeMetrics,
};
pub use typed_api::{
    follow_action, publish_note_action, publish_profile_action, react_action, repost_action,
    send_dm_action, unfollow_action, zap_action, ChirpClient,
};

/// V-80 rung 7 / issue #1613 — the home-feed snapshot served under `"nmp.feed.home"`.
///
/// Was `nmp_nip01::ModularTimelineSnapshot` (`{ blocks, cards, … }`); now the
/// OP-centric [`nmp_feed::RootFeedSnapshot`] instantiated with the NIP-10
/// render card ([`ChirpEventCard`]) and the NIP-10 reply attribution
/// ([`ChirpReplyAttribution`]). Wire shape:
/// `{ "cards": [{ "card": ChirpEventCard, "attribution": [ChirpReplyAttribution] }], "page": …, "metrics": … }`.
///
/// Named `OpFeedSnapshot` to match the Swift type and the framework type
/// `nmp_nip01::op_feed::OpFeedSnapshot` — no app name in a framework type alias.
pub type OpFeedSnapshot = nmp_feed::RootFeedSnapshot<ChirpEventCard, ChirpReplyAttribution>;

// ── Marmot (MLS encrypted groups) projection ─────────────────────────────
//
// The current NMP migration deleted the prior reusable C shell before exposing
// the replacement active-registration seam required by ADR-0025. Chirp keeps
// these exported names fail-closed so the migration validates the architecture
// gap instead of copying Marmot internals here. Upstream blocker:
// pablof7z/nostr-multi-platform#2495.
#[cfg(feature = "marmot")]
pub use native_ffi::{
    nmp_app_chirp_identity_remove_account, nmp_app_chirp_identity_restore,
    nmp_app_chirp_identity_sign_in_nsec,
};
#[cfg(feature = "marmot")]
pub use native_ffi::{
    nmp_marmot_register_active, nmp_marmot_unregister, KeyPackageStatus, MarmotGroupRow,
    MarmotHandle, MarmotMessageRow, MarmotProjection, MarmotSnapshot, PendingWelcomeRow,
};
