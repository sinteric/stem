//! Drawing emission — embedded images.
//!
//! Stem source:
//!
//!   `image[src:"logo.png", w:"3in", h:"2in", float:"anchor"]`
//!
//! Two layout modes:
//! - `inline` (default) — `<wp:inline>` with `<wp:extent>` only;
//!   image flows with the text.
//! - `anchor` — `<wp:anchor>` with `<wp:positionH>` /
//!   `<wp:positionV>` so the cover-page logo can land at an
//!   explicit page offset.
//!
//! The image bytes are read here, registered in the
//! [`EmitCtx::images`] registry, and emitted into the ZIP later
//! by [`super::super::parts::package_doc`].
//!
//! [`EmitCtx::images`]: super::ctx::EmitCtx::images

use std::path::{Path, PathBuf};

use stem_core::ast::Block;

use super::super::xml::XmlBuf;
use super::ctx::EmitCtx;

// EMU = English Metric Unit, OOXML's standard length unit.
// 914400 EMU per inch.
const EMU_PER_INCH: u32 = 914_400;
const EMU_PER_PT: u32 = 12_700;
const EMU_PER_CM: u32 = 360_000;
const EMU_PER_MM: u32 = 36_000;
const EMU_PER_PX: u32 = 9_525; // assumes 96 DPI

/// Render an `image` block. Reads bytes from disk, registers the
/// image in the context, and emits the `<w:p><w:r><w:drawing>…`
/// fragment (plus an optional Caption-styled paragraph).
///
/// On read failure, emits a placeholder `[image: <reason>]`
/// paragraph rather than aborting export — same behavior as the
/// current docx exporter.
pub fn render_image(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let Some(src) = b.prop_str("src") else {
        return placeholder(x, "missing src");
    };
    let path = resolve_image_path(src, ctx.image_base.as_deref());
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            return placeholder(x, &format!("failed to read {}: {e}", path.display()));
        }
    };
    let ext = detect_ext(&path, &bytes);

    // Native pixel dimensions. We don't decode here — the EMU
    // size is taken from the source's `w`/`h` properties, and
    // defaults to a sensible 3×2 inch if not provided. (The old
    // exporter used docx-rs's Pic::new which decoded to get
    // native dimensions; we avoid that dependency.)
    let (w_emu, h_emu) = resolve_size(b);
    let rid = ctx.add_image(bytes, ext);
    let drawing_id = ctx.alloc_drawing_id();
    let alt = b.prop_str("alt").unwrap_or("Image");

    let float_mode = b.prop_str("float").unwrap_or("inline");
    x.elem("w:p", &[], |x| {
        x.elem("w:r", &[], |x| {
            x.elem("w:drawing", &[], |x| {
                match float_mode {
                    "anchor" | "behind" => {
                        emit_anchor(x, &rid, drawing_id, alt, w_emu, h_emu, float_mode == "behind");
                    }
                    _ => emit_inline(x, &rid, drawing_id, alt, w_emu, h_emu),
                }
            });
        });
    });

    emit_caption(b, ctx, x);
}

fn placeholder(x: &mut XmlBuf, reason: &str) {
    let text = format!("[image: {reason}]");
    x.elem("w:p", &[], |x| {
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], &text, true);
        });
    });
}

