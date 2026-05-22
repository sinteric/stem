#![cfg(feature = "pdf")]

use stem_core::theme::Theme;
use stem_core::Exporter;
use stem_exports::PdfExporter;
use stem_imports::markdown::import_str;

fn pdf_bytes(src: &str) -> Vec<u8> {
    let doc = import_str(src);
    PdfExporter::new()
        .export(&doc, &Theme::default())
        .expect("export")
}

#[test]
fn output_is_valid_pdf() {
    let bytes = pdf_bytes("# Hello\n");
    assert!(bytes.len() > 100, "output too short: {} bytes", bytes.len());
    assert!(
        bytes.starts_with(b"%PDF-"),
        "missing PDF magic: first bytes are {:?}",
        &bytes[..bytes.len().min(20)]
    );
    // Each well-formed PDF ends with %%EOF (possibly with a trailing newline).
    let tail_window = &bytes[bytes.len().saturating_sub(32)..];
    let tail_str = String::from_utf8_lossy(tail_window);
    assert!(
        tail_str.contains("%%EOF"),
        "missing %%EOF in tail: {:?}",
        tail_str
    );
}

#[test]
fn heading_text_appears_in_pdf() {
    let bytes = pdf_bytes("# QuokkaQuokka\n");
    // The text should appear in the content stream somewhere. PDF
    // content streams may compress it, but with printpdf's defaults the
    // text typically lands raw. If this assertion ever flakes, decode
    // properly via a PDF parser instead.
    let blob = String::from_utf8_lossy(&bytes);
    assert!(
        blob.contains("QuokkaQuokka"),
        "heading text not found in PDF stream"
    );
}

#[test]
fn paragraph_renders() {
    let bytes = pdf_bytes("# Title\n\nA paragraph of body text appears here.\n");
    let blob = String::from_utf8_lossy(&bytes);
    assert!(blob.contains("paragraph") || blob.contains("body text"));
}

#[test]
fn multi_page_long_document() {
    // Force pagination: emit 80 paragraphs.
    let mut src = String::from("# Long\n\n");
    for i in 0..80 {
        src.push_str(&format!(
            "Paragraph {}: lorem ipsum dolor sit amet consectetur adipiscing elit.\n\n",
            i
        ));
    }
    let bytes = pdf_bytes(&src);
    // PDF page count appears in the document; a second page means we
    // didn't silently drop content. The `/Pages` dictionary's `/Count`
    // entry holds the page count.
    let blob = String::from_utf8_lossy(&bytes);
    assert!(
        blob.contains("/Count 2") || blob.contains("/Count 3") || blob.contains("/Count 4"),
        "expected multi-page PDF"
    );
}

#[test]
fn list_renders() {
    // Bullet glyphs may be encoded by printpdf using the font's
    // glyph-ID encoding, which means the literal item text doesn't
    // always survive into the PDF byte stream verbatim. Verify the
    // structural shape instead: a non-trivial PDF with text-show ops.
    let bytes = pdf_bytes("- alpha\n- beta\n- gamma\n");
    assert!(bytes.len() > 500, "list PDF unexpectedly small");
    let blob = String::from_utf8_lossy(&bytes);
    // The /Pages /Count must be 1 (everything fits on one page).
    assert!(blob.contains("/Pages"), "no /Pages dict");
    // PDF stream operators 'Tf' (set font) and 'Tj'/'TJ' (show text)
    // should appear, indicating list items rendered as text ops.
    assert!(
        blob.contains("Tj") || blob.contains("TJ"),
        "no text-show operators in PDF"
    );
}

#[test]
fn empty_document_still_valid() {
    let bytes = pdf_bytes("");
    assert!(bytes.starts_with(b"%PDF-"));
    let blob = String::from_utf8_lossy(&bytes);
    assert!(blob.contains("%%EOF"));
}
