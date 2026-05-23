//! Mutable context threaded through body emission.
//!
//! Tracks per-document side-state that the OOXML body needs to
//! reference but the document.xml.rels + media parts have to
//! produce: embedded images, external hyperlinks, footnotes,
//! bookmarks, etc. The body emitter populates the registries
//! while it walks the AST; the packager reads them when assembling
//! `word/_rels/document.xml.rels`, `word/media/imageN.*`, and the
//! header/footer/footnote parts.
//!
//! Threading this through the emit functions (rather than a
//! thread-local or a `RefCell`) keeps every body emitter pure on
//! its inputs, makes tests easy (just construct an `EmitCtx`), and
//! avoids the "where does this number come from?" debugging
//! pattern.

use std::path::{Path, PathBuf};

use stem_core::ast::Block;

/// One image embedded in the docx. Populated by the drawing
/// emitter; consumed by the packager.
#[derive(Clone)]
pub struct EmbeddedImage {
    /// Relationship ID used in `<a:blip r:embed="rIdN"/>`.
    pub rid: String,
    /// Path inside the ZIP, e.g. `word/media/image1.png`.
    pub zip_path: String,
    /// Image bytes — written to the ZIP verbatim.
    pub bytes: Vec<u8>,
    /// File extension used in Content_Types and the part path
    /// (`"png"`, `"jpeg"`, `"gif"`).
    pub ext: String,
}

/// One external hyperlink. Internal (anchor) hyperlinks don't
/// need a rel — they live in document.xml as `<w:hyperlink w:anchor="…">`.
#[derive(Clone)]
pub struct ExternalLink {
    pub rid: String,
    pub url: String,
}

#[derive(Default)]
pub struct EmitCtx {
    /// Directory used to resolve relative `image[src:...]` paths.
    /// `None` means resolve against the process CWD.
    pub image_base: Option<PathBuf>,
    /// Embedded images, in the order they were emitted.
    pub images: Vec<EmbeddedImage>,
    /// Monotonic counter — every kind of rId draws from this
    /// space so a single `document.xml.rels` can list them all
    /// without collisions. Static parts (styles, numbering,
    /// theme, ...) reserve the first N entries; this counter
    /// starts after that reservation.
    next_rid: u32,
    /// Number used for the next `wp:docPr` / `pic:cNvPr` id.
    /// Independent from rIds.
    next_drawing_id: u32,
    /// Caption counters — `SEQ Table` and `SEQ Figure` instances
    /// emit the pre-computed number into the cached field result
    /// so the document reads correctly before the user presses F9.
    pub table_caption_seq: u32,
    pub figure_caption_seq: u32,
    /// External hyperlinks. Each needs an Hyperlink relationship
    /// in `document.xml.rels` with `TargetMode="External"`.
    pub hyperlinks: Vec<ExternalLink>,
    /// Monotonic bookmark id counter. Word requires unique ids
    /// per `<w:bookmarkStart>` and the matching `<w:bookmarkEnd>`.
    next_bookmark_id: u32,
    /// Pre-collected heading anchors in document order — populated
    /// by [`super::prepass::collect`] before the emission walk so
    /// the TOC field can render with the full set of entries even
    /// when it sits at the top of the document.
    pub heading_anchors: Vec<HeadingAnchor>,
    /// Cursor into `heading_anchors`. Advanced once per emitted
    /// heading paragraph; the bookmark name comes from
    /// `heading_anchors[heading_cursor].bookmark`.
    pub heading_cursor: usize,
    /// Pre-collected caption anchors (tables + figures) in
    /// document order. LoT/LoF emission walks this vector.
    pub captions: Vec<CaptionAnchor>,
    /// Collected `header{...}` block bodies in source order. Each
    /// becomes its own `word/headerN.xml` part, with a
    /// `<w:headerReference w:type="…"/>` in `<w:sectPr>` using
    /// the matching scope from `header_scopes`.
    pub headers: Vec<Vec<Block>>,
    /// Per-header scope (default / first / even) from the source.
    pub header_scopes: Vec<HeaderFooterScope>,
    /// Same for `footer{...}` blocks.
    pub footers: Vec<Vec<Block>>,
    pub footer_scopes: Vec<HeaderFooterScope>,
    /// rId allocated for `headers[i]`'s relationship — populated
    /// just before body emission so `<w:headerReference r:id="…"/>`
    /// in sectPr can name it.
    pub header_rids: Vec<String>,
    pub footer_rids: Vec<String>,
    /// Footnote contents captured during body emission. Each
    /// entry's `id` matches the `<w:footnoteReference w:id="…"/>`
    /// emitted in the body; the packager renders them into
    /// `word/footnotes.xml`.
    pub footnotes: Vec<FootnoteEntry>,
    /// Next footnote id. Word reserves -1 (separator) and 0
    /// (continuation separator), so user notes start at 1.
    next_footnote_id: u32,
    /// Source-supplied style overrides — populated by the prepass
    /// from top-level `style[id:..., ...]` blocks and applied when
    /// the styles part is rendered.
    pub style_overrides: Vec<StyleOverride>,
}

/// One footnote — the `id` is referenced by both the
/// `<w:footnoteReference>` in the body and the matching
/// `<w:footnote w:id="…">` block in the footnotes part. The
/// `text` is the flattened content; rich runs are out of scope for
/// task 14 (footnotes carrying inline-formatted runs need the
/// same render_body machinery, deferred until needed).
#[derive(Clone)]
pub struct FootnoteEntry {
    pub id: u32,
    pub text: String,
}

