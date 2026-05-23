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

#[test]
fn empty_doc_styles_part_has_canonical_set() {
    let bytes = export_empty();
    let styles = read_entry(&bytes, "word/styles.xml");

    // Every style ID we'll reference from document.xml exists.
    for id in [
        "Normal",
        "DefaultParagraphFont",
        "TableNormal",
        "Heading1",
        "Heading6",
        "Title",
        "Caption",
        "Hyperlink",
        "FootnoteReference",
        "TOC1",
        "TOC9",
        "TOCHeading",
        "TableofFigures",
        "ListParagraph",
    ] {
        assert!(
            styles.contains(&format!(r#"w:styleId="{id}""#)),
            "missing styleId {id}"
        );
    }

    // docDefaults precedes latentStyles precedes real styles —
    // the schema requires this order.
    let dd = styles.find("<w:docDefaults>").unwrap();
    let ls = styles.find("<w:latentStyles ").unwrap();
    let normal = styles.find(r#"w:styleId="Normal""#).unwrap();
    assert!(dd < ls && ls < normal);

    // Heading1's <w:pPr> emits keepNext before spacing before
    // outlineLvl — the schema-order bug that motivated this
    // migration in the first place.
    let h1 = styles.find(r#"w:styleId="Heading1""#).unwrap();
    let h1_end = styles[h1..].find("</w:style>").unwrap() + h1;
    let h1_block = &styles[h1..h1_end];
    let kn = h1_block.find("<w:keepNext/>").unwrap();
    let sp = h1_block.find("<w:spacing").unwrap();
    let ol = h1_block.find("<w:outlineLvl").unwrap();
    assert!(kn < sp && sp < ol, "block:\n{h1_block}");

    // Style metadata (uiPriority, qFormat) precedes pPr in every
    // heading — the other schema-order bug docx-rs hit.
    let pri = h1_block.find("<w:uiPriority").unwrap();
    let qf = h1_block.find("<w:qFormat/>").unwrap();
    let ppr = h1_block.find("<w:pPr>").unwrap();
    assert!(pri < qf && qf < ppr);

    // Content_Types registers the styles part.
    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(ct.contains("/word/styles.xml"));
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Target="styles.xml""#));
}

#[test]
fn empty_doc_numbering_part_has_three_lists() {
    let bytes = export_empty();
    let n = read_entry(&bytes, "word/numbering.xml");

    // Three abstractNum + three num entries (ordered/bullet/heading).
    assert_eq!(n.matches("<w:abstractNum ").count(), 3);
    assert_eq!(n.matches("<w:num ").count(), 3);

    // Heading multilevel links to Heading1..Heading6.
    for level in 1..=6 {
        assert!(
            n.contains(&format!(r#"<w:pStyle w:val="Heading{level}"/>"#)),
            "missing heading link for level {level}"
        );
    }

    // Within the first <w:lvl> the children are in canonical
    // order: start → numFmt → lvlText → lvlJc → pPr.
    let first = n.find("<w:lvl ").unwrap();
    let end = n[first..].find("</w:lvl>").unwrap() + first;
    let block = &n[first..end];
    let start = block.find("<w:start").unwrap();
    let numfmt = block.find("<w:numFmt").unwrap();
    let lvltext = block.find("<w:lvlText").unwrap();
    let lvljc = block.find("<w:lvlJc").unwrap();
    let ppr = block.find("<w:pPr>").unwrap();
    assert!(start < numfmt && numfmt < lvltext && lvltext < lvljc && lvljc < ppr);

    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(ct.contains("/word/numbering.xml"));
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Target="numbering.xml""#));
}

fn export_stem(src: &str) -> Vec<u8> {
    let r = stem_parser::parse(src);
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

#[test]
fn dump_example_artifacts_when_env_set() {
    // Helper for side-by-side verification in Word. Enable with
    // `STEM_DOCX2_EXAMPLES_DIR=/tmp/docx2_out cargo test ...`.
    let Ok(dir) = std::env::var("STEM_DOCX2_EXAMPLES_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("mkdir");
    for example in ["paper.stem", "paper_boringcrypto.stem", "roadmap.stem"] {
        let path = format!("../../examples/{example}");
        if let Ok(src) = std::fs::read_to_string(&path) {
            let bytes = export_stem(&src);
            let out_path = format!("{}/{}", dir, example.replace(".stem", ".docx"));
            std::fs::write(&out_path, &bytes).expect("write");
            eprintln!("wrote {out_path} ({} bytes)", bytes.len());
        } else {
            eprintln!("skipping {path}");
        }
    }
}

#[test]
fn example_paper_renders_with_paragraph_body() {
    let src = std::fs::read_to_string("../../examples/paper.stem").expect("read paper.stem");
    let bytes = export_stem(&src);
    let doc = read_entry(&bytes, "word/document.xml");

    // The example uses Title, H1, H2 — every heading should land
    // with the matching pStyle.
    assert!(doc.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert!(doc.contains(r#"<w:pStyle w:val="Heading2"/>"#));

    // Each `numbered:true` heading carries `<w:numPr>` linked to
    // the heading numbering definition (numId 3).
    assert!(doc.contains("<w:numPr>"));
    assert!(doc.contains(r#"<w:numId w:val="3"/>"#));

    // Document opens with sectPr at the end of the body.
    assert!(doc.ends_with("</w:document>") || doc.ends_with("</w:document>\n"));
    assert!(doc.contains("<w:sectPr>"));
}

#[test]
fn example_boringcrypto_renders_structurally() {
    let path = "../../examples/paper_boringcrypto.stem";
    let Ok(src) = std::fs::read_to_string(path) else {
        // The example file isn't always present in CI checkout
        // contexts; skip the structural assertion if missing.
        eprintln!("skipping: {path} not present");
        return;
    };
    let bytes = export_stem(&src);
    let doc = read_entry(&bytes, "word/document.xml");

    // BoringCrypto has a `title` block on the cover, Heading1 +
    // Heading2 throughout, and many `numbered:true` headings.
    assert!(doc.contains(r#"<w:pStyle w:val="Title"/>"#));
    assert!(doc.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert!(doc.contains(r#"<w:pStyle w:val="Heading2"/>"#));

    // Paragraph count — task 6 emits one <w:p> per top-level
    // block (with section blocks recursed into). The source has
    // ~196 paragraph-like blocks; the rendered count should be in
    // the same ballpark.
    let p_count = doc.matches("<w:p>").count() + doc.matches("<w:p ").count() + doc.matches("<w:p/>").count();
    assert!(
        p_count > 100,
        "expected >100 paragraphs from boringcrypto, got {p_count}"
    );

    // @text[weight:bold] appears in the source — task 7 must emit
    // it as a bold run, not flatten it.
    assert!(
        doc.contains("<w:b/>"),
        "expected at least one <w:b/> run from `@text[weight:bold]` in the source"
    );
}

#[test]
fn embedded_image_lands_in_media_and_rels() {
    // Mint a 1×1 PNG on disk and reference it from a stem source.
    let tmp = std::env::temp_dir().join(format!(
        "docx2_img_{}_{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    let png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&tmp, png).expect("write png");
    let src = format!(
        r#"p(before)
image[src:"{}", w:"1in", h:"1in"]
p(after)
"#,
        tmp.display()
    );

    let bytes = export_stem(&src);
    let _ = std::fs::remove_file(&tmp);

    // word/media/image1.png must exist and equal the bytes we wrote.
    let media = read_entry_bytes(&bytes, "word/media/image1.png");
    assert_eq!(media, png);

    // document.xml.rels must link rId7 (first body-allocated rId
    // after the 6 static parts) to the image.
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Id="rId7""#));
    assert!(doc_rels.contains(r#"Target="media/image1.png""#));
    assert!(doc_rels.contains("relationships/image"));

    // Content_Types must declare image/png as a Default for the
    // png extension.
    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(
        ct.contains(r#"Extension="png""#) && ct.contains(r#"ContentType="image/png""#),
        "Content_Types missing png default: {ct}"
    );

    // The body must reference the image via <w:drawing> + r:embed.
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(doc.contains("<w:drawing>"));
    assert!(doc.contains(r#"r:embed="rId7""#));
    assert!(doc.contains("<wp:inline"));
}

fn read_entry_bytes(bytes: &[u8], path: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
    let mut entry = zip.by_name(path).unwrap_or_else(|_| panic!("missing {path}"));
    let mut out = Vec::new();
    std::io::copy(&mut entry, &mut out).expect("read");
    out
}

#[test]
fn boringcrypto_renders_all_tables() {
    let path = "../../examples/paper_boringcrypto.stem";
    let Ok(src) = std::fs::read_to_string(path) else {
        return;
    };
    let bytes = export_stem(&src);
    let doc = read_entry(&bytes, "word/document.xml");

    // Source has 15 `table[...]` blocks at the top level. Each must
    // emit one `<w:tbl>`.
    let tbl_count = doc.matches("<w:tbl>").count();
    assert!(
        tbl_count >= 15,
        "expected ≥15 tables in boringcrypto, got {tbl_count}"
    );
    // Every table emits its grid columns.
    assert!(doc.contains("<w:gridCol "));
    // Header rows mark themselves with `<w:tblHeader/>`.
    assert!(doc.contains("<w:tblHeader/>"));
    // Caption paragraphs immediately follow tables.
    assert!(doc.contains(r#"<w:pStyle w:val="Caption"/>"#));
}

#[test]
fn page_and_numpages_inlines_emit_fields() {
    let bytes = export_stem(r#"p(Page @page-number() of @total-pages())"#);
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(
        doc.contains(r#"w:instr=" PAGE   \* MERGEFORMAT ""#),
        "missing PAGE field: {doc}"
    );
    assert!(
        doc.contains(r#"w:instr=" NUMPAGES   \* MERGEFORMAT ""#),
        "missing NUMPAGES field"
    );
}

#[test]
fn table_caption_emits_seq_field_with_table_label() {
    let bytes = export_stem(
        r#"table[caption:"First"]{ row{ cell(a) } }
table[caption:"Second"]{ row{ cell(b) } }"#,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    // Two SEQ Table fields — one per caption.
    let seq_count = doc.matches(r#"w:instr=" SEQ Table \* ARABIC ""#).count();
    assert_eq!(seq_count, 2, "expected 2 SEQ Table fields, got {seq_count}");
    // Pre-computed numbers 1 and 2 appear in the cached results.
    assert!(doc.contains(r#"<w:t xml:space="preserve">1</w:t>"#));
    assert!(doc.contains(r#"<w:t xml:space="preserve">2</w:t>"#));
}

#[test]
fn rich_text_pieces_become_separate_runs() {
    // Tight end-to-end check on rPr extraction: a single paragraph
    // with two inline overrides must produce three runs with the
    // right rPr on each.
    let bytes = export_stem(
        r#"p(plain @text[weight:bold](bold) middle @text[style:italic](em) tail)"#,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    // Total runs in the document: 5 paragraph text runs.
    let r_count = doc.matches("<w:r>").count() + doc.matches("<w:r ").count();
    assert!(r_count >= 5, "expected at least 5 runs, got {r_count}: {doc}");
    // Exactly one bold and one italic run.
    assert_eq!(doc.matches("<w:b/>").count(), 1, "bold count mismatch");
    assert_eq!(doc.matches("<w:i/>").count(), 1, "italic count mismatch");
}
