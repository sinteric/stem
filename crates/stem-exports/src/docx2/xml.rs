//! String-based XML serializer for OOXML parts.
//!
//! Design notes:
//! - Closure-based: every element with children is emitted via
//!   `elem(name, attrs, |x| { ... })` so the opening and closing
//!   tags are guaranteed to match. There is no manual `close()` to
//!   forget.
//! - No reordering, no normalization. Child order is exactly what
//!   the caller emits. The whole reason we're moving off `docx-rs`
//!   is that OOXML's schema is order-sensitive (e.g. `<w:pPr>`
//!   wants `<w:pStyle>` first, `<w:rPr>` last) and an upstream
//!   "smart" serializer is worse than a dumb one here.
//! - No indentation. Word strips ignorable whitespace, but inside
//!   `<w:t>` runs leading/trailing whitespace is significant unless
//!   `xml:space="preserve"` is set. Flat output keeps that boundary
//!   unambiguous and shaves a few KB per part.
//! - Correct escaping of element text and attribute values. Quotes
//!   are not escaped in element text (they aren't special there) and
//!   `&apos;` is used for `'` in attribute values for safety even
//!   though we always emit double-quoted attributes.
//!
//! The builder doesn't know anything about OOXML — it's a generic
//! XML serializer. Per-element schemas live in `parts/` and `emit/`.

/// In-memory XML buffer. Construct, append, then call [`finish`] to
/// get the underlying `String`.
///
/// [`finish`]: XmlBuf::finish
pub struct XmlBuf {
    buf: String,
}

/// A namespace declaration emitted on an element (typically the
/// root). `prefix` is the part after `xmlns:` (e.g. `"w"`), `uri` is
/// the namespace URI.
pub struct Ns<'a> {
    pub prefix: &'a str,
    pub uri: &'a str,
}

impl Default for XmlBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlBuf {
    pub fn new() -> Self {
        Self {
            buf: String::with_capacity(2048),
        }
    }

    /// Emit the XML declaration. Always UTF-8, always `standalone="yes"`
    /// — OOXML parts use this exact prologue.
    pub fn xml_decl(&mut self) -> &mut Self {
        self.buf
            .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        self
    }

    /// Open `<name attrs ...>`, run the closure for children, then
    /// close `</name>`. Use this for any element that has children
    /// — even one text run is "children".
    pub fn elem(
        &mut self,
        name: &str,
        attrs: &[(&str, &str)],
        f: impl FnOnce(&mut XmlBuf),
    ) -> &mut Self {
        self.write_open_tag(name, attrs, &[], false);
        f(self);
        self.buf.push_str("</");
        self.buf.push_str(name);
        self.buf.push('>');
        self
    }