fn emit_inline(
    x: &mut XmlBuf,
    rid: &str,
    drawing_id: u32,
    alt: &str,
    w_emu: u32,
    h_emu: u32,
) {
    x.elem(
        "wp:inline",
        &[
            ("xmlns:wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
            ("distT", "0"),
            ("distB", "0"),
            ("distL", "0"),
            ("distR", "0"),
        ],
        |x| {
            emit_extent(x, w_emu, h_emu);
            x.empty(
                "wp:effectExtent",
                &[("l", "0"), ("t", "0"), ("r", "0"), ("b", "0")],
            );
            emit_doc_pr(x, drawing_id, alt);
            emit_graphic(x, rid, drawing_id, alt, w_emu, h_emu);
        },
    );
}

fn emit_anchor(
    x: &mut XmlBuf,
    rid: &str,
    drawing_id: u32,
    alt: &str,
    w_emu: u32,
    h_emu: u32,
    behind: bool,
) {
    let behind_attr = if behind { "1" } else { "0" };
    x.elem(
        "wp:anchor",
        &[
            ("xmlns:wp", "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"),
            ("distT", "0"),
            ("distB", "0"),
            ("distL", "0"),
            ("distR", "0"),
            ("simplePos", "0"),
            ("relativeHeight", "1"),
            ("behindDoc", behind_attr),
            ("locked", "0"),
            ("layoutInCell", "1"),
            ("allowOverlap", "1"),
        ],
        |x| {
            x.empty("wp:simplePos", &[("x", "0"), ("y", "0")]);
            // Positioning: anchored to the page, centered
            // horizontally and 1in down from the top — matches
            // the cover-page logo placement of the BoringCrypto
            // reference.
            x.elem(
                "wp:positionH",
                &[("relativeFrom", "page")],
                |x| {
                    x.elem_text("wp:align", &[], "center", false);
                },
            );
            x.elem(
                "wp:positionV",
                &[("relativeFrom", "page")],
                |x| {
                    let y = (EMU_PER_INCH).to_string();
                    x.elem_text("wp:posOffset", &[], &y, false);
                },
            );
            emit_extent(x, w_emu, h_emu);
            x.empty(
                "wp:effectExtent",
                &[("l", "0"), ("t", "0"), ("r", "0"), ("b", "0")],
            );
            x.empty(
                "wp:wrapNone",
                &[],
            );
            emit_doc_pr(x, drawing_id, alt);
            emit_graphic(x, rid, drawing_id, alt, w_emu, h_emu);
        },
    );
}

fn emit_extent(x: &mut XmlBuf, w_emu: u32, h_emu: u32) {
    let cx = w_emu.to_string();
    let cy = h_emu.to_string();
    x.empty("wp:extent", &[("cx", &cx), ("cy", &cy)]);
}

fn emit_doc_pr(x: &mut XmlBuf, id: u32, alt: &str) {
    let id_s = id.to_string();
    x.empty(
        "wp:docPr",
        &[
            ("id", &id_s),
            ("name", &format!("Picture {id}")),
            ("descr", alt),
        ],
    );
    x.elem("wp:cNvGraphicFramePr", &[], |x| {
        x.empty(
            "a:graphicFrameLocks",
            &[
                (
                    "xmlns:a",
                    "http://schemas.openxmlformats.org/drawingml/2006/main",
                ),
                ("noChangeAspect", "1"),
            ],
        );
    });
}

fn emit_graphic(
    x: &mut XmlBuf,
    rid: &str,
    drawing_id: u32,
    alt: &str,
    w_emu: u32,
    h_emu: u32,
) {
    x.elem(
        "a:graphic",
        &[("xmlns:a", "http://schemas.openxmlformats.org/drawingml/2006/main")],
        |x| {
            x.elem(
                "a:graphicData",
                &[(
                    "uri",
                    "http://schemas.openxmlformats.org/drawingml/2006/picture",
                )],
                |x| {
                    x.elem(
                        "pic:pic",
                        &[(
                            "xmlns:pic",
                            "http://schemas.openxmlformats.org/drawingml/2006/picture",
                        )],
                        |x| {
                            emit_nv_pic_pr(x, drawing_id, alt);
                            emit_blip_fill(x, rid);
                            emit_sp_pr(x, w_emu, h_emu);
                        },
                    );
                },
            );
        },
    );
}

fn emit_nv_pic_pr(x: &mut XmlBuf, id: u32, alt: &str) {
    let id_s = id.to_string();
    x.elem("pic:nvPicPr", &[], |x| {
        x.empty(
            "pic:cNvPr",
            &[
                ("id", &id_s),
                ("name", &format!("Picture {id}")),
                ("descr", alt),
            ],
        );
        x.empty("pic:cNvPicPr", &[]);
    });
}

fn emit_blip_fill(x: &mut XmlBuf, rid: &str) {
    x.elem("pic:blipFill", &[], |x| {
        x.empty(
            "a:blip",
            &[
                (
                    "xmlns:r",
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
                ),
                ("r:embed", rid),
            ],
        );
        x.elem("a:stretch", &[], |x| {
            x.empty("a:fillRect", &[]);
        });
    });
}

fn emit_sp_pr(x: &mut XmlBuf, w_emu: u32, h_emu: u32) {
    let cx = w_emu.to_string();
    let cy = h_emu.to_string();
    x.elem("pic:spPr", &[], |x| {
        x.elem("a:xfrm", &[], |x| {
            x.empty("a:off", &[("x", "0"), ("y", "0")]);
            x.empty("a:ext", &[("cx", &cx), ("cy", &cy)]);
        });
        x.elem("a:prstGeom", &[("prst", "rect")], |x| {
            x.empty("a:avLst", &[]);
        });
    });
}

fn emit_caption(b: &Block, ctx: &mut EmitCtx, x: &mut XmlBuf) {
    let Some(text) = b.prop_str("caption") else {
        return;
    };
    ctx.figure_caption_seq += 1;
    let seq_n = ctx.figure_caption_seq;
    let bookmark = format!("_Toc_figure_{seq_n}");
    let bm_id = ctx.alloc_bookmark_id();
    let bm_id_s = bm_id.to_string();
    x.elem("w:p", &[], |x| {
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "Caption")]);
        });
        x.empty(
            "w:bookmarkStart",
            &[("w:id", &bm_id_s), ("w:name", &bookmark)],
        );
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], "Figure ", true);
        });
        super::field::render_seq("Figure", seq_n, x);
        x.elem("w:r", &[], |x| {
            x.elem_text("w:t", &[], &format!(". {text}"), true);
        });
        x.empty("w:bookmarkEnd", &[("w:id", &bm_id_s)]);
    });
}

