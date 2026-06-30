use std::ffi::c_char;

use nmp_nostr_id::{self as nip19, NaddrData, NeventData, Nip19Entity, NprofileData};
use nmp_nostr_id::{nip21, Nip21Error, NostrUri};

use crate::ffi::helpers::c_string_opt;

use super::into_raw_string;

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn nmp_nip21_decode_uri(input: *const c_char) -> *mut c_char {
    let Some(input) = c_string_opt(input) else {
        return into_raw_string(error_json("null-input"));
    };
    into_raw_string(decode_uri_json(&input))
}

fn decode_uri_json(input: &str) -> String {
    match decode_uri(input) {
        Ok(target) => target.to_json(),
        Err(error) => error_json(error.code()),
    }
}

fn decode_uri(input: &str) -> Result<DecodedTarget, DecodeError> {
    if input.starts_with("nostr:") {
        return nip21::parse_nostr_uri(input)
            .map(DecodedTarget::from)
            .map_err(DecodeError::from);
    }
    nip19::parse(input)
        .map_err(|_| DecodeError::Unparseable)
        .and_then(DecodedTarget::try_from)
}

enum DecodedTarget {
    Profile {
        pubkey: String,
        relays: Vec<String>,
    },
    Event {
        event_id: String,
        relays: Vec<String>,
        author: Option<String>,
        kind: Option<u32>,
    },
    Address {
        identifier: String,
        pubkey: String,
        kind: u32,
        relays: Vec<String>,
    },
}

impl DecodedTarget {
    fn to_json(self) -> String {
        match self {
            Self::Profile { pubkey, relays } => serde_json::json!({
                "ok": true,
                "target": "profile",
                "pubkey": pubkey,
                "relays": relays,
            })
            .to_string(),
            Self::Event {
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
            })
            .to_string(),
            Self::Address {
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
            })
            .to_string(),
        }
    }
}

impl From<NostrUri> for DecodedTarget {
    fn from(target: NostrUri) -> Self {
        match target {
            NostrUri::Profile { pubkey, relays } => Self::Profile { pubkey, relays },
            NostrUri::Event {
                event_id,
                relays,
                author,
                kind,
            } => Self::Event {
                event_id,
                relays,
                author,
                kind,
            },
            NostrUri::Address {
                identifier,
                pubkey,
                kind,
                relays,
            } => Self::Address {
                identifier,
                pubkey,
                kind,
                relays,
            },
        }
    }
}

impl TryFrom<Nip19Entity> for DecodedTarget {
    type Error = DecodeError;

    fn try_from(entity: Nip19Entity) -> Result<Self, Self::Error> {
        match entity {
            Nip19Entity::Nsec(_) => Err(DecodeError::NsecForbidden),
            Nip19Entity::Npub(pubkey) => Ok(Self::Profile {
                pubkey,
                relays: Vec::new(),
            }),
            Nip19Entity::Nprofile(NprofileData { pubkey, relays }) => {
                Ok(Self::Profile { pubkey, relays })
            }
            Nip19Entity::Note(event_id) => Ok(Self::Event {
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
            }) => Ok(Self::Event {
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
            }) => Ok(Self::Address {
                identifier,
                pubkey,
                kind,
                relays,
            }),
        }
    }
}

enum DecodeError {
    NsecForbidden,
    Unparseable,
}

impl DecodeError {
    fn code(&self) -> &'static str {
        match self {
            Self::NsecForbidden => "nsec-forbidden",
            Self::Unparseable => "unparseable",
        }
    }
}

impl From<Nip21Error> for DecodeError {
    fn from(error: Nip21Error) -> Self {
        match error {
            Nip21Error::NsecForbidden => Self::NsecForbidden,
            Nip21Error::MissingScheme | Nip21Error::Nip19(_) => Self::Unparseable,
        }
    }
}

fn error_json(error: &str) -> String {
    serde_json::json!({ "ok": false, "error": error }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nmp_nostr_id::{
        encode_naddr, encode_nevent, encode_nprofile, encode_nsec, NaddrData, NeventData,
        NprofileData,
    };

    const PUBKEY: &str = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
    const EVENT_ID: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    #[test]
    fn bare_nprofile_decodes_profile_relays() {
        let input = encode_nprofile(&NprofileData {
            pubkey: PUBKEY.to_string(),
            relays: vec!["wss://relay.example".to_string()],
        })
        .unwrap();

        let value: serde_json::Value = serde_json::from_str(&decode_uri_json(&input)).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["target"], "profile");
        assert_eq!(value["pubkey"], PUBKEY);
        assert_eq!(value["relays"][0], "wss://relay.example");
    }

    #[test]
    fn nostr_nevent_decodes_event_metadata() {
        let input = encode_nevent(&NeventData {
            event_id: EVENT_ID.to_string(),
            relays: vec!["wss://relay.example".to_string()],
            author: Some(PUBKEY.to_string()),
            kind: Some(1),
        })
        .unwrap();

        let value: serde_json::Value =
            serde_json::from_str(&decode_uri_json(&format!("nostr:{input}"))).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["target"], "event");
        assert_eq!(value["event_id"], EVENT_ID);
        assert_eq!(value["author"], PUBKEY);
        assert_eq!(value["kind"], 1);
        assert_eq!(value["relays"][0], "wss://relay.example");
    }

    #[test]
    fn naddr_decodes_address_coordinate_fields() {
        let input = encode_naddr(&NaddrData {
            identifier: "root".to_string(),
            pubkey: PUBKEY.to_string(),
            kind: 30023,
            relays: vec!["wss://relay.example".to_string()],
        })
        .unwrap();

        let value: serde_json::Value = serde_json::from_str(&decode_uri_json(&input)).unwrap();

        assert_eq!(value["ok"], true);
        assert_eq!(value["target"], "address");
        assert_eq!(value["identifier"], "root");
        assert_eq!(value["pubkey"], PUBKEY);
        assert_eq!(value["kind"], 30023);
        assert_eq!(value["relays"][0], "wss://relay.example");
    }

    #[test]
    fn nsec_is_rejected_without_echoing_secret() {
        let input = encode_nsec(PUBKEY).unwrap();

        let value: serde_json::Value = serde_json::from_str(&decode_uri_json(&input)).unwrap();

        assert_eq!(value["ok"], false);
        assert_eq!(value["error"], "nsec-forbidden");
        assert!(!value.to_string().contains(&input));
        assert!(!value.to_string().contains(PUBKEY));
    }
}
