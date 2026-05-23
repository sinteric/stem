#![cfg(feature = "docx")]
//! Tests for the direct-OOXML docx exporter.
//!
//! Two slices: (1) structural assertions on the OPC parts that every
//! docx must carry — `[Content_Types].xml`, root rels,
//! `word/document.xml`, the static parts (styles, numbering, theme,
//! settings, …); (2) higher-level end-to-end checks driving the
//! exporter from stem source through to the rendered OOXML.

use std::io::Read;

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::DocxExporter;
use stem_parser::parse;

fn export_empty() -> Vec<u8> {
    let r = parse("");
    let errs: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, stem_core::Severity::Error))
        .collect();
    assert!(errs.is_empty(), "parse errors: {:?}", errs);
    DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export")
}

fn read_entry(bytes: &[u8], path: &str) -> String {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("docx output is a valid zip");
    let mut file = zip
        .by_name(path)
        .unwrap_or_else(|_| panic!("entry `{}` missing", path));
    let mut s = String::new();
    file.read_to_string(&mut s).expect("entry is utf-8 xml");
    s
}

#[test]
fn empty_doc_writes_an_inspectable_artifact_when_env_set() {
    // Helper to eyeball the produced file in Word. Disabled by
    // default; enable with `STEM_DOCX_DUMP=/tmp/empty.docx cargo
    // test ...`.
    if let Ok(path) = std::env::var("STEM_DOCX_DUMP") {
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
    assert!(app.contains("<Application>Stem</Application>"));

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
    DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export")
}

#[test]
fn dump_example_artifacts_when_env_set() {
    // Helper for side-by-side verification in Word. Enable with
    // `STEM_DOCX_EXAMPLES_DIR=/tmp/docx_out cargo test ...`.
    let Ok(dir) = std::env::var("STEM_DOCX_EXAMPLES_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("mkdir");
    // `examples/` paths reference images via `references/...`
    // which resolve from the workspace root. The test runs from
    // `crates/stem-exports/`, so set image_base accordingly.
    let repo_root = std::path::Path::new("../..");
    for example in ["paper.stem", "paper_boringcrypto.stem", "roadmap.stem"] {
        let path = format!("../../examples/{example}");
        if let Ok(src) = std::fs::read_to_string(&path) {
            let r = stem_parser::parse(&src);
            let errs: Vec<_> = r
                .diagnostics
                .iter()
                .filter(|d| matches!(d.severity, stem_core::Severity::Error))
                .collect();
            assert!(errs.is_empty(), "parse errors in {example}: {:?}", errs);
            let bytes = DocxExporter::new()
                .with_image_base(repo_root)
                .export(&r.document, &Theme::default())
                .expect("export");
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
        "docx_img_{}_{}.png",
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

    // document.xml.rels must link rId9 (first body-allocated rId
    // after the 8 static parts: styles, numbering, theme,
    // settings, webSettings, fontTable, footnotes, endnotes) to
    // the image.
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Id="rId9""#));
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
    assert!(doc.contains(r#"r:embed="rId9""#));
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
fn style_override_patches_styles_xml() {
    let bytes = export_stem(
        r##"style[id:Heading1, color:"#C0392B", size:20pt, after:200pt]
h1(Hello)"##,
    );
    let styles = read_entry(&bytes, "word/styles.xml");
    // Walk to Heading1's style block and check the override values.
    let h1_start = styles.find(r#"w:styleId="Heading1""#).unwrap();
    let h1_end = styles[h1_start..].find("</w:style>").unwrap() + h1_start;
    let block = &styles[h1_start..h1_end];

    // Size override: 20pt = 40 half-points.
    assert!(
        block.contains(r#"<w:sz w:val="40"/>"#),
        "expected size override 40 hp: {block}"
    );
    // Color override → C0392B uppercase.
    assert!(
        block.contains(r#"<w:color w:val="C0392B"/>"#),
        "expected color override C0392B: {block}"
    );
    // After-spacing override: 200pt = 4000 dxa.
    assert!(
        block.contains(r#"w:after="4000""#),
        "expected after=4000 dxa: {block}"
    );
}

#[test]
fn image_paragraph_defaults_to_centered_and_overrides_via_align() {
    let tmp = std::env::temp_dir().join(format!(
        "docx_imgalign_{}_{}.png",
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

    // Default: no `align:` → image paragraph carries
    // `<w:jc w:val="center"/>` so figures center under their caption.
    let bytes = export_stem(&format!(r#"image[src:"{}", caption:"x"]"#, tmp.display()));
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(
        doc.contains(r#"<w:jc w:val="center"/>"#),
        "image paragraph should default-center: {doc}"
    );

    // Override: `image[align:left]` opts out of the centered default
    // (and no center jc should leak onto the image paragraph).
    let bytes2 = export_stem(&format!(
        r#"image[src:"{}", align:left, caption:"y"]"#,
        tmp.display()
    ));
    let doc2 = read_entry(&bytes2, "word/document.xml");
    let _ = std::fs::remove_file(&tmp);
    let drawing_pos = doc2.find("<w:drawing>").expect("drawing present");
    // Image paragraph runs from its own <w:p> open to the closing
    // </w:p> wrapping the <w:drawing>.
    let p_open = doc2[..drawing_pos].rfind("<w:p>").expect("image <w:p> open");
    let p_close = doc2[drawing_pos..].find("</w:p>").map(|o| drawing_pos + o).unwrap();
    let p_block = &doc2[p_open..p_close];
    assert!(
        p_block.contains(r#"<w:jc w:val="left"/>"#),
        "image[align:left] should override center: {p_block}"
    );
    assert!(
        !p_block.contains(r#"<w:jc w:val="center"/>"#),
        "centered default should not leak through override: {p_block}"
    );
}

#[test]
fn caption_style_defaults_to_centered_and_overrides_via_style_block() {
    // Default: the `Caption` style sets `<w:jc w:val="center"/>`
    // in its pPr, so figure/table captions land centered.
    let bytes = export_stem(r#"table[caption:"Plain"]{ row{ cell(x) } }"#);
    let styles = read_entry(&bytes, "word/styles.xml");
    let cap_start = styles.find(r#"w:styleId="Caption""#).expect("Caption defined");
    let cap_end = styles[cap_start..].find("</w:style>").unwrap() + cap_start;
    let cap_block = &styles[cap_start..cap_end];
    assert!(
        cap_block.contains(r#"<w:jc w:val="center"/>"#),
        "Caption should default-center: {cap_block}"
    );

    // Override: `style[id:Caption, align:left]` patches the style's
    // pPr so callers can opt out of the centered default.
    let bytes2 = export_stem(
        r##"style[id:Caption, align:left]
table[caption:"Plain"]{ row{ cell(x) } }"##,
    );
    let styles2 = read_entry(&bytes2, "word/styles.xml");
    let cap_start2 = styles2.find(r#"w:styleId="Caption""#).expect("Caption defined");
    let cap_end2 = styles2[cap_start2..].find("</w:style>").unwrap() + cap_start2;
    let cap_block2 = &styles2[cap_start2..cap_end2];
    assert!(
        cap_block2.contains(r#"<w:jc w:val="left"/>"#),
        "style[id:Caption, align:left] should override center: {cap_block2}"
    );
    assert!(
        !cap_block2.contains(r#"<w:jc w:val="center"/>"#),
        "old center value should not survive override: {cap_block2}"
    );
}

#[test]
fn style_override_block_doesnt_emit_into_body() {
    let bytes = export_stem(
        r##"style[id:Title, color:"#00FF00"]
h1(Body)"##,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    // No paragraph for the style block — only the heading.
    let p_count = doc.matches("<w:p>").count() + doc.matches("<w:p ").count();
    // 1 heading + the trailing sectPr placeholder (none here) → 1.
    assert!(p_count <= 2, "style block leaked a paragraph: {doc}");
}

#[test]
fn page_geometry_comes_from_metadata() {
    let bytes = export_stem(
        r#"[page-size:letter, margin:0.75in, header:0.4in, footer:0.4in]
p(hello)"#,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    // 0.75in = 1080 dxa; 0.4in = 576 dxa.
    assert!(
        doc.contains(r#"<w:pgMar w:top="1080" w:right="1080" w:bottom="1080" w:left="1080" w:header="576" w:footer="576" w:gutter="0"/>"#),
        "pgMar didn't pick up metadata: {doc}"
    );
    // Letter dimensions still in pgSz.
    assert!(doc.contains(r#"<w:pgSz w:w="12240" w:h="15840"/>"#));
}

#[test]
fn page_size_a4_resolves_to_dxa_dimensions() {
    let bytes = export_stem(r#"[page-size:a4]
p(hi)"#);
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(doc.contains(r#"<w:pgSz w:w="11906" w:h="16838"/>"#));
}

#[test]
fn image_paragraph_after_property_collapses_inter_paragraph_gap() {
    let tmp = std::env::temp_dir().join(format!(
        "docx_img_{}_{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    ));
    let png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&tmp, png).expect("write");
    let src = format!(
        r#"image[src:"{}", w:"1in", h:"1in", after:0pt]"#,
        tmp.display()
    );
    let bytes = export_stem(&src);
    let _ = std::fs::remove_file(&tmp);
    let doc = read_entry(&bytes, "word/document.xml");
    // The image paragraph emits its pPr with the spacing override.
    assert!(
        doc.contains(r#"<w:spacing w:after="0"/>"#)
            || doc.contains(r#"<w:spacing w:after="0" "#),
        "expected zero after-spacing on image paragraph: {doc}"
    );
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
fn footnote_inline_lands_in_footnotes_part_and_body_ref() {
    let bytes =
        export_stem(r#"p(See@footnote(the spec) for details and@footnote(other) too)"#);

    let doc = read_entry(&bytes, "word/document.xml");
    // Body has two footnoteReference runs with sequential ids.
    assert!(doc.contains(r#"<w:footnoteReference w:id="1"/>"#));
    assert!(doc.contains(r#"<w:footnoteReference w:id="2"/>"#));
    assert!(doc.contains(r#"<w:rStyle w:val="FootnoteReference"/>"#));

    // Footnotes part exists with both entries + separator
    // boilerplate.
    let fn_xml = read_entry(&bytes, "word/footnotes.xml");
    assert!(fn_xml.contains(r#"<w:footnote w:type="separator" w:id="-1">"#));
    assert!(fn_xml.contains(r#"<w:footnote w:id="1">"#));
    assert!(fn_xml.contains(r#"<w:footnote w:id="2">"#));
    assert!(fn_xml.contains("the spec"));
    assert!(fn_xml.contains("other"));

    // Content_Types + document rels include it.
    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(ct.contains("/word/footnotes.xml"));
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Target="footnotes.xml""#));
    assert!(doc_rels.contains("relationships/footnotes"));
}

#[test]
fn footnotes_and_endnotes_parts_are_always_present() {
    // Even when the source uses no `@footnote()` and no `@endnote()`,
    // settings.xml names the placeholder ids -1 and 0 inside its
    // `<w:footnotePr>` / `<w:endnotePr>` blocks. The corresponding
    // parts must exist or Word reports a corrupted document.
    let bytes = export_stem(r#"p(plain paragraph)"#);
    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(ct.contains("/word/footnotes.xml"));
    assert!(ct.contains("/word/endnotes.xml"));
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Target="footnotes.xml""#));
    assert!(doc_rels.contains(r#"Target="endnotes.xml""#));
    // The parts themselves have the boilerplate separator entries.
    let fn_xml = read_entry(&bytes, "word/footnotes.xml");
    assert!(fn_xml.contains(r#"<w:footnote w:type="separator" w:id="-1">"#));
    let en_xml = read_entry(&bytes, "word/endnotes.xml");
    assert!(en_xml.contains(r#"<w:endnote w:type="separator" w:id="-1">"#));
}

#[test]
fn header_and_footer_become_separate_parts() {
    let bytes = export_stem(
        r#"header{ p(My document) }
footer{ p(Page @page-number() of @total-pages()) }
h1(Body)"#,
    );

    // The body must NOT contain the header/footer text — they
    // live in their own parts now.
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(!doc.contains("My document"), "header text leaked into body");

    // sectPr names both via headerReference / footerReference.
    assert!(doc.contains("<w:headerReference "));
    assert!(doc.contains("<w:footerReference "));

    // header1.xml + footer1.xml exist with the right roots.
    let h1 = read_entry(&bytes, "word/header1.xml");
    assert!(h1.contains("<w:hdr"));
    assert!(h1.contains("My document"));
    let f1 = read_entry(&bytes, "word/footer1.xml");
    assert!(f1.contains("<w:ftr"));
    // Footer pulls in real PAGE/NUMPAGES fields from the inline
    // field emitter.
    assert!(f1.contains(" PAGE "));
    assert!(f1.contains(" NUMPAGES "));

    // Content_Types registers both.
    let ct = read_entry(&bytes, "[Content_Types].xml");
    assert!(ct.contains("/word/header1.xml"));
    assert!(ct.contains("/word/footer1.xml"));
    // Document rels include header + footer entries.
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(doc_rels.contains(r#"Target="header1.xml""#));
    assert!(doc_rels.contains(r#"Target="footer1.xml""#));
    assert!(doc_rels.contains("relationships/header"));
    assert!(doc_rels.contains("relationships/footer"));
}

#[test]
fn toc_section_emits_field_with_prepopulated_heading_entries() {
    let bytes = export_stem(
        r#"section[id:toc]
h1(Intro)
h2(Why)
h1(Done)"#,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    // TOC heading paragraph.
    assert!(doc.contains(r#"<w:pStyle w:val="TOCHeading"/>"#));
    assert!(doc.contains("Table of Contents"));
    // TOC field present.
    assert!(doc.contains(" TOC "));
    // Three entries — one per heading.
    let toc_hyperlinks = doc.matches(r#"<w:hyperlink w:anchor="_Toc"#).count();
    assert!(toc_hyperlinks >= 3, "expected ≥3 TOC entries, got {toc_hyperlinks}");
    // The heading bookmarks each entry refers to are present in
    // the body.
    for i in 1..=3 {
        assert!(doc.contains(&format!(r#"w:name="_Toc{i}""#)));
    }
}

#[test]
fn list_of_tables_emits_table_caption_entries() {
    let bytes = export_stem(
        r#"section[id:list-of-tables]
table[caption:"Alpha"]{ row{ cell(a) } }
table[caption:"Beta"]{ row{ cell(b) } }"#,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(doc.contains("List of Tables"));
    // Entries pre-formatted as "Table N. <text>".
    assert!(doc.contains("Table 1. Alpha"));
    assert!(doc.contains("Table 2. Beta"));
    // Anchor hyperlinks to the matching caption bookmarks.
    assert!(doc.contains(r#"<w:hyperlink w:anchor="_Toc_table_1""#));
    assert!(doc.contains(r#"<w:hyperlink w:anchor="_Toc_table_2""#));
    // Caption paragraphs are bookmarked.
    assert!(doc.contains(r#"w:name="_Toc_table_1""#));
}

#[test]
fn external_link_lands_in_doc_rels_and_w_hyperlink_uses_rid() {
    let bytes = export_stem(
        r#"p(visit @link[to:"https://example.org/foo"](this site) for details)"#,
    );
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(
        doc_rels.contains(r#"Target="https://example.org/foo""#),
        "missing external target: {doc_rels}"
    );
    assert!(doc_rels.contains(r#"TargetMode="External""#));
    assert!(doc_rels.contains("relationships/hyperlink"));

    let doc = read_entry(&bytes, "word/document.xml");
    assert!(doc.contains("<w:hyperlink "));
    assert!(doc.contains(r#"<w:rStyle w:val="Hyperlink"/>"#));
    assert!(doc.contains("this site"));
}

#[test]
fn anchor_link_does_not_create_a_rel() {
    let bytes = export_stem(r#"p(see @link[to:"ref://_Toc1"](Section 1))"#);
    let doc_rels = read_entry(&bytes, "word/_rels/document.xml.rels");
    assert!(
        !doc_rels.contains("TargetMode"),
        "anchor link should not create a rel: {doc_rels}"
    );
    let doc = read_entry(&bytes, "word/document.xml");
    assert!(doc.contains(r#"<w:hyperlink w:anchor="_Toc1""#));
}

#[test]
fn headings_emit_toc_bookmarks_in_document_order() {
    let bytes = export_stem(
        r#"h1(Alpha)
h2(Beta)
h1(Gamma)"#,
    );
    let doc = read_entry(&bytes, "word/document.xml");
    for n in 1..=3 {
        let name = format!(r#"w:name="_Toc{n}""#);
        assert!(doc.contains(&name), "missing bookmark name {name}");
    }
    // Bookmarks appear in document order.
    let p1 = doc.find(r#"w:name="_Toc1""#).unwrap();
    let p2 = doc.find(r#"w:name="_Toc2""#).unwrap();
    let p3 = doc.find(r#"w:name="_Toc3""#).unwrap();
    assert!(p1 < p2 && p2 < p3);
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
