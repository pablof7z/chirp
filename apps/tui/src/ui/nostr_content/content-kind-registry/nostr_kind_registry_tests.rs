use std::cell::RefCell;
use std::sync::Arc;

use nmp_content::{ContentTreeWire, ShortNoteProjection, UnknownProjection};
use nmp_core::display::short_npub;
use ratatui::{buffer::Buffer, layout::Rect};

use super::super::content_render_data::ContentProfileRenderData;
use super::super::nostr_mention_chip::NostrMentionProfileHost;
use super::kind_renderer::{author_byline, KindRenderer};
use super::nostr_kind_registry::NostrKindRegistry;

struct HeightRenderer(u16);

impl KindRenderer for HeightRenderer {
    fn render(
        &self,
        _projection: &nmp_content::embed_projection::EmbedKindProjection,
        _ctx: &nmp_content::RenderContext,
        _registry: &NostrKindRegistry,
        _host: Option<&dyn NostrMentionProfileHost>,
        _consumer_id: Option<&str>,
        _area: Rect,
        _buf: &mut Buffer,
    ) {
    }

    fn preferred_height(
        &self,
        _projection: &nmp_content::embed_projection::EmbedKindProjection,
        _width: u16,
    ) -> u16 {
        self.0
    }
}

#[test]
fn unknown_kind_specific_renderer_overrides_fallback() {
    let mut registry = NostrKindRegistry::make_default();
    registry.register_unknown(30_402, Arc::new(HeightRenderer(7)));

    let projection = unknown_projection(30_402);
    assert_eq!(
        registry
            .resolve(&projection)
            .preferred_height(&projection, 80),
        7
    );
}

#[test]
fn unknown_kind_without_registration_uses_fallback() {
    let registry = NostrKindRegistry::make_default();
    let projection = unknown_projection(39_000);

    assert!(
        registry
            .resolve(&projection)
            .preferred_height(&projection, 80)
            >= 2
    );
}

fn unknown_projection(kind: u32) -> nmp_content::embed_projection::EmbedKindProjection {
    nmp_content::embed_projection::EmbedKindProjection::Unknown(UnknownProjection {
        kind,
        author_pubkey: "a".repeat(64),
        author_display_name: None,
        author_picture_url: None,
        created_at: 0,
        content: "hello".to_string(),
        content_tree: ContentTreeWire::default(),
        tags: Vec::new(),
        alt_text: None,
    })
}

const SHOWCASE_PUBKEY: &str = "fa984bd7dbb282f07e16e7ae87b26a2a7b9b90b7246a44771f0cf5ae58018f52";

/// Fake host that records claims and returns a known live-resolved name.
struct FakeAuthorHost {
    display: Option<String>,
    claimed: RefCell<Vec<(String, String)>>,
}

impl NostrMentionProfileHost for FakeAuthorHost {
    fn resolve_ref(&self, pubkey: &str, consumer_id: &str) {
        self.claimed
            .borrow_mut()
            .push((pubkey.to_string(), consumer_id.to_string()));
    }

    fn profile_for_pubkey(&self, pubkey: &str) -> Option<ContentProfileRenderData> {
        Some(ContentProfileRenderData {
            pubkey: pubkey.to_string(),
            display_name: self.display.clone(),
            npub: None,
            picture_url: None,
        })
    }
}

fn buffer_text(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            if let Some(cell) = buf.cell((x, y)) {
                out.push_str(cell.symbol());
            }
        }
        out.push('\n');
    }
    out
}

#[test]
fn author_byline_claims_and_reads_live_name() {
    let host = FakeAuthorHost {
        display: Some("pablof7z".to_string()),
        claimed: RefCell::new(Vec::new()),
    };

    let byline = author_byline(Some(&host), Some("content-kind-registry"), SHOWCASE_PUBKEY);

    assert_eq!(byline, "pablof7z");
    assert_eq!(
        host.claimed.borrow().as_slice(),
        [(
            SHOWCASE_PUBKEY.to_string(),
            "content-kind-registry".to_string()
        )]
    );
}