/// Pick the file extension to use for the part path + content
/// type. Prefer the source path's suffix; fall back to magic
/// byte sniffing; default to png.
fn detect_ext(path: &Path, bytes: &[u8]) -> &'static str {
    if let Some(e) = path.extension().and_then(|s| s.to_str()) {
        match e.to_ascii_lowercase().as_str() {
            "png" => return "png",
            "jpg" | "jpeg" => return "jpeg",
            "gif" => return "gif",
            "bmp" => return "bmp",
            "tif" | "tiff" => return "tiff",
            _ => {}
        }
    }
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return "png";
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "jpeg";
    }
    if bytes.starts_with(b"GIF8") {
        return "gif";
    }
    "png"
}

fn resolve_image_path(src: &str, base: Option<&Path>) -> PathBuf {
    let p = Path::new(src);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    match base {
        Some(b) => b.join(p),
        None => p.to_path_buf(),
    }
}

fn resolve_size(b: &Block) -> (u32, u32) {
    let w = b.prop_str("w").and_then(parse_length_to_emu);
    let h = b.prop_str("h").and_then(parse_length_to_emu);
    // Default 3"×2" — middle-of-the-road figure size matching the
    // BoringCrypto cover logo. Either axis may be overridden;
    // when only one is set we keep the 3:2 ratio.
    let default_w = 3 * EMU_PER_INCH;
    let default_h = 2 * EMU_PER_INCH;
    match (w, h) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => {
            let h = (w as u64 * default_h as u64 / default_w as u64) as u32;
            (w, h)
        }
        (None, Some(h)) => {
            let w = (h as u64 * default_w as u64 / default_h as u64) as u32;
            (w, h)
        }
        (None, None) => (default_w, default_h),
    }
}

fn parse_length_to_emu(s: &str) -> Option<u32> {
    let s = s.trim();
    if s.ends_with('%') {
        return None;
    }
    let (num, unit) = split_length(s)?;
    let value: f64 = num.parse().ok()?;
    let emu_per_unit = match unit {
        "" | "px" => EMU_PER_PX as f64,
        "pt" => EMU_PER_PT as f64,
        "in" => EMU_PER_INCH as f64,
        "cm" => EMU_PER_CM as f64,
        "mm" => EMU_PER_MM as f64,
        _ => return None,
    };
    let emu = (value * emu_per_unit).round();
    if emu < 0.0 || emu > u32::MAX as f64 {
        return None;
    }
    Some(emu as u32)
}

