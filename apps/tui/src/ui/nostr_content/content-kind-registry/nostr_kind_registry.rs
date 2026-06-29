//! NostrKindRegistry for the TUI (F-CR-06).
//!
//! Single source of truth for kind → renderer dispatch in the terminal.

use std::collections::HashMap;
use std::sync::Arc;

use nmp_content::embed_projection::EmbedKindProjection;
use nmp_core::display::short_npub;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Widget, Wrap};

use super::super::nostr_mention_chip::NostrMentionProfileHost;
use super::kind_renderer::{author_byline, KindRenderer, KindRendererRef};
use super::text_helpers::{
    estimate_reading_time, format_relative_time, format_short_date, render_two_line, text_height,
    tree_text, truncate_chars,
};

/// The registry consulted by `EmbeddedEvent` (and by `NostrContentView`).
pub struct NostrKindRegistry {
    short_note: Option<KindRendererRef>,
    article: Option<KindRendererRef>,
    highlight: Option<KindRendererRef>,
    profile: Option<KindRendererRef>,
    unknown_by_kind: HashMap<u32, KindRendererRef>,
    fallback: KindRendererRef,
}

impl NostrKindRegistry {
    pub fn new(fallback: KindRendererRef) -> Self {
        Self {
            short_note: None,
            article: None,
            highlight: None,
            profile: None,
            unknown_by_kind: HashMap::new(),
            fallback,
        }
    }

    /// Installs the built-in default renderer for each known projection variant,
    /// plus `DefaultUnknownRenderer` as the fallback for unregistered numeric kinds.
    /// Replace any slot with `set_*` to swap in a richer handler (e.g. F-CR-09).
    pub fn make_default() -> Self {
        let mut reg = Self::new(Arc::new(DefaultUnknownRenderer));
        reg.short_note = Some(Arc::new(DefaultShortNoteRenderer));
        reg.article = Some(Arc::new(DefaultArticleRenderer));
        reg.highlight = Some(Arc::new(DefaultHighlightRenderer));
        reg.profile = Some(Arc::new(DefaultProfileRenderer));
        reg
    }

    pub fn set_short_note(&mut self, r: KindRendererRef) {
        self.short_note = Some(r);
    }

    pub fn set_article(&mut self, r: KindRendererRef) {
        self.article = Some(r);
    }

    pub fn set_highlight(&mut self, r: KindRendererRef) {
        self.highlight = Some(r);
    }

    pub fn set_profile(&mut self, r: KindRendererRef) {
        self.profile = Some(r);
    }

    pub fn register_unknown(&mut self, kind: u32, r: KindRendererRef) {
        self.unknown_by_kind.insert(kind, r);
    }

    pub fn resolve(&self, projection: &EmbedKindProjection) -> &dyn KindRenderer {
        match projection {
            EmbedKindProjection::ShortNote(_) => {
                self.short_note.as_deref().unwrap_or(self.fallback.as_ref())
            }
            EmbedKindProjection::Article(_) => {
                self.article.as_deref().unwrap_or(self.fallback.as_ref())
            }
            EmbedKindProjection::Highlight(_) => {
                self.highlight.as_deref().unwrap_or(self.fallback.as_ref())
            }
            EmbedKindProjection::Profile(_) => {
                self.profile.as_deref().unwrap_or(self.fallback.as_ref())
            }
            EmbedKindProjection::Unknown(p) => self
                .unknown_by_kind
                .get(&p.kind)
                .map(|r| r.as_ref())
                .unwrap_or(self.fallback.as_ref()),
        }
    }
}

/// Default renderer for `ShortNoteProjection` (kind:1 quoted notes).
/// Renders in a rounded box matching `DefaultArticleRenderer`, with author
/// byline and relative timestamp.
pub struct DefaultShortNoteRenderer;

