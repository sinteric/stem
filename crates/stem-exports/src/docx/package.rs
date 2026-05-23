//! OPC ZIP packaging.
//!
//! A `.docx` file is an Open Packaging Convention ZIP archive whose
//! entries are the OOXML parts plus their relationship descriptors.
//! Word doesn't care about deflate vs store for typical parts, so
//! every entry is deflated for size. Word also doesn't enforce a
//! particular entry order, but we keep them in the order callers
//! add them — easier to diff against a reference docx that way.

use std::io::{Cursor, Write};

use super::DocxError;

/// One entry in the package: the part's path inside the ZIP
/// (e.g. `word/document.xml`, `word/media/image1.png`) and its
/// already-serialized bytes.
pub struct Part {
    pub path: String,
    pub data: Vec<u8>,
}

impl Part {
    /// A part holding raw bytes — use for binary parts like
    /// embedded images.
    pub fn bytes(path: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            data: data.into(),
        }
    }

    /// A part holding XML or other text — convenience for the many
    /// XML parts that get built by [`super::xml::XmlBuf`] and
    /// arrive as a `String`.
    pub fn text(path: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            data: data.into().into_bytes(),
        }
    }
}

/// Accumulates parts and produces the final `.docx` byte stream.
/// Maintains insertion order — callers should add `[Content_Types].xml`
/// first by convention (though Word doesn't require it).
#[derive(Default)]
pub struct Package {
    parts: Vec<Part>,
}

impl Package {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, part: Part) -> &mut Self {
        self.parts.push(part);
        self
    }

    pub fn add_text(&mut self, path: impl Into<String>, xml: impl Into<String>) -> &mut Self {
        self.parts.push(Part::text(path, xml));
        self
    }

    pub fn add_bytes(&mut self, path: impl Into<String>, data: impl Into<Vec<u8>>) -> &mut Self {
        self.parts.push(Part::bytes(path, data));
        self
    }

    pub fn finish(self) -> Result<Vec<u8>, DocxError> {
        write_zip(&self.parts)
    }
}

/// One-shot pack of a slice of parts. Used by the task-1 minimal
/// scaffold; later parts of the pipeline build via [`Package`].
pub fn pack(parts: &[Part]) -> Result<Vec<u8>, DocxError> {
    write_zip(parts)
}

fn write_zip(parts: &[Part]) -> Result<Vec<u8>, DocxError> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for part in parts {
            zip.start_file(&part.path, opts)
                .map_err(|e| DocxError::Pack(format!("zip start {}: {}", part.path, e)))?;
            zip.write_all(&part.data)
                .map_err(|e| DocxError::Pack(format!("zip write {}: {}", part.path, e)))?;
        }
        zip.finish()
            .map_err(|e| DocxError::Pack(format!("zip finish: {}", e)))?;
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut out = Vec::new();
        for i in 0..archive.len() {
            let mut e = archive.by_index(i).unwrap();
            let name = e.name().to_string();
            let mut data = Vec::new();
            e.read_to_end(&mut data).unwrap();
            out.push((name, data));
        }
        out
    }

    #[test]
    fn package_writes_parts_in_insertion_order() {
        let mut pkg = Package::new();
        pkg.add_text("a.xml", "<a/>");
        pkg.add_text("b.xml", "<b/>");
        pkg.add_bytes("c.bin", vec![0u8, 1, 2]);
        let bytes = pkg.finish().unwrap();

        let entries = unzip(&bytes);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].0, "a.xml");
        assert_eq!(entries[1].0, "b.xml");
        assert_eq!(entries[2].0, "c.bin");
        assert_eq!(entries[0].1, b"<a/>");
        assert_eq!(entries[2].1, vec![0u8, 1, 2]);
    }

    #[test]
    fn one_shot_pack_round_trips() {
        let parts = vec![Part::text("doc.xml", "<root/>")];
        let bytes = pack(&parts).unwrap();
        let entries = unzip(&bytes);
        assert_eq!(entries[0].0, "doc.xml");
        assert_eq!(entries[0].1, b"<root/>");
    }
}