#[test]
fn author_byline_falls_back_to_npub_short_not_hex() {
    let host = FakeAuthorHost {
        display: None,
        claimed: RefCell::new(Vec::new()),
    };

    let byline = author_byline(Some(&host), Some("content-kind-registry"), SHOWCASE_PUBKEY);

    let expected = short_npub(SHOWCASE_PUBKEY);
    assert_eq!(byline, expected);
    assert!(byline.starts_with("npub1"), "{byline}");
    assert!(
        !byline.starts_with(&SHOWCASE_PUBKEY[..8]),
        "byline must not be a hex prefix: {byline}"
    );
    assert_eq!(host.claimed.borrow().len(), 1);
}

#[test]
fn author_byline_without_host_uses_npub_short() {
    let byline = author_byline(None, None, SHOWCASE_PUBKEY);
    assert_eq!(byline, short_npub(SHOWCASE_PUBKEY));
}

#[test]
fn short_note_renderer_paints_live_resolved_byline() {
    let host = FakeAuthorHost {
        display: Some("pablof7z".to_string()),
        claimed: RefCell::new(Vec::new()),
    };
    let projection =
        nmp_content::embed_projection::EmbedKindProjection::ShortNote(ShortNoteProjection {
            id: "b".repeat(64),
            author_pubkey: SHOWCASE_PUBKEY.to_string(),
            author_display_name: Some("STATIC-SHOULD-NOT-SHOW".to_string()),
            author_picture_url: None,
            created_at: 0,
            content_tree: ContentTreeWire::default(),
            media_urls: Vec::new(),
        });

    let area = Rect::new(0, 0, 40, 6);
    let mut buf = Buffer::empty(area);
    let registry = NostrKindRegistry::make_default();
    let ctx = nmp_content::RenderContext::new();
    registry.resolve(&projection).render(
        &projection,
        &ctx,
        &registry,
        Some(&host),
        Some("content-kind-registry"),
        area,
        &mut buf,
    );

    let text = buffer_text(&buf);
    assert!(text.contains("pablof7z"), "{text}");
    assert!(!text.contains("STATIC-SHOULD-NOT-SHOW"), "{text}");
    assert_eq!(host.claimed.borrow().len(), 1);
}

#[test]
fn embedded_event_forwards_author_host_to_renderer() {
    use nmp_content::embed_projection::{
        EmbedKindProjection, EmbeddedEventEnvelope, RenderContextWire,
    };
    use nmp_content::RenderContext;
    use ratatui::widgets::Widget;

    use super::EmbeddedEvent;

    let host = FakeAuthorHost {
        display: Some("pablof7z".to_string()),
        claimed: RefCell::new(Vec::new()),
    };
    let envelope = EmbeddedEventEnvelope {
        uri: "nostr:nevent1example".to_string(),
        primary_id: "b".repeat(64),
        render_context: RenderContextWire::from(&RenderContext::new()),
        projection: EmbedKindProjection::ShortNote(ShortNoteProjection {
            id: "b".repeat(64),
            author_pubkey: SHOWCASE_PUBKEY.to_string(),
            author_display_name: Some("STATIC-SHOULD-NOT-SHOW".to_string()),
            author_picture_url: None,
            created_at: 0,
            content_tree: ContentTreeWire::default(),
            media_urls: Vec::new(),
        }),
        collapsed: false,
        collapse_reason: None,
    };

    let area = Rect::new(0, 0, 48, 8);
    let mut buf = Buffer::empty(area);
    let registry = NostrKindRegistry::make_default();
    EmbeddedEvent::new(&envelope, &registry)
        .author_host(Some(&host), Some("content-kind-registry"))
        .render(area, &mut buf);

    let text = buffer_text(&buf);
    assert!(text.contains("pablof7z"), "{text}");
    assert!(!text.contains("STATIC-SHOULD-NOT-SHOW"), "{text}");
    assert_eq!(host.claimed.borrow().len(), 1);
}
