//! OOXML parts. Each submodule emits one part (or family of parts)
//! as a `String` ready to be added to the ZIP package.

use stem_core::ast::Document;

use super::package::Package;
use super::DocxV2Error;

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
/// [`super::emit::paragraph`].
pub fn package_doc(doc: &Document) -> Result<Vec<u8>, DocxV2Error> {
    pack_with_body(document::body(doc))
}

/// Build a docx with a single empty paragraph. Kept so the
/// scaffold tests (and the dev `STEM_DOCX2_DUMP` smoke artifact)
/// have a deterministic minimum reference.
pub fn minimal_empty_doc() -> Result<Vec<u8>, DocxV2Error> {
    pack_with_body(document::minimal())
}

fn pack_with_body(document_body_xml: String) -> Result<Vec<u8>, DocxV2Error> {
    use content_type_names as ct;

    let content_types = content_types::builder()
        .override_part("/word/document.xml", ct::DOC_MAIN)
        .override_part("/word/styles.xml", ct::STYLES)
        .override_part("/word/numbering.xml", ct::NUMBERING)
        .override_part("/word/theme/theme1.xml", ct::THEME)
        .override_part("/word/settings.xml", ct::SETTINGS)
        .override_part("/word/webSettings.xml", ct::WEB_SETTINGS)
        .override_part("/word/fontTable.xml", ct::FONT_TABLE)
        .override_part("/docProps/core.xml", ct::CORE)
        .override_part("/docProps/app.xml", ct::EXTENDED)
        .finish();

    let root_rels = rels::root_with_metadata();

    let doc_rels = rels::build(&[
        rels::Rel::new("rId1", rels::kind::STYLES, "styles.xml"),
        rels::Rel::new("rId2", rels::kind::NUMBERING, "numbering.xml"),
        rels::Rel::new("rId3", rels::kind::THEME, "theme/theme1.xml"),
        rels::Rel::new("rId4", rels::kind::SETTINGS, "settings.xml"),
        rels::Rel::new("rId5", rels::kind::WEB_SETTINGS, "webSettings.xml"),
        rels::Rel::new("rId6", rels::kind::FONT_TABLE, "fontTable.xml"),
    ]);

    let mut pkg = Package::new();
    pkg.add_text("[Content_Types].xml", content_types);
    pkg.add_text("_rels/.rels", root_rels);
    pkg.add_text("word/_rels/document.xml.rels", doc_rels);
    pkg.add_text("word/document.xml", document_body_xml);
    pkg.add_text("word/styles.xml", styles::styles());
    pkg.add_text("word/numbering.xml", numbering::numbering());
    pkg.add_text("word/theme/theme1.xml", theme::theme1());
    pkg.add_text("word/settings.xml", settings::settings());
    pkg.add_text("word/webSettings.xml", web_settings::web_settings());
    pkg.add_text("word/fontTable.xml", font_table::font_table());
    pkg.add_text("docProps/core.xml", doc_props::core(&doc_props::now_w3cdtf()));
    pkg.add_text("docProps/app.xml", doc_props::app());
    pkg.finish()
}
