use std::ffi::c_char;

use nmp_core::{EventShape, ProfileShape, RefLiveness, RefNamespace, RefResolveMetadata, RefShape};
use nmp_native_runtime::NmpApp;
use nmp_nostr_id::{encode_nprofile, encode_npub, NprofileData};

use crate::ffi::helpers::c_string_opt;

use super::{app_ref, into_raw_string};

struct DecodedRefArgs {
    namespace: RefNamespace,
    key: String,
    consumer_id: String,
    shape: RefShape,
    liveness: RefLiveness,
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_resolve_ref(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: i32,
    liveness: i32,
) {
    let (Some(app), Some(args)) = (
        app_ref(app),
        decode_ref_args(namespace, key, consumer_id, shape, liveness),
    ) else {
        return;
    };
    app.resolve_ref(
        args.namespace,
        args.key,
        args.consumer_id,
        args.shape,
        args.liveness,
    );
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_resolve_ref_with_metadata(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: i32,
    liveness: i32,
    metadata_json: *const c_char,
) {
    let (Some(app), Some(args)) = (
        app_ref(app),
        decode_ref_args(namespace, key, consumer_id, shape, liveness),
    ) else {
        return;
    };
    app.resolve_ref_with_metadata(
        args.namespace,
        args.key,
        args.consumer_id,
        args.shape,
        args.liveness,
        parse_ref_metadata(metadata_json),
    );
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_release_ref(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    let (Some(app), Some(namespace), Some(key), Some(consumer_id)) = (
        app_ref(app),
        decode_namespace(namespace),
        c_string_opt(key),
        c_string_opt(consumer_id),
    ) else {
        return;
    };
    app.release_ref(namespace, key, consumer_id);
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_profile_ref(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    resolve_profile(
        app,
        key,
        consumer_id,
        ProfileShape::Ref,
        RefLiveness::CacheOk,
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_profile_card_live(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    resolve_profile(app, key, consumer_id, ProfileShape::Card, RefLiveness::Live);
}

#[no_mangle]
pub extern "C" fn nmp_app_release_profile_ref(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    nmp_app_release_ref(app, 0, key, consumer_id);
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_event_embed(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    resolve_event(
        app,
        key,
        consumer_id,
        EventShape::Embed,
        RefLiveness::CacheOk,
        None,
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_event_embed_live(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    resolve_event(
        app,
        key,
        consumer_id,
        EventShape::Embed,
        RefLiveness::Live,
        None,
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_event_embed_with_metadata(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
    metadata_json: *const c_char,
) {
    resolve_event(
        app,
        key,
        consumer_id,
        EventShape::Embed,
        RefLiveness::CacheOk,
        Some(parse_ref_metadata(metadata_json)),
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_event_embed_live_with_metadata(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
    metadata_json: *const c_char,
) {
    resolve_event(
        app,
        key,
        consumer_id,
        EventShape::Embed,
        RefLiveness::Live,
        Some(parse_ref_metadata(metadata_json)),
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_release_event_ref(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    nmp_app_release_ref(app, 1, key, consumer_id);
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_app_encode_profile(
    app: *mut NmpApp,
    pubkey_hex: *const c_char,
) -> *mut c_char {
    let Some(pubkey_hex) = c_string_opt(pubkey_hex) else {
        return std::ptr::null_mut();
    };
    let relays = app_ref(app)
        .and_then(|app| app.mailbox_cache_reader())
        .and_then(|cache| cache.write_relays(&pubkey_hex))
        .filter(|relays| !relays.is_empty());
    let encoded = match relays {
        Some(mut relays) => {
            relays.truncate(3);
            encode_nprofile(&NprofileData {
                pubkey: pubkey_hex.clone(),
                relays,
            })
            .unwrap_or(pubkey_hex)
        }
        None => encode_npub(&pubkey_hex).unwrap_or(pubkey_hex),
    };
    into_raw_string(encoded)
}

fn decode_namespace(namespace: i32) -> Option<RefNamespace> {
    match namespace {
        0 => Some(RefNamespace::Profile),
        1 => Some(RefNamespace::Event),
        _ => None,
    }
}

fn decode_shape(shape: i32) -> Option<RefShape> {
    match shape {
        0 => Some(RefShape::Profile(ProfileShape::Ref)),
        1 => Some(RefShape::Profile(ProfileShape::Card)),
        2 => Some(RefShape::Event(EventShape::Embed)),
        3 => Some(RefShape::Event(EventShape::Raw)),
        _ => None,
    }
}

fn decode_ref_args(
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: i32,
    liveness: i32,
) -> Option<DecodedRefArgs> {
    let namespace = decode_namespace(namespace)?;
    let shape = decode_shape(shape)?;
    if shape.namespace() != namespace {
        return None;
    }
    Some(DecodedRefArgs {
        namespace,
        key: c_string_opt(key)?,
        consumer_id: c_string_opt(consumer_id)?,
        shape,
        liveness: RefLiveness::from_ffi(liveness),
    })
}

fn parse_ref_metadata(ptr: *const c_char) -> RefResolveMetadata {
    let Some(raw) = c_string_opt(ptr) else {
        return RefResolveMetadata::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return RefResolveMetadata::default();
    };
    let hints = value
        .get("hints")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_string)
        .collect();
    let event_author = value
        .get("event_author")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    RefResolveMetadata {
        hints,
        event_author,
    }
}

fn resolve_profile(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: ProfileShape,
    liveness: RefLiveness,
) {
    let (Some(app), Some(key), Some(consumer_id)) =
        (app_ref(app), c_string_opt(key), c_string_opt(consumer_id))
    else {
        return;
    };
    app.resolve_ref(
        RefNamespace::Profile,
        key,
        consumer_id,
        RefShape::Profile(shape),
        liveness,
    );
}

fn resolve_event(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: EventShape,
    liveness: RefLiveness,
    metadata: Option<RefResolveMetadata>,
) {
    let (Some(app), Some(key), Some(consumer_id)) =
        (app_ref(app), c_string_opt(key), c_string_opt(consumer_id))
    else {
        return;
    };
    let shape = RefShape::Event(shape);
    match metadata {
        Some(metadata) => app.resolve_ref_with_metadata(
            RefNamespace::Event,
            key,
            consumer_id,
            shape,
            liveness,
            metadata,
        ),
        None => app.resolve_ref(RefNamespace::Event, key, consumer_id, shape, liveness),
    }
}
