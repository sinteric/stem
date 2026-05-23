//! `[Content_Types].xml` — declares the MIME type of every part in
//! the package. Word refuses to open a docx whose parts aren't
//! registered here, so this list must stay in sync with what we
//! actually emit.

use super::super::xml::XmlBuf;

const NS_CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";

const DOC_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

/// Content types for the minimal empty docx (task 1 scaffold).
/// Subsequent tasks register styles, numbering, theme, header/footer,
/// images, etc. by calling [`builder`] and chaining `.override_part`.
pub fn minimal() -> String {
    builder().override_part("/word/document.xml", DOC_MAIN).finish()
}

/// Builder for the content-types part. The output emits the two
/// default `<Default>` rules for `rels` and `xml`, then the
/// `<Override>` rules in insertion order.
pub fn builder() -> ContentTypes {
    ContentTypes::new()
}

#[derive(Default)]
pub struct ContentTypes {
    extra_defaults: Vec<(String, String)>,
    overrides: Vec<(String, String)>,
}

impl ContentTypes {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn override_part(mut self, part_name: &str, content_type: &str) -> Self {
        self.overrides
            .push((part_name.to_string(), content_type.to_string()));
        self
    }

    /// Add a `<Default Extension="…" ContentType="…"/>` entry —
    /// used for image extensions (png/jpeg/gif/etc.) so Word can
    /// dispatch any `word/media/imageN.<ext>` part to the right
    /// decoder.
    pub fn default_extension(mut self, ext: &str, content_type: &str) -> Self {
        self.extra_defaults
            .push((ext.to_string(), content_type.to_string()));
        self
    }

    pub fn finish(self) -> String {
        let mut x = XmlBuf::new();
        x.xml_decl();
        // `<Types xmlns="...">` — default namespace passed as a
        // normal attribute; the XML 1.0 spec accepts `xmlns` as
        // attribute-syntactic without needing special builder
        // support.
        x.elem("Types", &[("xmlns", NS_CT)], |x| {
            x.empty(
                "Default",
                &[
                    ("Extension", "rels"),
                    (
                        "ContentType",
                        "application/vnd.openxmlformats-package.relationships+xml",
                    ),
                ],
            );
            x.empty(
                "Default",
                &[("Extension", "xml"), ("ContentType", "application/xml")],
            );
            for (ext, ct) in &self.extra_defaults {
                x.empty(
                    "Default",
                    &[("Extension", ext.as_str()), ("ContentType", ct.as_str())],
                );
            }
            for (part, ct) in &self.overrides {
                x.empty(
                    "Override",
                    &[("PartName", part.as_str()), ("ContentType", ct.as_str())],
                );
            }
        });
        x.finish()
    }
}