impl KindRenderer for DefaultShortNoteRenderer {
    fn render(
        &self,
        projection: &EmbedKindProjection,
        _ctx: &nmp_content::context::RenderContext,
        _registry: &NostrKindRegistry,
        host: Option<&dyn NostrMentionProfileHost>,
        consumer_id: Option<&str>,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) {
        let EmbedKindProjection::ShortNote(note) = projection else {
            return;
        };
        if area.height < 4 || area.width < 6 {
            return;
        }

        // Component-owned kind:0: this byline claims the author's profile and
        // reads the live-resolved name, instead of painting the static
        // `author_display_name` projection field (mirrors iOS PR #833).
        let author = author_byline(host, consumer_id, &note.author_pubkey);
        let body = tree_text(&note.content_tree);
        let rel_time = format_relative_time(note.created_at);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(71, 85, 105)));
        let inner = block.inner(area);
        block.render(area, buf);

        let content = Rect {
            x: inner.x + 1,
            y: inner.y,
            width: inner.width.saturating_sub(1),
            height: inner.height,
        };
        if content.width == 0 || content.height == 0 {
            return;
        }

        let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).split(content);

        // Byline: ● author · relative_time
        Paragraph::new(Line::from(vec![
            Span::styled("\u{25CF} ", Style::default().fg(Color::Rgb(220, 38, 38))),
            Span::styled(author, Style::default().fg(Color::Rgb(203, 213, 225))),
            Span::styled(
                format!(" \u{00B7} {}", rel_time),
                Style::default().fg(Color::Rgb(100, 116, 139)),
            ),
        ]))
        .render(rows[0], buf);

        // Body
        Paragraph::new(Line::from(Span::styled(
            body,
            Style::default().fg(Color::Rgb(148, 163, 184)),
        )))
        .wrap(Wrap { trim: true })
        .render(rows[1], buf);
    }

    fn preferred_height(&self, projection: &EmbedKindProjection, width: u16) -> u16 {
        let EmbedKindProjection::ShortNote(note) = projection else {
            return 4;
        };
        let wrap_width = width.saturating_sub(3).max(1);
        text_height(&tree_text(&note.content_tree), wrap_width)
            .saturating_add(1) // byline
            .saturating_add(2) // top + bottom borders
            .max(4)
    }
}

/// Default renderer for `ArticleProjection` (kind:30023).
/// Continuous-byline card: rounded box, bold title, `● author · date · N min read`, summary.
pub struct DefaultArticleRenderer;

impl KindRenderer for DefaultArticleRenderer {
    fn render(
        &self,
        projection: &EmbedKindProjection,
        _ctx: &nmp_content::context::RenderContext,
        _registry: &NostrKindRegistry,
        host: Option<&dyn NostrMentionProfileHost>,
        consumer_id: Option<&str>,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) {
        let EmbedKindProjection::Article(article) = projection else {
            return;
        };
        if area.height < 5 || area.width < 6 {
            return;
        }

        // Component-owned kind:0: self-claiming author byline (iOS PR #833).
        let author = author_byline(host, consumer_id, &article.author_pubkey);
        let title = article.title.as_deref().unwrap_or("article");
        let summary = article
            .summary
            .as_deref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| tree_text(&article.content_tree));
        let short_date = format_short_date(article.created_at);
        let reading_min = estimate_reading_time(title, &summary);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(71, 85, 105)));
        let inner = block.inner(area);
        block.render(area, buf);

        let content = Rect {
            x: inner.x + 1,
            y: inner.y,
            width: inner.width.saturating_sub(1),
            height: inner.height,
        };
        if content.width == 0 || content.height == 0 {
            return;
        }

        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(content);

        // Title
        let title_str = truncate_chars(title, content.width as usize);
        Paragraph::new(Line::from(Span::styled(
            title_str,
            Style::default()
                .fg(Color::Rgb(241, 245, 249))
                .add_modifier(Modifier::BOLD),
        )))
        .render(rows[0], buf);

        // Byline: ● Author · Date · N min read
        let meta = format!(" \u{00B7} {} \u{00B7} {} min read", short_date, reading_min);
        Paragraph::new(Line::from(vec![
            Span::styled("\u{25CF} ", Style::default().fg(Color::Rgb(220, 38, 38))),
            Span::styled(author, Style::default().fg(Color::Rgb(203, 213, 225))),
            Span::styled(meta, Style::default().fg(Color::Rgb(100, 116, 139))),
        ]))
        .render(rows[1], buf);

        // Summary
        let summary_str = truncate_chars(&summary, content.width as usize);
        Paragraph::new(Line::from(Span::styled(
            summary_str,
            Style::default().fg(Color::Rgb(148, 163, 184)),
        )))
        .render(rows[2], buf);
    }

    fn preferred_height(&self, _projection: &EmbedKindProjection, _width: u16) -> u16 {
        5
    }
}