fn split_length(s: &str) -> Option<(&str, &str)> {
    let idx = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num, unit) = s.split_at(idx);
    if num.is_empty() {
        return None;
    }
    Some((num, unit))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use stem_parser::parse;

    use super::*;

    fn make_png() -> Vec<u8> {
        // Tiny 1×1 transparent PNG hand-crafted. Decoded by Word
        // happily as a placeholder.
        // Source: standard 67-byte 1×1 PNG fixture.
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    fn write_png(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(&make_png()).unwrap();
        p
    }

    fn first_block(src: &str) -> Block {
        let r = parse(src);
        r.document.blocks.first().unwrap().clone()
    }

    fn render(b: &Block, ctx: &mut EmitCtx) -> String {
        let mut x = XmlBuf::new();
        render_image(b, ctx, &mut x);
        x.finish()
    }

    #[test]
    fn missing_src_emits_placeholder_paragraph() {
        let mut ctx = EmitCtx::new(None, 1);
        let b = first_block(r#"image[]"#);
        let s = render(&b, &mut ctx);
        assert!(s.contains("[image: missing src]"));
        assert!(ctx.images.is_empty());
    }

    #[test]
    fn missing_file_emits_placeholder_with_error_reason() {
        let mut ctx = EmitCtx::new(None, 1);
        let b = first_block(r#"image[src:"does-not-exist.png"]"#);
        let s = render(&b, &mut ctx);
        assert!(
            s.contains("failed to read") && s.contains("does-not-exist.png"),
            "got: {s}"
        );
        assert!(ctx.images.is_empty());
    }

    #[test]
    fn inline_image_registers_and_emits_drawing() {
        let dir = tempdir();
        let png = write_png(dir.path(), "logo.png");
        let png_str = png.to_string_lossy().into_owned();
        let src = format!(r#"image[src:"{png_str}", w:"2in", h:"1in"]"#);
        let b = first_block(&src);
        let mut ctx = EmitCtx::new(None, 5);
        let s = render(&b, &mut ctx);
        assert_eq!(ctx.images.len(), 1);
        assert_eq!(ctx.images[0].rid, "rId5");
        assert_eq!(ctx.images[0].zip_path, "word/media/image1.png");
        assert!(s.contains("<wp:inline"));
        assert!(s.contains(r#"r:embed="rId5""#));
        // 2in × 1in in EMU.
        assert!(s.contains(r#"cx="1828800""#));
        assert!(s.contains(r#"cy="914400""#));
    }

    #[test]
    fn anchored_image_emits_wp_anchor_with_position() {
        let dir = tempdir();
        let png = write_png(dir.path(), "logo.png");
        let png_str = png.to_string_lossy().into_owned();
        let src = format!(r#"image[src:"{png_str}", float:"anchor"]"#);
        let b = first_block(&src);
        let mut ctx = EmitCtx::new(None, 5);
        let s = render(&b, &mut ctx);
        assert!(s.contains("<wp:anchor"));
        assert!(s.contains(r#"<wp:positionH relativeFrom="page">"#));
        assert!(s.contains(r#"<wp:positionV relativeFrom="page">"#));
        assert!(s.contains(r#"behindDoc="0""#));
    }

    #[test]
    fn behind_image_sets_behindDoc_1() {
        let dir = tempdir();
        let png = write_png(dir.path(), "logo.png");
        let png_str = png.to_string_lossy().into_owned();
        let src = format!(r#"image[src:"{png_str}", float:"behind"]"#);
        let b = first_block(&src);
        let mut ctx = EmitCtx::new(None, 1);
        let s = render(&b, &mut ctx);
        assert!(s.contains(r#"behindDoc="1""#));
    }

    #[test]
    fn caption_property_emits_caption_paragraph_below() {
        let dir = tempdir();
        let png = write_png(dir.path(), "logo.png");
        let png_str = png.to_string_lossy().into_owned();
        let src = format!(r#"image[src:"{png_str}", caption:"Cover logo"]"#);
        let b = first_block(&src);
        let mut ctx = EmitCtx::new(None, 1);
        let s = render(&b, &mut ctx);
        let img = s.find("<w:drawing").unwrap();
        let cap = s.find(r#"<w:pStyle w:val="Caption"/>"#).unwrap();
        assert!(img < cap);
        assert!(s.contains("Cover logo"));
    }

    #[test]
    fn detect_ext_uses_magic_bytes_when_no_suffix() {
        let p = PathBuf::from("/tmp/anonymous");
        let png = make_png();
        assert_eq!(detect_ext(&p, &png), "png");
        let jpeg_magic: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_ext(&p, jpeg_magic), "jpeg");
    }

    #[test]
    fn parse_length_handles_pt_in_cm_px() {
        assert_eq!(parse_length_to_emu("72pt"), Some(72 * EMU_PER_PT));
        assert_eq!(parse_length_to_emu("1in"), Some(EMU_PER_INCH));
        assert_eq!(parse_length_to_emu("2.54cm"), Some((2.54 * EMU_PER_CM as f64).round() as u32));
        assert_eq!(parse_length_to_emu("96px"), Some(96 * EMU_PER_PX));
        assert_eq!(parse_length_to_emu("96"), Some(96 * EMU_PER_PX));
        assert_eq!(parse_length_to_emu("50%"), None);
    }

    fn tempdir() -> TempDir {
        TempDir::new()
    }

    /// Tiny self-cleaning temp dir helper (we don't want to add the
    /// `tempfile` crate just for image tests).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let p = std::env::temp_dir().join(format!("docx2-test-{pid}-{nanos}-{n}"));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
