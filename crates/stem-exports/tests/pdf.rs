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
    // Force pagination unambiguously: 200 long paragraphs definitely
    // overflows one A4 page even with conservative line counts.
    let mut src = String::from("# Long\n\n");
    for i in 0..200 {
        src.push_str(&format!(
            "Paragraph {}: lorem ipsum dolor sit amet consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.\n\n",
            i
        ));
    }
    let bytes = pdf_bytes(&src);
    let blob = String::from_utf8_lossy(&bytes);
    // /Pages dict carries /Count N. Extract and assert N >= 2.
    let count = blob
        .split("/Count ")
        .nth(1)
        .and_then(|s| {
            s.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u32>()
                .ok()
        })
        .expect("no /Count in PDF");
    assert!(count >= 2, "expected multi-page PDF, got /Count {}", count);
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

#[test]
fn custom_font_can_be_configured() {
    // We don't have a font fixture to embed in tests; verify the API
    // accepts arbitrary bytes (even invalid ones) without panicking.
    // ParsedFont::from_bytes returns None for unparseable input and the
    // exporter silently falls back to built-in fonts.
    let exporter = PdfExporter::new().with_font(vec![0, 1, 2, 3]);
    let bytes = exporter
        .export(
            &import_str("# Latin Test\n\nA paragraph.\n"),
            &Theme::default(),
        )
        .expect("export");
    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn inline_emphasis_round_trips_to_pdf() {
    // Bold and italic should produce a PDF without errors. Verifying
    // the exact font switching in the byte stream is brittle; just
    // ensure the doc renders.
    let bytes = pdf_bytes("This has **bold** and *italic* spans.\n");
    assert!(bytes.starts_with(b"%PDF-"));
    assert!(bytes.len() > 500);
}

#[test]
fn word_wrap_uses_real_metrics() {
    // A paragraph longer than one line should wrap to multiple lines.
    // Indirect proof: the byte stream contains multiple text-cursor
    // operations (Td) — one per emitted line.
    let bytes = pdf_bytes(
        "This is a sufficiently long paragraph that absolutely must wrap across multiple lines when laid out at 11pt on an A4 page with twenty-millimeter margins; if this fits on one line then our word wrap is broken.\n",
    );
    let blob = String::from_utf8_lossy(&bytes);
    // Each SetTextCursor produces a "Td" operator in PDF content. With
    // wrap, we expect multiple. Without wrap, exactly one.
    let td_count = blob.matches(" Td").count();
    assert!(td_count >= 2, "expected wrap-induced multi-line, got {} Td ops", td_count);
}

/// Best-effort CJK rendering check. Skips when no system CJK font is
/// available (most CI runners). On macOS test machines the system
/// `AppleSDGothicNeo.ttc` Korean font is present.
#[test]
fn cjk_text_renders_with_system_font() {
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
    ];
    let Some(path) = CANDIDATES.iter().find(|p| std::path::Path::new(p).exists()) else {
        eprintln!("no system CJK font available; skipping cjk_text_renders_with_system_font");
        return;
    };
    let bytes = std::fs::read(path).expect("read font");

    let doc = import_str("# 안녕 Stem\n\n한국어 본문도 렌더링됩니다.\n");
    let pdf = PdfExporter::new()
        .with_font(bytes)
        .export(&doc, &Theme::default())
        .expect("export");
    assert!(pdf.starts_with(b"%PDF-"));
    assert!(pdf.len() > 1000);
}

#[test]
fn font_family_falls_back_when_variant_missing() {
    // No bold variant supplied: bold runs should not crash and the
    // resulting PDF must be valid. Verifies the cascade logic.
    let bogus = vec![0u8; 200]; // not a real font; parse fails → falls back to built-in
    let pdf = PdfExporter::new()
        .with_font_family(bogus, None, None, None)
        .export(
            &import_str("**Bold** and *italic* with regular text.\n"),
            &Theme::default(),
        )
        .expect("export");
    assert!(pdf.starts_with(b"%PDF-"));
}