/// Default renderer for `HighlightProjection` (kind:9802).
/// Shows highlighted text + source. Replace via `registry.set_highlight(...)` for F-CR-10.
pub struct DefaultHighlightRenderer;

impl KindRenderer for DefaultHighlightRenderer {
    fn render(
        &self,
        projection: &EmbedKindProjection,
        _ctx: &nmp_content::context::RenderContext,
        _registry: &NostrKindRegistry,
        host: Option<&dyn NostrMentionProfileHost>,
        consumer_id: Option<&str>,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) {
        let EmbedKindProjection::Highlight(highlight) = projection else {
            return;
        };
        // Component-owned kind:0: self-claiming author byline (iOS PR #833).
        let author = author_byline(host, consumer_id, &highlight.author_pubkey);
        render_two_line(
            &format!("highlight · {author}"),
            &highlight.highlighted_text,
            area,
            buf,
        );
    }

    fn preferred_height(&self, projection: &EmbedKindProjection, width: u16) -> u16 {
        let EmbedKindProjection::Highlight(highlight) = projection else {
            return 2;
        };
        text_height(&highlight.highlighted_text, width)
            .saturating_add(1)
            .max(2)
    }
}

/// Default renderer for `ProfileProjection` (kind:0).
/// Shows display name + about. Replace via `registry.set_profile(...)` for F-CR-11.
pub struct DefaultProfileRenderer;

impl KindRenderer for DefaultProfileRenderer {
    fn render(
        &self,
        projection: &EmbedKindProjection,
        _ctx: &nmp_content::context::RenderContext,
        _registry: &NostrKindRegistry,
        _host: Option<&dyn NostrMentionProfileHost>,
        _consumer_id: Option<&str>,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) {
        let EmbedKindProjection::Profile(profile) = projection else {
            return;
        };
        // The kind:0 is itself the displayed entity here, so its own
        // `display_name` is legitimate profile data — not a separate author
        // claim. Fall back to a Rust-formatted `npub_short`, never raw hex.
        let label = profile
            .display_name
            .clone()
            .unwrap_or_else(|| short_npub(&profile.pubkey));
        let about = profile.about.clone().unwrap_or_default();
        render_two_line("profile", &format!("{label} — {about}"), area, buf);
    }

    fn preferred_height(&self, projection: &EmbedKindProjection, width: u16) -> u16 {
        let EmbedKindProjection::Profile(profile) = projection else {
            return 2;
        };
        let about = profile.about.clone().unwrap_or_default();
        text_height(&about, width).saturating_add(1).max(2)
    }
}

/// Fallback renderer for `EmbedKindProjection::Unknown` — numeric Nostr kinds
/// that have no registered handler. Knows nothing about named variants.
pub struct DefaultUnknownRenderer;

impl KindRenderer for DefaultUnknownRenderer {
    fn render(
        &self,
        projection: &EmbedKindProjection,
        _ctx: &nmp_content::context::RenderContext,
        _registry: &NostrKindRegistry,
        host: Option<&dyn NostrMentionProfileHost>,
        consumer_id: Option<&str>,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
    ) {
        let EmbedKindProjection::Unknown(unknown) = projection else {
            return;
        };
        // Component-owned kind:0: self-claiming author byline (iOS PR #833).
        let author = author_byline(host, consumer_id, &unknown.author_pubkey);
        let body = if unknown.content.is_empty() {
            tree_text(&unknown.content_tree)
        } else {
            unknown.content.clone()
        };
        render_two_line(
            &format!("kind:{} · {author}", unknown.kind),
            &body,
            area,
            buf,
        );
    }

    fn preferred_height(&self, projection: &EmbedKindProjection, width: u16) -> u16 {
        let EmbedKindProjection::Unknown(unknown) = projection else {
            return 2;
        };
        let body = if unknown.content.is_empty() {
            tree_text(&unknown.content_tree)
        } else {
            unknown.content.clone()
        };
        text_height(&body, width).saturating_add(1).max(2)
    }
}
