use std::ffi::c_char;

use nmp_core::{EventShape, ProfileShape, RefLiveness, RefNamespace, RefResolveMetadata, RefShape};
use nmp_native_runtime::NmpApp;

use super::{app_ref, c_string_nonempty, c_string_opt};

#[no_mangle]
pub extern "C" fn nmp_app_resolve_ref(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: i32,
    liveness: i32,
) {
    resolve_ref(app, namespace, key, consumer_id, shape, liveness, None);
}

#[no_mangle]
pub extern "C" fn nmp_app_resolve_ref_with_metadata(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: i32,
    liveness: i32,
    metadata_json: *const c_char,
) {
    resolve_ref(
        app,
        namespace,
        key,
        consumer_id,
        shape,
        liveness,
        parse_metadata(metadata_json),
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_release_ref(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    let Some(app) = app_ref(app) else { return };
    let (Some(namespace), Some(key), Some(consumer_id)) = (
        namespace_from_ffi(namespace),
        c_string_nonempty(key),
        c_string_nonempty(consumer_id),
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
    release_typed(app, RefNamespace::Profile, key, consumer_id);
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
        parse_metadata(metadata_json),
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
        parse_metadata(metadata_json),
    );
}

#[no_mangle]
pub extern "C" fn nmp_app_release_event_ref(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    release_typed(app, RefNamespace::Event, key, consumer_id);
}

fn resolve_ref(
    app: *mut NmpApp,
    namespace: i32,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: i32,
    liveness: i32,
    metadata: Option<RefResolveMetadata>,
) {
    let Some(app) = app_ref(app) else { return };
    let (Some(namespace), Some(key), Some(consumer_id)) = (
        namespace_from_ffi(namespace),
        c_string_nonempty(key),
        c_string_nonempty(consumer_id),
    ) else {
        return;
    };
    let Some(shape) = shape_from_ffi(namespace, shape) else {
        return;
    };
    let liveness = RefLiveness::from_ffi(liveness);
    if let Some(metadata) = metadata {
        app.resolve_ref_with_metadata(namespace, key, consumer_id, shape, liveness, metadata);
    } else {
        app.resolve_ref(namespace, key, consumer_id, shape, liveness);
    }
}

fn resolve_profile(
    app: *mut NmpApp,
    key: *const c_char,
    consumer_id: *const c_char,
    shape: ProfileShape,
    liveness: RefLiveness,
) {
    let Some(app) = app_ref(app) else { return };
    let (Some(key), Some(consumer_id)) = (c_string_nonempty(key), c_string_nonempty(consumer_id))
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
    let Some(app) = app_ref(app) else { return };
    let (Some(key), Some(consumer_id)) = (c_string_nonempty(key), c_string_nonempty(consumer_id))
    else {
        return;
    };
    let shape = RefShape::Event(shape);
    if let Some(metadata) = metadata {
        app.resolve_ref_with_metadata(
            RefNamespace::Event,
            key,
            consumer_id,
            shape,
            liveness,
            metadata,
        );
    } else {
        app.resolve_ref(RefNamespace::Event, key, consumer_id, shape, liveness);
    }
}

fn release_typed(
    app: *mut NmpApp,
    namespace: RefNamespace,
    key: *const c_char,
    consumer_id: *const c_char,
) {
    let Some(app) = app_ref(app) else { return };
    if let (Some(key), Some(consumer_id)) = (c_string_nonempty(key), c_string_nonempty(consumer_id))
    {
        app.release_ref(namespace, key, consumer_id);
    }
}

fn namespace_from_ffi(namespace: i32) -> Option<RefNamespace> {
    match namespace {
        0 => Some(RefNamespace::Profile),
        1 => Some(RefNamespace::Event),
        _ => None,
    }
}

fn shape_from_ffi(namespace: RefNamespace, shape: i32) -> Option<RefShape> {
    match (namespace, shape) {
        (RefNamespace::Profile, 0) => Some(RefShape::Profile(ProfileShape::Ref)),
        (RefNamespace::Profile, 1) => Some(RefShape::Profile(ProfileShape::Card)),
        (RefNamespace::Event, 0) => Some(RefShape::Event(EventShape::Embed)),
        (RefNamespace::Event, 1) => Some(RefShape::Event(EventShape::Raw)),
        _ => None,
    }
}

fn parse_metadata(ptr: *const c_char) -> Option<RefResolveMetadata> {
    let text = c_string_opt(ptr)?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let hints = value
        .get("hints")
        .and_then(serde_json::Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let event_author = value
        .get("event_author")
        .or_else(|| value.get("author"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Some(RefResolveMetadata {
        hints,
        event_author,
    })
}
