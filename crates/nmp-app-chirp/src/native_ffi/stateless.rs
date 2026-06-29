use std::ffi::c_char;

use nmp_content::{tokenize, tokenize_with_kind, RenderMode};
use nmp_core::nip19::{self, NaddrData, NeventData, Nip19Entity, NprofileData};
use nmp_core::nip21::{self, Nip21Error, NostrUri};
use nmp_core::substrate::InputIntentRequest;
use nmp_native_runtime::{InputIntentDispatch, NmpApp};

use super::{app_ref, c_string_nonempty, c_string_opt, into_c_string, json_error};

const MAX_NPROFILE_RELAYS: usize = 3;

#[no_mangle]
pub extern "C" fn nmp_app_encode_profile(
    app: *mut NmpApp,
    pubkey_hex: *const c_char,
) -> *mut c_char {
    let Some(pubkey_hex) = c_string_opt(pubkey_hex) else {
        return into_c_string("");
    };
    let relays = app_ref(app)
        .and_then(NmpApp::mailbox_cache_reader)
        .and_then(|cache| cache.write_relays(&pubkey_hex))
        .filter(|relays| !relays.is_empty());
    if let Some(mut relays) = relays {
        relays.truncate(MAX_NPROFILE_RELAYS);
        let data = NprofileData {
            pubkey: pubkey_hex.clone(),
            relays,
        };
        return into_c_string(nip19::encode_nprofile(&data).unwrap_or(pubkey_hex));
    }
    into_c_string(nip19::encode_npub(&pubkey_hex).unwrap_or(pubkey_hex))
}

#[no_mangle]
pub extern "C" fn nmp_nip21_decode_uri(input: *const c_char) -> *mut c_char {
    let Some(input) = c_string_nonempty(input) else {
        return decode_error("unparseable");
    };
    match decode_uri(&input) {
        Ok(value) => into_c_string(value.to_string()),
        Err(error) => decode_error(error),
    }
}

#[no_mangle]
pub extern "C" fn nmp_content_tokenize_text(
    content: *const c_char,
    tags_json: *const c_char,
    mode: i32,
    kind: u32,
) -> *mut c_char {
    let Some(content) = c_string_opt(content) else {
        return json_error("missing_content");
    };
    let tags = c_string_opt(tags_json)
        .and_then(|json| serde_json::from_str::<Vec<Vec<String>>>(&json).ok())
        .unwrap_or_default();
    let render_mode = match mode {
        1 => RenderMode::Markdown,
        2 => RenderMode::Auto,
        _ => RenderMode::Plain,
    };
    let tree = if render_mode == RenderMode::Auto {
        tokenize_with_kind(&content, &tags, render_mode, kind)
    } else {
        tokenize(&content, &tags, render_mode)
    }
    .to_wire();
    into_c_string(serde_json::json!({ "ok": true, "tree": tree }).to_string())
}

#[no_mangle]
pub extern "C" fn nmp_app_intent_classify(
    app: *mut NmpApp,
    request_json: *const c_char,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return json_error("null_app");
    };
    let Some(request) = parse_intent_request(request_json) else {
        return json_error("bad_request");
    };
    into_c_string(
        serde_json::json!({
            "ok": true,
            "classification": app.classify_input_intent(&request),
        })
        .to_string(),
    )
}

#[no_mangle]
pub extern "C" fn nmp_app_intent_dispatch(
    app: *mut NmpApp,
    request_json: *const c_char,
    session_id: *const c_char,
) -> *mut c_char {
    let Some(app) = app_ref(app) else {
        return json_error("null_app");
    };
    let Some(request) = parse_intent_request(request_json) else {
        return json_error("bad_request");
    };
    let session_id = c_string_opt(session_id);
    let value = match app.dispatch_input_intent(&request, session_id.as_deref()) {
        InputIntentDispatch::Dispatched(candidate) => {
            serde_json::json!({ "ok": true, "dispatched": candidate })
        }
        InputIntentDispatch::Rejection(rejection) => {
            serde_json::json!({ "ok": true, "rejection": rejection })
        }
    };
    into_c_string(value.to_string())
}

fn parse_intent_request(ptr: *const c_char) -> Option<InputIntentRequest> {
    let json = c_string_opt(ptr)?;
    serde_json::from_str(&json).ok()
}

fn decode_uri(input: &str) -> Result<serde_json::Value, &'static str> {
    let target = if input.starts_with("nostr:") {
        nip21::parse_nostr_uri(input).map_err(map_nip21_error)?
    } else {
        let entity = nip19::parse(input).map_err(|_| "unparseable")?;
        target_from_entity(entity)?
    };
    Ok(target_json(target))
}

fn target_from_entity(entity: Nip19Entity) -> Result<NostrUri, &'static str> {
    match entity {
        Nip19Entity::Nsec(_) => Err("nsec-forbidden"),
        Nip19Entity::Npub(pubkey) => Ok(NostrUri::Profile {
            pubkey,
            relays: Vec::new(),
        }),
        Nip19Entity::Nprofile(NprofileData { pubkey, relays }) => {
            Ok(NostrUri::Profile { pubkey, relays })
        }
        Nip19Entity::Note(event_id) => Ok(NostrUri::Event {
            event_id,
            relays: Vec::new(),
            author: None,
            kind: None,
        }),
        Nip19Entity::Nevent(NeventData {
            event_id,
            relays,
            author,
            kind,
        }) => Ok(NostrUri::Event {
            event_id,
            relays,
            author,
            kind,
        }),
        Nip19Entity::Naddr(NaddrData {
            identifier,
            pubkey,
            kind,
            relays,
        }) => Ok(NostrUri::Address {
            identifier,
            pubkey,
            kind,
            relays,
        }),
    }
}

fn target_json(target: NostrUri) -> serde_json::Value {
    match target {
        NostrUri::Profile { pubkey, relays } => {
            serde_json::json!({ "ok": true, "target": "profile", "pubkey": pubkey, "relays": relays })
        }
        NostrUri::Event {
            event_id,
            relays,
            author,
            kind,
        } => serde_json::json!({
            "ok": true,
            "target": "event",
            "event_id": event_id,
            "relays": relays,
            "author": author,
            "kind": kind,
        }),
        NostrUri::Address {
            identifier,
            pubkey,
            kind,
            relays,
        } => serde_json::json!({
            "ok": true,
            "target": "address",
            "identifier": identifier,
            "pubkey": pubkey,
            "kind": kind,
            "relays": relays,
        }),
    }
}

fn map_nip21_error(error: Nip21Error) -> &'static str {
    match error {
        Nip21Error::NsecForbidden => "nsec-forbidden",
        Nip21Error::MissingScheme | Nip21Error::Nip19(_) => "unparseable",
    }
}

fn decode_error(error: &'static str) -> *mut c_char {
    into_c_string(serde_json::json!({ "ok": false, "error": error }).to_string())
}
