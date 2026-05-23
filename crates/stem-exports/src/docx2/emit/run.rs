//! Runs — `<w:r>` + `<w:rPr>` emission from text pieces.
//!
//! Task 6 scope: emit a single run per block body holding the
//! flattened plain text. Inline pieces (`@b(...)`, `@i(...)`,
//! `@code(...)`, `@link(...)`, `@math(...)`, ...) are not yet
//! formatted — they contribute their text content but no run
//! properties. Task 7 expands this with rPr support.

use stem_core::ast::{Block, Body, TextPiece};

use super::super::xml::XmlBuf;

/// Append the run(s) for the body of `b` to `x`. For task 6, this
/// is always one plain `<w:r><w:t>...</w:t></w:r>` carrying the
/// block's flattened text.
pub fn render_body(b: &Block, x: &mut XmlBuf) {
    let text = flatten_body(b);
    if text.is_empty() {
        return;
    }
    render_plain_run(&text, x);
}

/// Walk a block's body and concatenate every literal + inline
/// piece's text into a single string. Inline blocks with children
/// recurse; inline blocks with text bodies append their text.
pub fn flatten_body(b: &Block) -> String {
    let mut out = String::new();
    flatten_into(b, &mut out);
    out
}

fn flatten_into(b: &Block, out: &mut String) {
    match &b.body {
        Body::None => {}
        Body::Text(pieces) => {
            for p in pieces {
                match p {
                    TextPiece::Literal { text, .. } => out.push_str(text),
                    TextPiece::Inline(inner) => flatten_into(inner, out),
                }
            }
        }
        Body::Children(blocks) => {
            // Children bodies aren't paragraph-shaped text, but when
            // an inline element happens to use a children-body shape
            // (rare) we still want its text contribution.
            for child in blocks {
                flatten_into(child, out);
            }
        }
    }
}

/// Emit a single `<w:r><w:t xml:space="preserve">…</w:t></w:r>`.
/// `preserve` is always true so leading/trailing whitespace is
/// kept — Word's default is to strip it.
pub fn render_plain_run(text: &str, x: &mut XmlBuf) {
    x.elem("w:r", &[], |x| {
        x.elem_text("w:t", &[], text, true);
    });
}

/// Emit `<w:r><w:br w:type="page"/></w:r>`. Used by pagebreak.
pub fn render_page_break(x: &mut XmlBuf) {
    x.elem("w:r", &[], |x| {
        x.empty("w:br", &[("w:type", "page")]);
    });
}

#[cfg(test)]
mod tests {
    use stem_parser::parse;

    use super::*;

    fn first_block(src: &str) -> Block {
        let r = parse(src);
        assert!(r.document.blocks.first().is_some());
        r.document.blocks.first().unwrap().clone()
    }

    #[test]
    fn flatten_collects_literal_text() {
        let b = first_block("p(hello world)");
        assert_eq!(flatten_body(&b), "hello world");
    }

    #[test]
    fn flatten_descends_into_inlines() {
        let b = first_block("p(hello @b(bold) world)");
        assert_eq!(flatten_body(&b), "hello bold world");
    }

    #[test]
    fn empty_body_renders_nothing() {
        let b = first_block("p()");
        let mut x = XmlBuf::new();
        render_body(&b, &mut x);
        assert_eq!(x.finish(), "");
    }

    #[test]
    fn plain_run_wraps_in_r_and_t_with_preserve() {
        let mut x = XmlBuf::new();
        render_plain_run("hi", &mut x);
        let s = x.finish();
        assert_eq!(s, r#"<w:r><w:t xml:space="preserve">hi</w:t></w:r>"#);
    }

    #[test]
    fn page_break_emits_w_br() {
        let mut x = XmlBuf::new();
        render_page_break(&mut x);
        assert_eq!(x.finish(), r#"<w:r><w:br w:type="page"/></w:r>"#);
    }
}
