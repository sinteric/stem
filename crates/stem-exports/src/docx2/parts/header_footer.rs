//! `word/headerN.xml` and `word/footerN.xml` parts.
//!
//! Each header/footer is a self-contained part that contains its
//! own paragraph content. `<w:sectPr>` in `document.xml` binds
//! them to the section via `<w:headerReference>` and
//! `<w:footerReference>` elements with rIds.
//!
//! For task 13 we emit one of each kind ("default" — used on all
//! pages). Word also supports "first" and "even" types for
//! distinguishing the title page or even/odd pages; we leave
//! those for a future polish pass.

use stem_core::ast::Block;

use super::super::emit::ctx::EmitCtx;
use super::super::emit::paragraph;
use super::super::xml::{Ns, XmlBuf};

const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";

/// Build a `word/headerN.xml` body. `blocks` are the children of
/// the source `header{...}` block (or empty for a blank header).
pub fn header(blocks: &[Block], ctx: &mut EmitCtx) -> String {
    build("w:hdr", blocks, ctx)
}

/// Build a `word/footerN.xml` body.
pub fn footer(blocks: &[Block], ctx: &mut EmitCtx) -> String {
    build("w:ftr", blocks, ctx)
}

fn build(root: &str, blocks: &[Block], ctx: &mut EmitCtx) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(
        root,
        &[
            Ns { prefix: "w", uri: NS_W },
            Ns { prefix: "r", uri: NS_R },
            Ns { prefix: "wp", uri: NS_WP },
            Ns { prefix: "a", uri: NS_A },
            Ns { prefix: "pic", uri: NS_PIC },
        ],
        &[],
        |x| {
            if blocks.is_empty() {
                // A header/footer part must contain at least one
                // paragraph; Word will refuse to open an empty one.
                x.empty("w:p", &[]);
                return;
            }
            for b in blocks {
                paragraph::render_block(b, ctx, x);
            }
        },
    );
    x.finish()
}
