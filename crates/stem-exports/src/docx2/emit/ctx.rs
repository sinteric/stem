//! Mutable context threaded through body emission.
//!
//! Tracks per-document side-state that the OOXML body needs to
//! reference but the document.xml.rels + media parts have to
//! produce: embedded images, external hyperlinks, footnotes,
//! bookmarks, etc. The body emitter populates the registries
//! while it walks the AST; the packager reads them when assembling
//! `word/_rels/document.xml.rels`, `word/media/imageN.*`, and the
//! header/footer/footnote parts.
//!
//! Threading this through the emit functions (rather than a
//! thread-local or a `RefCell`) keeps every body emitter pure on
//! its inputs, makes tests easy (just construct an `EmitCtx`), and
//! avoids the "where does this number come from?" debugging
//! pattern.

use std::path::{Path, PathBuf};

/// One image embedded in the docx. Populated by the drawing
/// emitter; consumed by the packager.
#[derive(Clone)]
pub struct EmbeddedImage {
    /// Relationship ID used in `<a:blip r:embed="rIdN"/>`.
    pub rid: String,
    /// Path inside the ZIP, e.g. `word/media/image1.png`.
    pub zip_path: String,
    /// Image bytes — written to the ZIP verbatim.
    pub bytes: Vec<u8>,
    /// File extension used in Content_Types and the part path
    /// (`"png"`, `"jpeg"`, `"gif"`).
    pub ext: String,
}

#[derive(Default)]
pub struct EmitCtx {
    /// Directory used to resolve relative `image[src:...]` paths.
    /// `None` means resolve against the process CWD.
    pub image_base: Option<PathBuf>,
    /// Embedded images, in the order they were emitted.
    pub images: Vec<EmbeddedImage>,
    /// Monotonic counter — every kind of rId draws from this
    /// space so a single `document.xml.rels` can list them all
    /// without collisions. Static parts (styles, numbering,
    /// theme, ...) reserve the first N entries; this counter
    /// starts after that reservation.
    next_rid: u32,
    /// Number used for the next `wp:docPr` / `pic:cNvPr` id.
    /// Independent from rIds.
    next_drawing_id: u32,
}

impl EmitCtx {
    /// Create a new context whose first allocated rId will be
    /// `rId{start_rid}`. The packager reserves the static-parts
    /// rIds before calling into body emission, so this starts
    /// counting at the next free slot.
    pub fn new(image_base: Option<&Path>, start_rid: u32) -> Self {
        Self {
            image_base: image_base.map(Path::to_path_buf),
            images: Vec::new(),
            next_rid: start_rid,
            next_drawing_id: 1,
        }
    }

    /// Allocate the next `rIdN` string.
    pub fn alloc_rid(&mut self) -> String {
        let n = self.next_rid;
        self.next_rid = n + 1;
        format!("rId{n}")
    }

    /// Allocate the next drawing id (wp:docPr / pic:cNvPr).
    pub fn alloc_drawing_id(&mut self) -> u32 {
        let id = self.next_drawing_id;
        self.next_drawing_id = id + 1;
        id
    }

    /// Register an embedded image; returns its allocated rId.
    pub fn add_image(&mut self, bytes: Vec<u8>, ext: &str) -> String {
        let n = self.images.len() + 1;
        let rid = self.alloc_rid();
        let zip_path = format!("word/media/image{n}.{ext}");
        self.images.push(EmbeddedImage {
            rid: rid.clone(),
            zip_path,
            bytes,
            ext: ext.to_string(),
        });
        rid
    }
}