/// Bookmark + display text for one heading. Populated by the
/// prepass; consumed by heading emission and the TOC field
/// builder.
#[derive(Clone)]
pub struct HeadingAnchor {
    /// `_Toc<n>` — anchor name a `PAGEREF` can resolve.
    pub bookmark: String,
    /// Heading level 1..6.
    pub level: u32,
    /// Visible text after inline elements are flattened.
    pub text: String,
}

/// One-style override patch from the source. Top-level
/// `style[id:..., ...]` blocks become `StyleOverride` entries that
/// patch the matching `<w:style>` in `word/styles.xml`. Every
/// attribute is `Option`-wrapped so an unset field means "inherit
/// the docx default for this style"; a set field replaces it.
#[derive(Clone, Default)]
pub struct StyleOverride {
    pub id: String,
    // pPr overrides
    pub before_dxa: Option<u32>,
    pub after_dxa: Option<u32>,
    pub line_dxa: Option<u32>,
    pub align: Option<String>,
    pub keep_next: Option<bool>,
    pub keep_lines: Option<bool>,
    pub outline_lvl: Option<u32>,
    pub contextual_spacing: Option<bool>,
    pub border_top: Option<bool>,
    // rPr overrides
    pub size_hp: Option<u32>,
    pub color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub strike: Option<bool>,
    pub underline: Option<String>,
    pub font: Option<String>,
}

/// Bookmark + display text for one caption (table or figure).
/// Drives LoT/LoF entries and PAGEREF lookup.
#[derive(Clone)]
pub struct CaptionAnchor {
    pub kind: CaptionKind,
    pub bookmark: String,
    pub text: String,
    /// 1-based sequence index within its kind.
    pub seq: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CaptionKind {
    Table,
    Figure,
}

/// Which pages a header/footer applies to. Maps directly to the
/// `w:type` attribute on `<w:headerReference>` / `<w:footerReference>`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeaderFooterScope {
    /// All pages without a more specific override.
    Default,
    /// First page of the section. Requires `<w:titlePg/>` in
    /// sectPr.
    First,
    /// Even-numbered pages. Requires `<w:evenAndOddHeaders/>` in
    /// settings.xml.
    Even,
}

impl HeaderFooterScope {
    pub fn from_prop(s: Option<&str>) -> Self {
        match s {
            Some("first") => Self::First,
            Some("even") => Self::Even,
            _ => Self::Default,
        }
    }
    pub fn w_type(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::First => "first",
            Self::Even => "even",
        }
    }
}

impl EmitCtx {
    /// Create a new context whose first allocated rId will be
    /// `rId{start_rid}`. The packager reserves the static-parts
    /// rIds before calling into body emission, so this starts
    /// counting at the next free slot.
    pub fn new(image_base: Option<&Path>, start_rid: u32) -> Self {
        Self {
            image_base: image_base.map(Path::to_path_buf),
            images: Vec::new(),
            next_rid: start_rid,
            next_drawing_id: 1,
            table_caption_seq: 0,
            figure_caption_seq: 0,
            hyperlinks: Vec::new(),
            next_bookmark_id: 1,
            heading_anchors: Vec::new(),
            heading_cursor: 0,
            captions: Vec::new(),
            headers: Vec::new(),
            header_scopes: Vec::new(),
            footers: Vec::new(),
            footer_scopes: Vec::new(),
            header_rids: Vec::new(),
            footer_rids: Vec::new(),
            footnotes: Vec::new(),
            next_footnote_id: 1,
            style_overrides: Vec::new(),
        }
    }

    /// Register a footnote — returns the id to embed in
    /// `<w:footnoteReference w:id="…"/>`.
    pub fn add_footnote(&mut self, text: String) -> u32 {
        let id = self.next_footnote_id;
        self.next_footnote_id = id + 1;
        self.footnotes.push(FootnoteEntry { id, text });
        id
    }

    /// Register an external hyperlink target; returns its rId.
    /// De-duplicates by URL so a document linking the same URL
    /// multiple times only consumes one rel.
    pub fn add_external_link(&mut self, url: &str) -> String {
        if let Some(existing) = self.hyperlinks.iter().find(|h| h.url == url) {
            return existing.rid.clone();
        }
        let rid = self.alloc_rid();
        self.hyperlinks.push(ExternalLink {
            rid: rid.clone(),
            url: url.to_string(),
        });
        rid
    }

    pub fn alloc_bookmark_id(&mut self) -> u32 {
        let id = self.next_bookmark_id;
        self.next_bookmark_id = id + 1;
        id
    }

    /// Allocate the next `rIdN` string.
    pub fn alloc_rid(&mut self) -> String {
        let n = self.next_rid;
        self.next_rid = n + 1;
        format!("rId{n}")
    }

    /// Allocate the next drawing id (wp:docPr / pic:cNvPr).
    pub fn alloc_drawing_id(&mut self) -> u32 {
        let id = self.next_drawing_id;
        self.next_drawing_id = id + 1;
        id
    }

    /// Register an embedded image; returns its allocated rId.
    pub fn add_image(&mut self, bytes: Vec<u8>, ext: &str) -> String {
        let n = self.images.len() + 1;
        let rid = self.alloc_rid();
        let zip_path = format!("word/media/image{n}.{ext}");
        self.images.push(EmbeddedImage {
            rid: rid.clone(),
            zip_path,
            bytes,
            ext: ext.to_string(),
        });
        rid
    }
}