    /// Open `<name attrs ...>` with extra namespace declarations
    /// (typically used on the root element of each part). Children
    /// then proceed as with [`elem`].
    ///
    /// [`elem`]: XmlBuf::elem
    pub fn elem_with_ns(
        &mut self,
        name: &str,
        nss: &[Ns<'_>],
        attrs: &[(&str, &str)],
        f: impl FnOnce(&mut XmlBuf),
    ) -> &mut Self {
        self.write_open_tag(name, attrs, nss, false);
        f(self);
        self.buf.push_str("</");
        self.buf.push_str(name);
        self.buf.push('>');
        self
    }

    /// Self-closing element: `<name attrs .../>`. Use this for the
    /// many empty OOXML markers (`<w:b/>`, `<w:pStyle w:val="..."/>`,
    /// `<w:br w:type="page"/>`, etc.).
    pub fn empty(&mut self, name: &str, attrs: &[(&str, &str)]) -> &mut Self {
        self.write_open_tag(name, attrs, &[], true);
        self
    }

    /// Element wrapping a single text node:
    /// `<name attrs ...>escaped text</name>`. If `preserve` is true,
    /// emits `xml:space="preserve"` so Word keeps leading/trailing
    /// whitespace — required on `<w:t>` for any run that ends in a
    /// space.
    pub fn elem_text(
        &mut self,
        name: &str,
        attrs: &[(&str, &str)],
        text: &str,
        preserve: bool,
    ) -> &mut Self {
        self.buf.push('<');
        self.buf.push_str(name);
        for (k, v) in attrs {
            self.write_attr(k, v);
        }
        if preserve {
            self.buf.push_str(" xml:space=\"preserve\"");
        }
        self.buf.push('>');
        self.write_escaped_text(text);
        self.buf.push_str("</");
        self.buf.push_str(name);
        self.buf.push('>');
        self
    }

    /// Append text as a child of the current element, escaping XML
    /// specials. Use this only inside an [`elem`] closure that
    /// expects text content; most OOXML text goes through
    /// [`elem_text`] instead.
    ///
    /// [`elem`]: XmlBuf::elem
    /// [`elem_text`]: XmlBuf::elem_text
    pub fn text(&mut self, s: &str) -> &mut Self {
        self.write_escaped_text(s);
        self
    }

    /// Append raw, already-well-formed XML. Trapdoor for hand-built
    /// fragments. Prefer [`elem`] / [`empty`] / [`elem_text`] —
    /// `raw` exists for cases like splicing in a prebuilt drawing
    /// or a pre-rendered headerN.xml snippet.
    ///
    /// [`elem`]: XmlBuf::elem
    /// [`empty`]: XmlBuf::empty
    /// [`elem_text`]: XmlBuf::elem_text
    pub fn raw(&mut self, xml: &str) -> &mut Self {
        self.buf.push_str(xml);
        self
    }

    /// Consume the buffer and return the serialized XML.
    pub fn finish(self) -> String {
        self.buf
    }

    fn write_open_tag(
        &mut self,
        name: &str,
        attrs: &[(&str, &str)],
        nss: &[Ns<'_>],
        self_closing: bool,
    ) {
        self.buf.push('<');
        self.buf.push_str(name);
        for ns in nss {
            self.buf.push_str(" xmlns:");
            self.buf.push_str(ns.prefix);
            self.buf.push_str("=\"");
            // Namespace URIs in well-known OOXML are plain ASCII;
            // escape anyway in case a caller passes something
            // unusual.
            self.write_escaped_attr(ns.uri);
            self.buf.push('"');
        }
        for (k, v) in attrs {
            self.write_attr(k, v);
        }
        if self_closing {
            self.buf.push_str("/>");
        } else {
            self.buf.push('>');
        }
    }

    fn write_attr(&mut self, k: &str, v: &str) {
        self.buf.push(' ');
        self.buf.push_str(k);
        self.buf.push_str("=\"");
        self.write_escaped_attr(v);
        self.buf.push('"');
    }

    fn write_escaped_text(&mut self, s: &str) {
        for ch in s.chars() {
            match ch {
                '&' => self.buf.push_str("&amp;"),
                '<' => self.buf.push_str("&lt;"),
                '>' => self.buf.push_str("&gt;"),
                _ => self.buf.push(ch),
            }
        }
    }

    fn write_escaped_attr(&mut self, s: &str) {
        for ch in s.chars() {
            match ch {
                '&' => self.buf.push_str("&amp;"),
                '<' => self.buf.push_str("&lt;"),
                '>' => self.buf.push_str("&gt;"),
                '"' => self.buf.push_str("&quot;"),
                '\'' => self.buf.push_str("&apos;"),
                _ => self.buf.push(ch),
            }
        }
    }
}

/// Shorthand: produce a complete XML document by emitting the
/// XML declaration plus a single root element via the closure.
pub fn document(root: &str, nss: &[Ns<'_>], f: impl FnOnce(&mut XmlBuf)) -> String {
    let mut x = XmlBuf::new();
    x.xml_decl();
    x.elem_with_ns(root, nss, &[], f);
    x.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_root_serializes() {
        let s = document(
            "Types",
            &[Ns {
                prefix: "x",
                uri: "urn:test",
            }],
            |_| {},
        );
        assert!(s.starts_with("<?xml"));
        assert!(s.contains("<Types xmlns:x=\"urn:test\"></Types>"));
    }

    #[test]
    fn nested_elements_emit_in_order() {
        let mut x = XmlBuf::new();
        x.elem("w:pPr", &[], |x| {
            x.empty("w:pStyle", &[("w:val", "Heading1")]);
            x.empty("w:rPr", &[]);
        });
        let s = x.finish();
        // pStyle must come before rPr (the order we wrote).
        let p = s.find("w:pStyle").unwrap();
        let r = s.find("w:rPr").unwrap();
        assert!(p < r, "got: {s}");
    }

    #[test]
    fn text_escapes_specials() {
        let mut x = XmlBuf::new();
        x.elem_text("w:t", &[], "a < b & c > d", false);
        assert_eq!(x.finish(), "<w:t>a &lt; b &amp; c &gt; d</w:t>");
    }

    #[test]
    fn attrs_escape_quotes_and_specials() {
        let mut x = XmlBuf::new();
        x.empty("a", &[("href", r#"http://x/?q=1&r="hi""#)]);
        assert_eq!(
            x.finish(),
            r#"<a href="http://x/?q=1&amp;r=&quot;hi&quot;"/>"#
        );
    }

    #[test]
    fn preserve_attr_is_written_for_whitespace_runs() {
        let mut x = XmlBuf::new();
        x.elem_text("w:t", &[], " leading", true);
        assert_eq!(
            x.finish(),
            r#"<w:t xml:space="preserve"> leading</w:t>"#
        );
    }

    #[test]
    fn raw_is_pasted_verbatim() {
        let mut x = XmlBuf::new();
        x.elem("w:p", &[], |x| {
            x.raw("<w:r><w:t>hi</w:t></w:r>");
        });
        assert_eq!(x.finish(), "<w:p><w:r><w:t>hi</w:t></w:r></w:p>");
    }

    #[test]
    fn attr_order_is_preserved() {
        let mut x = XmlBuf::new();
        x.empty("w:pgMar", &[("w:top", "1440"), ("w:bottom", "1440")]);
        assert_eq!(x.finish(), r#"<w:pgMar w:top="1440" w:bottom="1440"/>"#);
    }
}
