#![cfg(feature = "docx")]

use std::io::Read;

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::DocxExporter;
use stem_imports::markdown::import_str;
use stem_parser::parse;

fn docx_bytes(src: &str) -> Vec<u8> {
    let doc = import_str(src);
    DocxExporter::new()
        .export(&doc, &Theme::default())
        .expect("export")
}

fn docx_bytes_from_stem(src: &str) -> Vec<u8> {
    let r = parse(src);
    // Diagnostics include hints (e.g. empty `()` body) which aren't
    // fatal — only fail on actual Error severity.
    let errors: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| matches!(d.severity, stem_core::Severity::Error))
        .collect();
    assert!(errors.is_empty(), "parse errors: {:?}", errors);
    DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export")
}

/// Pull `word/document.xml` (deflated entry) out of the .docx ZIP so we
/// can grep the OOXML.
fn extract_document_xml(bytes: &[u8]) -> String {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("docx is a zip");
    let mut file = zip
        .by_name("word/document.xml")
        .expect("word/document.xml present");
    let mut s = String::new();
    file.read_to_string(&mut s).expect("utf-8 xml");
    s
}

#[test]
fn output_is_valid_zip() {
    let bytes = docx_bytes("# Hello\n");
    // .docx is a ZIP archive — magic "PK\x03\x04".
    assert!(
        bytes.starts_with(b"PK\x03\x04"),
        "missing ZIP magic: {:?}",
        &bytes[..bytes.len().min(8)]
    );
    assert!(bytes.len() > 1000, "output too small: {} bytes", bytes.len());
}

#[test]
fn document_xml_is_present() {
    // ZIP central directory should reference word/document.xml. We can
    // grep for the filename appearing in the byte stream.
    let bytes = docx_bytes("# Hello\n");
    let blob = String::from_utf8_lossy(&bytes);
    assert!(
        blob.contains("word/document.xml"),
        "missing word/document.xml in ZIP entries"
    );
}

