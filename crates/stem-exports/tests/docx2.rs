#![cfg(feature = "docx2")]
//! Tests for the in-progress direct-OOXML docx exporter.
//!
//! Task 1 scaffold: the exporter must produce a valid OPC ZIP whose
//! mandatory parts are present. Body content + style fidelity are
//! verified by later tasks; here we only check that the bytes are
//! well-formed and the structure is recognized by the `zip` reader.

use std::io::Read;

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::DocxV2Exporter;
use stem_parser::parse;

fn export_empty() -> Vec<u8> {
    let r = parse("");
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, stem_core::Severity::Error))
        .collect();
    assert!(errs.is_empty(), "parse errors: {:?}", errs);
    DocxV2Exporter::new()
        .export(&r.document, &Theme::default())
        .expect("export")
}

fn read_entry(bytes: &[u8], path: &str) -> String {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("docx2 output is a valid zip");
    let mut file = zip
        .by_name(path)
        .unwrap_or_else(|_| panic!("entry `{}` missing", path));
    let mut s = String::new();
    file.read_to_string(&mut s).expect("entry is utf-8 xml");
    s
}

#[test]
fn empty_doc_writes_an_inspectable_artifact_when_env_set() {
    // Helper to eyeball the produced file in Word during scaffold
    // work. Disabled by default; enable with
    // `STEM_DOCX2_DUMP=/tmp/empty.docx cargo test ...`.
    if let Ok(path) = std::env::var("STEM_DOCX2_DUMP") {
        let bytes = export_empty();
        std::fs::write(&path, &bytes).expect("dump");
    }
}

#[test]
fn empty_doc_is_valid_opc_package() {
    let bytes = export_empty();

    // Required OPC parts for a minimal docx.
    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(ct.contains("wordprocessingml.document.main+xml"));

    let root_rels = read_entry(&bytes, "_rels/.rels");
    assert!(root_rels.contains("word/document.xml"));

    // Document rels present (possibly empty body — that's fine).
    let _ = read_entry(&bytes, "word/_rels/document.xml.rels");

    let doc = read_entry(&bytes, "word/document.xml");
    assert!(doc.contains("<w:document"));
    assert!(doc.contains("<w:body>"));
    // Section properties must be present so Word lays the page out.
    assert!(doc.contains("<w:sectPr>"));
    assert!(doc.contains("<w:pgSz"));
    assert!(doc.contains("<w:pgMar"));
}

#[test]
fn empty_doc_includes_static_parts() {
    let bytes = export_empty();

    // Each static part exists and is content-type-registered.
    let ct = read_entry(&bytes, "[Content_Types].xml");
    for needed in [
        "/word/theme/theme1.xml",
        "/word/settings.xml",
        "/word/webSettings.xml",
        "/word/fontTable.xml",
        "/docProps/core.xml",
        "/docProps/app.xml",
    ] {
        assert!(
            ct.contains(needed),
            "Content_Types missing override for {needed}: {ct}"
        );
    }

    let theme = read_entry(&bytes, "word/theme/theme1.xml");
    assert!(theme.contains("<a:clrScheme"));
    assert!(theme.contains("<a:fontScheme"));

    let settings = read_entry(&bytes, "word/settings.xml");
    assert!(settings.contains("<w:zoom"));
    assert!(settings.contains("compatibilityMode"));

    let web = read_entry(&bytes, "word/webSettings.xml");
    assert!(web.contains("<w:optimizeForBrowser/>"));

    let fonts = read_entry(&bytes, "word/fontTable.xml");
    assert!(fonts.contains(r#"w:name="Calibri""#));
    assert!(fonts.contains(r#"w:name="Cambria""#));

    let core = read_entry(&bytes, "docProps/core.xml");
    assert!(core.contains("<cp:coreProperties"));
    let app = read_entry(&bytes, "docProps/app.xml");
    assert!(app.contains("<Application>Stem (docx2)</Application>"));

    // Document rels reference each of the new parts so Word doesn't
    // see a dangling Override.
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    for needed in [
        "theme/theme1.xml",
        "settings.xml",
        "webSettings.xml",
        "fontTable.xml",
    ] {
        assert!(
            doc_rels.contains(&format!(r#"Target="{needed}""#)),
            "document rels missing target {needed}: {doc_rels}"
        );
    }

    // Root rels include the docProps refs.
    let root = read_entry(&bytes, "_rels/.rels");
    assert!(root.contains("docProps/core.xml"));
    assert!(root.contains("docProps/app.xml"));
}
