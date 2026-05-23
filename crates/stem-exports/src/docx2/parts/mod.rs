//! OOXML parts. Each submodule emits one part (or family of parts)
//! as a `String` ready to be added to the ZIP package.

use std::path::Path;

use stem_core::ast::Document;

use super::emit::ctx::EmitCtx;
use super::package::Package;
use super::DocxV2Error;

/// rId space layout for `document.xml.rels`:
/// rIds 1..STATIC_RID_COUNT inclusive are the static parts
/// (styles, numbering, theme, settings, webSettings, fontTable,
/// footnotes, endnotes). Body emission allocates from
/// `STATIC_RID_COUNT + 1` onward for images, hyperlinks, headers,
/// footers.
const STATIC_RID_COUNT: u32 = 8;

pub mod content_types;
pub mod doc_props;
pub mod document;
pub mod endnotes;
pub mod font_table;
pub mod footnotes;
pub mod header_footer;
pub mod numbering;
pub mod rels;
pub mod settings;
pub mod styles;
pub mod theme;
pub mod web_settings;

mod content_type_names {
    //! Canonical Content_Types `Override` content-type strings.
    pub const STYLES: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
    pub const NUMBERING: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
    pub const SETTINGS: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
    pub const WEB_SETTINGS: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.webSettings+xml";
    pub const FONT_TABLE: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml";
    pub const FOOTNOTES: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
    pub const ENDNOTES: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
    pub const HEADER: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
    pub const FOOTER: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
    pub const THEME: &str =
        "application/vnd.openxmlformats-officedocument.theme+xml";
    pub const DOC_MAIN: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
    pub const CORE: &str =
        "application/vnd.openxmlformats-package.core-properties+xml";
    pub const EXTENDED: &str =
        "application/vnd.openxmlformats-officedocument.extended-properties+xml";
}

/// Build the full `.docx` package for a cooked AST.
///
/// All static parts (theme, settings, webSettings, fontTable,
/// styles, numbering, docProps) are present; the document body is
/// emitted from `doc.blocks` via the paragraph dispatcher in
/// [`super::emit::paragraph`]. `image_base` is used by the drawing
/// emitter to resolve relative `image[src:...]` paths.
pub fn package_doc(doc: &Document, image_base: Option<&Path>) -> Result<Vec<u8>, DocxV2Error> {
    let mut ctx = EmitCtx::new(image_base, STATIC_RID_COUNT + 1);
    let body_xml = document::body(doc, &mut ctx);
    // Header/footer parts need to render with the ctx so any
    // embedded `@page-number()` / `@total-pages()` inlines emit
    // the right fields. We snapshot the blocks first and clear
    // them from the ctx so the recursive emission doesn't double-
    // collect.
    let headers = std::mem::take(&mut ctx.headers);
    let footers = std::mem::take(&mut ctx.footers);
    let mut header_xmls: Vec<String> = Vec::with_capacity(headers.len());
    for blocks in &headers {
        header_xmls.push(header_footer::header(blocks, &mut ctx));
    }
    let mut footer_xmls: Vec<String> = Vec::with_capacity(footers.len());
    for blocks in &footers {
        footer_xmls.push(header_footer::footer(blocks, &mut ctx));
    }
    pack(body_xml, &ctx, &header_xmls, &footer_xmls)
}

/// Build a docx with a single empty paragraph. Kept so the
/// scaffold tests (and the dev `STEM_DOCX2_DUMP` smoke artifact)
/// have a deterministic minimum reference.
pub fn minimal_empty_doc() -> Result<Vec<u8>, DocxV2Error> {
    let ctx = EmitCtx::new(None, STATIC_RID_COUNT + 1);
    pack(document::minimal(), &ctx, &[], &[])
}

