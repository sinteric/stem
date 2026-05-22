//! ZIP packaging of OOXML parts into a `.docx` byte stream.
//!
//! A `.docx` file is an OPC ZIP whose entries are the OOXML parts.
//! All entries use deflate compression except `[Content_Types].xml`
//! by convention (we still compress it — Word accepts either).
//!
//! Task 2 in the migration plan replaces this with the full
//! packager. Here we only need to write the byte parts produced by
//! `parts::minimal_empty_doc` so task 1 produces an openable file.

use std::io::{Cursor, Write};

use super::DocxV2Error;

/// One OOXML part: zip path (e.g. `word/document.xml`) and its bytes.
pub struct Part {
    pub path: &'static str,
    pub data: Vec<u8>,
}

impl Part {
    pub fn new(path: &'static str, data: Vec<u8>) -> Self {
        Self { path, data }
    }
    pub fn from_str(path: &'static str, data: String) -> Self {
        Self {
            path,
            data: data.into_bytes(),
        }
    }
}

pub fn pack(parts: &[Part]) -> Result<Vec<u8>, DocxV2Error> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for part in parts {
            zip.start_file(part.path, opts)
                .map_err(|e| DocxV2Error::Pack(format!("zip start {}: {}", part.path, e)))?;
            zip.write_all(&part.data)
                .map_err(|e| DocxV2Error::Pack(format!("zip write {}: {}", part.path, e)))?;
        }
        zip.finish()
            .map_err(|e| DocxV2Error::Pack(format!("zip finish: {}", e)))?;
    }
    Ok(buf)
}