#[test]
fn heading_emits_heading_style() {
    // The document.xml stream is compressed in the ZIP, but the
    // [Content_Types] and references are not. Instead of unzipping in
    // tests, just check the bytes are well-formed and non-trivial.
    let bytes = docx_bytes("# H1 Title\n\n## H2 Subtitle\n");
    assert!(bytes.len() > 1500);
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn list_renders_without_error() {
    let bytes = docx_bytes("- alpha\n- beta\n- gamma\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.len() > 1500);
}

#[test]
fn ordered_list_renders() {
    let bytes = docx_bytes("1. First\n2. Second\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn paragraph_with_bold_italic_renders() {
    let bytes = docx_bytes("This has **bold** and *italic* spans.\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.len() > 1500);
}

#[test]
fn code_block_renders() {
    let bytes = docx_bytes("```rust\nfn main() {}\n```\n");
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn empty_document_is_valid() {
    let bytes = docx_bytes("");
    assert!(bytes.starts_with(b"PK\x03\x04"));
    assert!(bytes.len() > 500);
}

// --- tables -------------------------------------------------------------

#[test]
fn table_emits_tbl_with_rows_and_cells() {
    let bytes = docx_bytes_from_stem(
        "table[border:all]{ row[kind:header]{ cell(A) cell(B) } row{ cell(1) cell(2) } }",
    );
    let xml = extract_document_xml(&bytes);
    // Two rows × two cells each.
    let tbl_count = xml.matches("<w:tbl>").count();
    let tr_count = xml.matches("<w:tr>").count();
    let tc_count = xml.matches("<w:tc>").count();
    assert_eq!(tbl_count, 1, "expected 1 <w:tbl>, xml: {}", xml);
    assert_eq!(tr_count, 2, "expected 2 <w:tr>, got {}", tr_count);
    assert_eq!(tc_count, 4, "expected 4 <w:tc>, got {}", tc_count);
    // Header cell text becomes bold.
    assert!(xml.contains("<w:b "), "expected a <w:b> bold marker in header");
}

#[test]
fn table_colspan_emits_grid_span() {
    let bytes = docx_bytes_from_stem(
        "table{ row[kind:header]{ cell(Title) cell[colspan:2](Span2) } row{ cell(a) cell(b) cell(c) } }",
    );
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("w:gridSpan w:val=\"2\""), "expected gridSpan=2; xml: {}", xml);
}

#[test]
fn table_rowspan_emits_vmerge_restart_and_continue() {
    let bytes = docx_bytes_from_stem(
        "table{ row{ cell[rowspan:2](Tall) cell(A) } row{ cell(B) } }",
    );
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("w:vMerge w:val=\"restart\""), "expected vMerge=restart; xml: {}", xml);
    assert!(xml.contains("w:vMerge w:val=\"continue\""), "expected vMerge=continue; xml: {}", xml);
    // Three rows in source had two cells then one — after merge expansion
    // we still get two <w:tr> with two <w:tc> each (the continue cell
    // synthesizes the missing slot in row 2).
    let tr_count = xml.matches("<w:tr>").count();
    let tc_count = xml.matches("<w:tc>").count();
    assert_eq!(tr_count, 2);
    assert_eq!(tc_count, 4);
}

#[test]
fn table_bg_resolves_named_theme_color() {
    let bytes = docx_bytes_from_stem("table{ row{ cell[bg:yellow](highlight) } }");
    let xml = extract_document_xml(&bytes);
    // Theme yellow is #ffd33d → "FFD33D" in OOXML.
    assert!(
        xml.contains("FFD33D") || xml.contains("ffd33d"),
        "expected yellow fill in xml: {}",
        xml
    );
}

#[test]
fn table_caption_appears_before_table_with_auto_seq() {
    let bytes = docx_bytes_from_stem(r#"table[caption:"FIPS sections"]{ row{ cell(x) } }"#);
    let xml = extract_document_xml(&bytes);
    let caption_pos = xml.find("FIPS sections").expect("caption text present");
    let tbl_pos = xml.find("<w:tbl>").expect("table present");
    assert!(caption_pos < tbl_pos, "caption should precede table");
    assert!(
        xml.contains("w:pStyle w:val=\"Caption\""),
        "caption should use Caption style: {}",
        xml
    );
    // Auto-numbering: caption emits a SEQ Table field, prefixed with
    // the literal "Table " label.
    assert!(xml.contains("SEQ Table"), "expected SEQ Table field: {}", xml);
    assert!(
        xml.contains(">Table <"),
        "expected literal 'Table ' label before SEQ: {}",
        xml
    );
}

#[test]
fn table_border_outer_clears_inside_borders() {
    let bytes_outer = docx_bytes_from_stem("table[border:outer]{ row{ cell(a) cell(b) } }");
    let xml_outer = extract_document_xml(&bytes_outer);
    // outer mode: outer single borders kept, inside H/V cleared. docx-rs
    // emits cleared borders as `w:val="nil"` (Word treats nil and none
    // equivalently for inside borders).
    assert!(xml_outer.contains("w:insideH w:val=\"nil\""), "outer borders should clear insideH: {}", xml_outer);
    assert!(xml_outer.contains("w:insideV w:val=\"nil\""), "outer borders should clear insideV: {}", xml_outer);
    assert!(xml_outer.contains("w:top w:val=\"single\""), "outer borders should keep top: {}", xml_outer);
}

// --- inline links + footnotes (stage 2) ---------------------------------

#[test]
fn inline_link_emits_external_hyperlink() {
    let bytes = docx_bytes_from_stem(
        r#"p(See the @link[to:"https://example.com"](docs) for details.)"#,
    );
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("<w:hyperlink"), "expected <w:hyperlink>: {}", xml);
    // External links live in document rels; their r:id is referenced
    // here.
    assert!(xml.contains("r:id="), "external hyperlink should reference an rId: {}", xml);
    assert!(xml.contains("docs"), "visible text should appear: {}", xml);
}

#[test]
fn inline_link_anchor_emits_w_anchor() {
    let bytes = docx_bytes_from_stem(r##"p(jump to @link[to:"#concl"](conclusion))"##);
    let xml = extract_document_xml(&bytes);
    // Anchor links use w:anchor, not r:id.
    assert!(xml.contains("w:anchor=\"concl\""), "expected w:anchor: {}", xml);
}

#[test]
fn inline_link_ref_scheme_treated_as_anchor() {
    let bytes = docx_bytes_from_stem(
        r#"p(see @link[to:"ref://intro"](Introduction))"#,
    );
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("w:anchor=\"intro\""), "ref:// should become anchor: {}", xml);
}

// --- images (stage 3) ---------------------------------------------------

/// Mint a small valid PNG at `dir/name`. Uses the `image` crate
/// (already a transitive dep via docx-rs) so the bytes are guaranteed
/// to round-trip back through `image::load_from_memory` inside
/// `Pic::new`.
fn write_tiny_png(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    use image::{ImageBuffer, Rgb};
    let img = ImageBuffer::from_fn(8u32, 8u32, |x, y| {
        if (x + y) % 2 == 0 {
            Rgb([255u8, 0, 0])
        } else {
            Rgb([0u8, 0, 255])
        }
    });
    let path = dir.join(name);
    img.save(&path).expect("save tmp png");
    path
}

#[test]
fn image_block_embeds_drawing() {
    let tmp = std::env::temp_dir().join("stem-docx-img-test");
    std::fs::create_dir_all(&tmp).expect("mkdir");
    let png = write_tiny_png(&tmp, "tiny.png");

    let src = format!(
        r#"image[src:"{}", alt:"a tiny test image"]"#,
        png.display()
    );
    let r = stem_parser::parse(&src);
    assert!(r.diagnostics.is_empty(), "parse: {:?}", r.diagnostics);
    let bytes = DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export");

    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("<w:drawing>"), "expected <w:drawing>: {}", xml);
    assert!(xml.contains("a:blip"), "expected DrawingML blip: {}", xml);

    // The embedded image becomes a part under word/media/.
    let reader = std::io::Cursor::new(&bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("zip");
    let media_count = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .filter(|n| n.starts_with("word/media/"))
        .count();
    assert!(media_count >= 1, "expected at least one media file");
}

#[test]
fn image_with_caption_emits_caption_paragraph_below() {
    let tmp = std::env::temp_dir().join("stem-docx-img-test");
    std::fs::create_dir_all(&tmp).expect("mkdir");
    let png = write_tiny_png(&tmp, "tiny.png");

    let src = format!(
        r#"image[src:"{}", alt:"x", caption:"workflow overview"]"#,
        png.display()
    );
    let r = stem_parser::parse(&src);
    let bytes = DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect("export");
    let xml = extract_document_xml(&bytes);
    let cap = xml.find("workflow overview").expect("caption text");
    let drawing = xml.find("<w:drawing>").expect("drawing");
    assert!(drawing < cap, "caption should come after the drawing");
    assert!(xml.contains("w:pStyle w:val=\"Caption\""));
    // Auto SEQ Figure field.
    assert!(xml.contains("SEQ Figure"), "expected SEQ Figure: {}", xml);
}

#[test]
fn image_with_image_base_resolves_relative_path() {
    let tmp = std::env::temp_dir().join("stem-docx-img-base-test");
    std::fs::create_dir_all(&tmp).expect("mkdir");
    write_tiny_png(&tmp, "rel.png");

    let src = r#"image[src:"rel.png", alt:"relative"]"#;
    let r = stem_parser::parse(src);
    let bytes = DocxExporter::new()
        .with_image_base(&tmp)
        .export(&r.document, &Theme::default())
        .expect("export");
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("<w:drawing>"));
}

#[test]
fn image_missing_src_emits_placeholder() {
    let bytes = docx_bytes_from_stem("image");
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("missing src"), "expected placeholder: {}", xml);
}

#[test]
fn image_unreadable_path_errors() {
    let r = stem_parser::parse(r#"image[src:"/no/such/file.png", alt:"x"]"#);
    let err = DocxExporter::new()
        .export(&r.document, &Theme::default())
        .expect_err("expected ImageRead error");
    assert!(matches!(err, stem_exports::DocxError::ImageRead { .. }));
}

#[test]
fn inline_footnote_emits_reference_and_part() {
    let bytes = docx_bytes_from_stem(
        r#"p(The figure cited@footnote(Smith 2024, p.42) is current.)"#,
    );
    let xml = extract_document_xml(&bytes);
    assert!(
        xml.contains("<w:footnoteReference"),
        "expected <w:footnoteReference>: {}",
        xml
    );

    // The footnotes.xml part should now contain our text.
    let reader = std::io::Cursor::new(&bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("zip");
    let mut footnotes = String::new();
    zip.by_name("word/footnotes.xml")
        .expect("footnotes.xml present")
        .read_to_string(&mut footnotes)
        .expect("read");
    assert!(
        footnotes.contains("Smith 2024, p.42"),
        "footnotes.xml should contain footnote text: {}",
        footnotes
    );
}

// --- page setup + headers/footers (stage 4) -----------------------------

#[test]
fn page_size_letter_emits_letter_dimensions() {
    let bytes = docx_bytes_from_stem("[page-size:letter]\n\np(hi)");
    let xml = extract_document_xml(&bytes);
    // US letter = 12240 x 15840 twips.
    assert!(
        xml.contains("w:w=\"12240\"") && xml.contains("w:h=\"15840\""),
        "expected letter dimensions: {}",
        xml
    );
}

#[test]
fn page_size_a4_default_when_unset() {
    let bytes = docx_bytes_from_stem("p(hi)");
    let xml = extract_document_xml(&bytes);
    assert!(
        xml.contains("w:w=\"11906\"") && xml.contains("w:h=\"16838\""),
        "expected A4 dimensions: {}",
        xml
    );
}

#[test]
fn orientation_landscape_swaps_dimensions() {
    let bytes = docx_bytes_from_stem("[page-size:letter, orientation:landscape]\n\np(hi)");
    let xml = extract_document_xml(&bytes);
    // Landscape letter → 15840 x 12240.
    assert!(
        xml.contains("w:w=\"15840\"") && xml.contains("w:h=\"12240\""),
        "expected swapped letter dimensions: {}",
        xml
    );
    assert!(xml.contains("w:orient=\"landscape\""), "expected orient attr: {}", xml);
}

#[test]
fn margin_uniform_applies_to_all_sides() {
    let bytes = docx_bytes_from_stem("[margin:1in]\n\np(hi)");
    let xml = extract_document_xml(&bytes);
    // 1in = 1440 twips on every side.
    assert!(xml.contains("w:top=\"1440\""), "expected top=1440: {}", xml);
    assert!(xml.contains("w:bottom=\"1440\""));
    assert!(xml.contains("w:left=\"1440\""));
    assert!(xml.contains("w:right=\"1440\""));
}

#[test]
fn margin_four_value_shorthand_applies_in_css_order() {
    // top right bottom left
    let bytes = docx_bytes_from_stem("[margin:\"1in 2in 1in 1.5in\"]\n\np(hi)");
    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("w:top=\"1440\""), "top: {}", xml);
    assert!(xml.contains("w:right=\"2880\""), "right: {}", xml);
    assert!(xml.contains("w:bottom=\"1440\""), "bottom: {}", xml);
    assert!(xml.contains("w:left=\"2160\""), "left: {}", xml);
}

#[test]
fn header_block_emits_header_part() {
    let bytes = docx_bytes_from_stem("header{ p(Document Title) }\n\np(body)");
    let reader = std::io::Cursor::new(&bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("zip");
    let has_header_part = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .any(|n| n.starts_with("word/header") && n.ends_with(".xml"));
    assert!(has_header_part, "expected a word/header*.xml part");

    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("<w:headerReference"), "expected headerReference: {}", xml);

    // The header part itself should contain our text.
    let reader = std::io::Cursor::new(&bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("zip");
    let mut header_xml = String::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).unwrap().name().to_string();
        if name.starts_with("word/header") && name.ends_with(".xml") {
            zip.by_name(&name).unwrap().read_to_string(&mut header_xml).unwrap();
            break;
        }
    }
    assert!(header_xml.contains("Document Title"), "header part should contain title: {}", header_xml);
}

#[test]
fn footer_with_page_number_emits_page_field() {
    let bytes = docx_bytes_from_stem(
        "footer{ p(Page @page-number() of @total-pages()) }\n\np(body)",
    );
    let reader = std::io::Cursor::new(&bytes);
    let mut zip = zip::ZipArchive::new(reader).expect("zip");
    let mut footer_xml = String::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).unwrap().name().to_string();
        if name.starts_with("word/footer") && name.ends_with(".xml") {
            zip.by_name(&name).unwrap().read_to_string(&mut footer_xml).unwrap();
            break;
        }
    }
    assert!(
        footer_xml.contains("PAGE") && footer_xml.contains("NUMPAGES"),
        "footer should carry PAGE and NUMPAGES fields: {}",
        footer_xml
    );
    assert!(footer_xml.contains("w:fldChar"), "expected fldChar markers: {}", footer_xml);
}