fn pack(
    document_body_xml: String,
    ctx: &EmitCtx,
    header_xmls: &[String],
    footer_xmls: &[String],
) -> Result<Vec<u8>, DocxV2Error> {
    use content_type_names as ct;

    let mut ct_builder = content_types::builder()
        .override_part("/word/document.xml", ct::DOC_MAIN)
        .override_part("/word/styles.xml", ct::STYLES)
        .override_part("/word/numbering.xml", ct::NUMBERING)
        .override_part("/word/theme/theme1.xml", ct::THEME)
        .override_part("/word/settings.xml", ct::SETTINGS)
        .override_part("/word/webSettings.xml", ct::WEB_SETTINGS)
        .override_part("/word/fontTable.xml", ct::FONT_TABLE)
        .override_part("/word/footnotes.xml", ct::FOOTNOTES)
        .override_part("/word/endnotes.xml", ct::ENDNOTES)
        .override_part("/docProps/core.xml", ct::CORE)
        .override_part("/docProps/app.xml", ct::EXTENDED);
    for (i, _) in header_xmls.iter().enumerate() {
        ct_builder = ct_builder
            .override_part(&format!("/word/header{}.xml", i + 1), ct::HEADER);
    }
    for (i, _) in footer_xmls.iter().enumerate() {
        ct_builder = ct_builder
            .override_part(&format!("/word/footer{}.xml", i + 1), ct::FOOTER);
    }
    // Register a Default content-type per image extension actually
    // used so Word knows how to decode the bytes.
    let mut image_exts: Vec<&str> = ctx.images.iter().map(|i| i.ext.as_str()).collect();
    image_exts.sort();
    image_exts.dedup();
    for ext in &image_exts {
        ct_builder = ct_builder.default_extension(ext, image_content_type(ext));
    }
    let content_types = ct_builder.finish();

    let root_rels = rels::root_with_metadata();

    // Static-part rels — rIds 1..8 are reserved per STATIC_RID_COUNT.
    let mut doc_rels = vec![
        rels::Rel::new("rId1", rels::kind::STYLES, "styles.xml"),
        rels::Rel::new("rId2", rels::kind::NUMBERING, "numbering.xml"),
        rels::Rel::new("rId3", rels::kind::THEME, "theme/theme1.xml"),
        rels::Rel::new("rId4", rels::kind::SETTINGS, "settings.xml"),
        rels::Rel::new("rId5", rels::kind::WEB_SETTINGS, "webSettings.xml"),
        rels::Rel::new("rId6", rels::kind::FONT_TABLE, "fontTable.xml"),
        rels::Rel::new("rId7", rels::kind::FOOTNOTES, "footnotes.xml"),
        rels::Rel::new("rId8", rels::kind::ENDNOTES, "endnotes.xml"),
    ];
    for img in &ctx.images {
        // Target is the path relative to `word/`, which is where
        // `document.xml.rels` is interpreted.
        let target = img
            .zip_path
            .strip_prefix("word/")
            .unwrap_or(&img.zip_path)
            .to_string();
        doc_rels.push(rels::Rel::new(&img.rid, rels::kind::IMAGE, target));
    }
    for link in &ctx.hyperlinks {
        doc_rels.push(rels::Rel::external(
            &link.rid,
            rels::kind::HYPERLINK,
            &link.url,
        ));
    }
    for (i, rid) in ctx.header_rids.iter().enumerate() {
        doc_rels.push(rels::Rel::new(
            rid,
            rels::kind::HEADER,
            format!("header{}.xml", i + 1),
        ));
    }
    for (i, rid) in ctx.footer_rids.iter().enumerate() {
        doc_rels.push(rels::Rel::new(
            rid,
            rels::kind::FOOTER,
            format!("footer{}.xml", i + 1),
        ));
    }
    let doc_rels_xml = rels::build(&doc_rels);

    let mut pkg = Package::new();
    pkg.add_text("[Content_Types].xml", content_types);
    pkg.add_text("_rels/.rels", root_rels);
    pkg.add_text("word/_rels/document.xml.rels", doc_rels_xml);
    pkg.add_text("word/document.xml", document_body_xml);
    pkg.add_text(
        "word/styles.xml",
        styles::styles_with_overrides(&ctx.style_overrides),
    );
    pkg.add_text("word/numbering.xml", numbering::numbering());
    pkg.add_text("word/theme/theme1.xml", theme::theme1());
    let has_even = ctx
        .header_scopes
        .iter()
        .chain(ctx.footer_scopes.iter())
        .any(|s| matches!(s, super::emit::ctx::HeaderFooterScope::Even));
    pkg.add_text("word/settings.xml", settings::settings_with(has_even));
    pkg.add_text("word/webSettings.xml", web_settings::web_settings());
    pkg.add_text("word/fontTable.xml", font_table::font_table());
    pkg.add_text("docProps/core.xml", doc_props::core(&doc_props::now_w3cdtf()));
    pkg.add_text("docProps/app.xml", doc_props::app());
    for img in &ctx.images {
        pkg.add_bytes(img.zip_path.clone(), img.bytes.clone());
    }
    for (i, xml) in header_xmls.iter().enumerate() {
        pkg.add_text(format!("word/header{}.xml", i + 1), xml.clone());
    }
    for (i, xml) in footer_xmls.iter().enumerate() {
        pkg.add_text(format!("word/footer{}.xml", i + 1), xml.clone());
    }
    // Footnotes + endnotes parts are always present even if no
    // user-level notes were registered — settings.xml names their
    // placeholder ids -1 and 0, and Word reports a corrupted
    // document if the references dangle.
    pkg.add_text("word/footnotes.xml", footnotes::footnotes(&ctx.footnotes));
    pkg.add_text("word/endnotes.xml", endnotes::endnotes());
    pkg.finish()
}

fn image_content_type(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}
