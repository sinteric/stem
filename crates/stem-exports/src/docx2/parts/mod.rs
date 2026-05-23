//! OOXML parts. Each submodule emits one part (or family of parts)
//! as a `String` ready to be added to the ZIP package.

use std::path::Path;

use stem_core::ast::Document;

use super::emit::ctx::EmitCtx;
use super::package::Package;
use super::DocxV2Error;

/// rId space layout for `document.xml.rels`:
/// rIds 1..STATIC_RID_COUNT inclusive are the static parts
/// (styles, numbering, theme, settings, webSettings, fontTable).
/// Body emission allocates from `STATIC_RID_COUNT + 1` onward
/// for images, hyperlinks, footnotes, headers, footers.
const STATIC_RID_COUNT: u32 = 6;

pub mod content_types;
pub mod doc_props;
pub mod document;
pub mod font_table;
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
    pack(body_xml, &ctx)
}

/// Build a docx with a single empty paragraph. Kept so the
/// scaffold tests (and the dev `STEM_DOCX2_DUMP` smoke artifact)
/// have a deterministic minimum reference.
pub fn minimal_empty_doc() -> Result<Vec<u8>, DocxV2Error> {
    let ctx = EmitCtx::new(None, STATIC_RID_COUNT + 1);
    pack(document::minimal(), &ctx)
}

fn pack(document_body_xml: String, ctx: &EmitCtx) -> Result<Vec<u8>, DocxV2Error> {
    use content_type_names as ct;

    let mut ct_builder = content_types::builder()
        .override_part("/word/document.xml", ct::DOC_MAIN)
        .override_part("/word/styles.xml", ct::STYLES)
        .override_part("/word/numbering.xml", ct::NUMBERING)
        .override_part("/word/theme/theme1.xml", ct::THEME)
        .override_part("/word/settings.xml", ct::SETTINGS)
        .override_part("/word/webSettings.xml", ct::WEB_SETTINGS)
        .override_part("/word/fontTable.xml", ct::FONT_TABLE)
        .override_part("/docProps/core.xml", ct::CORE)
        .override_part("/docProps/app.xml", ct::EXTENDED);
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

    // Static-part rels — rIds 1..6 are reserved per STATIC_RID_COUNT.
    let mut doc_rels = vec![
        rels::Rel::new("rId1", rels::kind::STYLES, "styles.xml"),
        rels::Rel::new("rId2", rels::kind::NUMBERING, "numbering.xml"),
        rels::Rel::new("rId3", rels::kind::THEME, "theme/theme1.xml"),
        rels::Rel::new("rId4", rels::kind::SETTINGS, "settings.xml"),
        rels::Rel::new("rId5", rels::kind::WEB_SETTINGS, "webSettings.xml"),
        rels::Rel::new("rId6", rels::kind::FONT_TABLE, "fontTable.xml"),
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
    let doc_rels_xml = rels::build(&doc_rels);

    let mut pkg = Package::new();
    pkg.add_text("[Content_Types].xml", content_types);
    pkg.add_text("_rels/.rels", root_rels);
    pkg.add_text("word/_rels/document.xml.rels", doc_rels_xml);
    pkg.add_text("word/document.xml", document_body_xml);
    pkg.add_text("word/styles.xml", styles::styles());
    pkg.add_text("word/numbering.xml", numbering::numbering());
    pkg.add_text("word/theme/theme1.xml", theme::theme1());
    pkg.add_text("word/settings.xml", settings::settings());
    pkg.add_text("word/webSettings.xml", web_settings::web_settings());
    pkg.add_text("word/fontTable.xml", font_table::font_table());
    pkg.add_text("docProps/core.xml", doc_props::core(&doc_props::now_w3cdtf()));
    pkg.add_text("docProps/app.xml", doc_props::app());
    for img in &ctx.images {
        pkg.add_bytes(img.zip_path.clone(), img.bytes.clone());
    }
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