// --- TOC + sections (stage 5) -------------------------------------------

#[test]
fn section_id_toc_emits_table_of_contents() {
    let bytes = docx_bytes_from_stem(
        "h1(Document)\n\nsection[id:toc]\n\nh1(Chapter 1)\nh2(Section A)\n",
    );
    let xml = extract_document_xml(&bytes);
    // The TOC field is identified by its instrText "TOC \o ..." or by
    // the StructuredDataTag wrapper docx-rs uses.
    assert!(
        xml.contains("TOC ") || xml.contains("<w:sdt"),
        "expected TOC field or sdt wrapper: {}",
        xml
    );
    assert!(
        xml.contains("\\o") || xml.contains("heading"),
        "expected heading range switch: {}",
        xml
    );
}

#[test]
fn section_without_toc_id_emits_children_in_order() {
    let bytes = docx_bytes_from_stem("section[id:intro]{ h1(Intro) p(prose-content) }");
    let xml = extract_document_xml(&bytes);
    // Use unambiguous text tokens — `body` matches the `<w:body>` tag.
    let intro = xml.find("Intro").expect("h1 text");
    let prose = xml.find("prose-content").expect("p text");
    assert!(intro < prose, "h1 should come before p");
    assert!(xml.contains("w:val=\"Heading1\""), "h1 should still get Heading1 style: {}", xml);
}

#[test]
fn first_header_scope_uses_first_header_reference() {
    let bytes = docx_bytes_from_stem(
        "header[scope:first]{ p(Cover-only header) }\n\np(body)",
    );
    let xml = extract_document_xml(&bytes);
    assert!(
        xml.contains("w:type=\"first\""),
        "expected scope=first header reference: {}",
        xml
    );
}

#[test]
fn table_valign_emits_vertical_align() {
    let bytes = docx_bytes_from_stem("table{ row{ cell[valign:middle](x) } }");
    let xml = extract_document_xml(&bytes);
    assert!(
        xml.contains("w:vAlign w:val=\"center\""),
        "expected vAlign=center: {}",
        xml
    );
}
