# Implementing a Stem importer or exporter

This is a guide for adding a new format-side module to `stem-imports`
or `stem-exports`. It documents the conventions established by the
existing implementations (markdown, html, pdf, docx, xlsx) so the next
ones (pptx, hwpx, hwp, image, …) land consistently.

Read `docs/architecture.md` first for the big picture.

---

## Pick a side

Each format module lives on one side only:

- **Importer**: format → Stem `Document`. Lives in `crates/stem-imports/src/<format>.rs`.
- **Exporter**: Stem `Document` → format. Lives in `crates/stem-exports/src/<format>.rs`.

If you're shipping both directions for the same format, write two
modules. They share no code — different libraries, different concerns,
often different shipping cadence.

## File layout

```
crates/
  stem-imports/
    src/
      lib.rs           ← declare `pub mod <format>` under #[cfg(feature = "<format>")]
      <format>.rs      ← your importer
    tests/
      <format>.rs      ← integration tests
  stem-exports/
    src/
      lib.rs           ← declare `pub mod <format>` under #[cfg(feature = "<format>")]
      <format>.rs      ← your exporter
    tests/
      <format>.rs      ← integration tests
```

A format module that grows past ~500 LOC can become `<format>/mod.rs`
with submodules (e.g. `<format>/import.rs`, `<format>/elements.rs`).
Start as a single file; split only when navigation suffers.

## Cargo feature

Each module gates itself behind a feature with the same name as the
module. In the crate's `Cargo.toml`:

```toml
[features]
default = [...]                    # whatever you want compiled by default
<format> = ["dep:<crate>"]         # bring in the format library
```

The `dep:` prefix keeps the dependency optional (it's only pulled in
when the feature is enabled).

## Implement the trait

### Importer

```rust
use stem_core::ast::{Block, Body, Document, Metadata, Property, PropertyValue, TextPiece};
use stem_core::span::Span;
use stem_core::Importer;
use thiserror::Error;

#[derive(Default)]
pub struct MyImporter;

impl MyImporter {
    pub fn new() -> Self { Self }
}

#[derive(Debug, Error)]
pub enum MyImportError {
    #[error("parse: {0}")]
    Parse(String),
}

impl Importer for MyImporter {
    type Input = &'static str; // or Vec<u8> for binary formats
    type Error = MyImportError;
    fn import(&self, input: Self::Input) -> Result<Document, Self::Error> {
        // … build a Document …
    }
}
```

For binary formats use `Vec<u8>` (or `&[u8]` if you can do zero-copy).

A practical wrinkle: the `Importer` trait has a single associated
`Input` type, but you often want `&str` (any lifetime) rather than
`&'static str`. The markdown importer exposes a plain function
`pub fn import_str(src: &str) -> Document` alongside the trait impl,
and consumers prefer that. Do the same.

### Exporter

```rust
use stem_core::ast::Document;
use stem_core::theme::Theme;
use stem_core::Exporter;
use thiserror::Error;

#[derive(Default)]
pub struct MyExporter;

impl MyExporter {
    pub fn new() -> Self { Self }
}

#[derive(Debug, Error)]
pub enum MyExportError {
    #[error("write: {0}")]
    Write(String),
}

impl Exporter for MyExporter {
    type Output = Vec<u8>; // or String for text formats
    type Error = MyExportError;
    fn export(&self, doc: &Document, theme: &Theme) -> Result<Vec<u8>, Self::Error> {
        let cooked = stem_parser::cook_document(doc);
        // … walk cooked.blocks, build the format, serialize to bytes …
    }
}
```

**Always call `stem_parser::cook_document(doc)` first.** It runs the
sheet desugar / cell merge / col/row/format cascade so you see a
normalized tree. Skipping it means cascade rules won't apply.

## Wire it up

In the crate's `lib.rs`:

```rust
#[cfg(feature = "<format>")]
pub mod <format>;
#[cfg(feature = "<format>")]
pub use <format>::My{Importer,Exporter};
```

If your format has helpful re-exports (specific errors, builders),
expose them via the module — but keep the prelude (`stem-imports::*`
or `stem-exports::*`) lean. Each format owns its module's namespace.

## Mapping Stem to your format

### General principles

- **Stem's vocabulary is settled.** Don't extend the AST to fit your
  format — map the format to what already exists. If the format has
  something Stem can't represent (e.g. drop caps, tracked changes),
  drop it on export with a diagnostic; on import, map to the closest
  Stem analog and emit `import.<format>.lossy.*` warnings.
- **Block names match docs/schema.md.** Headings are `h1`..`h6`,
  paragraphs are `p`, lists are `ol`/`ul`/`li`, tables are
  `table`/`row`/`cell`, sheet cells are `cell[at:...]`, etc.
- **Inline elements use `text`/`code`/`link`/`footnote`/`mention`/`math`/`formula`/`date`.**
  Inline styling lives in `@text[weight:bold, style:italic, decoration:strike]`.
- **For sheet documents, set metadata `type:sheet`.** Without it, the
  validator complains about `cell[at:X]` blocks at the top level.

### Common mapping table (importer → AST)

| Source feature | Stem mapping |
|---|---|
| Heading level N | `h{1..6}` block |
| Paragraph | `p` block with text body |
| Ordered list | `ol{ li(...) li(...) }` |
| Unordered list | `ul{ ... }` |
| Bold span | `@text[weight:bold](...)` in body |
| Italic span | `@text[style:italic](...)` |
| Strikethrough | `@text[decoration:strike](...)` |
| Inline code | `@code(...)` |
| Hyperlink | `@link[to:url](text)` |
| Code block | `code[lang:rust]("...")` block |
| Blockquote | `blockquote(...)` |
| Image | `image[src:..., alt:..., caption:...]` |
| Footnote | `@footnote(text)` (inline) |
| Horizontal rule | `hr` |
| Table | `table{ row{ cell(...) cell(...) } row{ ... } }` |
| Sheet cell formula | `cell[at:A1](@formula("SUM(B2:B5)"))` |
| Sheet cell value | `cell[at:A1](42)` |
| Page break | `pagebreak` |

When in doubt: round-trip a small example through the format's reader,
then through `MarkdownExporter` (a known-good exporter), and check the
shape.

### Building Block AST values

`Block` has the field `inline_form: bool`. Set it to `true` for any
block placed inside `TextPiece::Inline` (i.e. for inline elements
`@text`, `@link`, `@code`, …). Set it to `false` for top-level / nested
block positions.

`Property` value is a `PropertyValue::Bare(String)` for unquoted values
and `PropertyValue::Quoted(String)` for quoted. Use `Bare` unless the
content contains spaces, commas, quotes, or characters that need
escaping.

Spans on imported blocks should be `Span::default()` (the source isn't
Stem; there's no meaningful span). Errors from the format library are
attached to the document-level metadata span or to no span.

## Tests

Two patterns:

### For importers: structural assertions over the AST

```rust
let doc = import_str("# Hello\n");
assert_eq!(doc.blocks[0].name, "h1");
assert_eq!(doc.blocks[0].plain_text().unwrap().trim(), "Hello");
```

Followed by a round-trip end-to-end:

```rust
let html = HtmlExporter::fragment()
    .export(&doc, &Theme::default()).unwrap();
assert!(html.contains("<h1>Hello</h1>"));
```

The end-to-end test catches schema mismatches: if you generated a
block name that the validator rejects, the rendering pipeline will
either drop it or surface a `type.unknown_*` warning.

### For exporters: format magic bytes + content sniffs

Most binary formats have a magic prefix:

| Format | Magic | Method |
|---|---|---|
| PDF | `%PDF-` start, `%%EOF` near end | byte scan |
| docx, xlsx, pptx, hwpx | `PK\x03\x04` start (ZIP) | byte scan |
| HTML | text — assert on tag/attribute presence | string contains |
| Markdown | text — assert on prefix chars | string contains |

For ZIPs, you can also scan the byte stream for expected entry
filenames (e.g. `xl/worksheets/sheet1.xml` for xlsx, `word/document.xml`
for docx) without unzipping. That's enough to catch "valid file, wrong
content" regressions for MVP. Save proper round-trip tests through a
parser for when the exporter matures.

Importer/exporter tests live in `crates/stem-{imports,exports}/tests/<format>.rs`.
Both wrap in `#![cfg(feature = "<format>")]` so they're skipped when
the feature is off.

## Per-format quick notes

### docx (export: ✅ MVP, import: stub)

- Library: `docx-rs` 0.4.20.
- Lists need an `AbstractNumbering` definition registered on the `Docx`
  before referencing them from list-item paragraphs. The existing
  exporter pre-registers two definitions (id 1 = decimal, id 2 = bullet),
  each with 9 nesting levels. Reuse.
- Built-in heading styles are named `Heading1`..`Heading9`. Word
  recognizes them automatically.
- Inline run styling: `Run::new().bold().italic().strike()`. Mix freely.
- For importer: the docx file is a ZIP. Use `zip` + an XML parser. The
  body is in `word/document.xml`. Walk `<w:p>` (paragraphs) and `<w:r>`
  (runs). Styles in `word/styles.xml` map Heading1..6 back to h1..h6.
  Numbering definitions in `word/numbering.xml` identify lists.

### xlsx (export: ✅ MVP, import: stub)

- Library: `rust_xlsxwriter` 0.95.0.
- Each `sheet[id:..., name:...]` block becomes one worksheet via
  `workbook.add_worksheet()`. Name via `worksheet.set_name(...)`.
- Cell address parsing is in `stem-exports/src/xlsx.rs::parse_address`.
  Copy or extract to a shared helper if you reuse it.
- Formula cells use `worksheet.write_formula(row, col, "=SUM(A1:A10)")`.
  Strip any leading `=` from the formula body before passing — Excel
  expects exactly one `=` and rust_xlsxwriter prepends it.
- `fmt:currency`/`percent`/etc. are not yet mapped to Excel cell
  formats. To add: register a `Format` per fmt kind on the workbook,
  apply via `write_number_with_format`. Map `currency` → `"$#,##0.00"`,
  `percent` → `"0.00%"`, `date` → `"yyyy-mm-dd"`, etc.
- For importer: read the ZIP, parse `xl/worksheets/sheet*.xml`. Each
  `<c>` (cell) has an `r="A1"` address and a `<v>` (value) or `<f>`
  (formula). Strings are interned in `xl/sharedStrings.xml`; cell
  type `t="s"` means the value is a 0-based index into that table.

### pptx (export: stub, import: stub)

Library candidates (vet maturity):
- `simple_pptx` — basic
- `pptx-rs` — search for current state
- Hand-build via the OOXML structure (`ppt/slides/slide1.xml`, etc.)
  using `zip` + an XML writer.

The hard part isn't generating the XML — pptx is a zip of well-defined
XML. The hard part is layout: a slide has a `<p:sld>` with shapes
(`<p:sp>`) at absolute positions, each containing text frames
(`<p:txBody>`). Mapping `slide{ title(...) bullets{ item(...) } }` to
that requires picking layout templates (master slide + slide layout).

MVP scope: use the default layout master, place title at fixed position,
bullets in the body placeholder.

Reference: <https://learn.microsoft.com/en-us/openspecs/office_standards/ms-pptx/>

### hwpx (export: stub, import: stub)

HWPX is Hancom's modern OOXML-style format. It's a zip of XML, similar
to docx in structure.

Spec: <https://www.hancom.com/etc/hwpDownload.do> (OWPML spec — search
"HWPX open document format").

Implementation library: probably hand-rolled with `zip` + `quick-xml`.
There's a `rhwp` crate but it focuses on the legacy binary `.hwp`
format. HWPX is similar enough to docx structurally that the same
patterns apply.

Note: HWPX content is in `Contents/section0.xml` (not `word/document.xml`).
Element names start with `hp:` (HWPX namespace). Run formatting goes
in `<hp:run>` elements.

For the Korean market specifically, this is the differentiator —
Korean government, schools, and many businesses standardize on HWPX.

### hwp (legacy binary)

The legacy `.hwp` binary format. Use the `rhwp` crate.

This is much harder than HWPX. Binary structure, compressed streams,
proprietary encoding. Consider implementing HWPX first and treating
hwp as "we read what rhwp can give us; we don't write binary hwp."

### image (export: not built)

Stem AST → PNG or SVG, single page per document (or pagination as
separate images).

Approach A: render to SVG natively, then rasterize to PNG via
`resvg`. SVG is straightforward — flow text along a path manually
since SVG doesn't have native flow.

Approach B: render via the PDF exporter, then convert PDF→PNG (would
need a PDF renderer like `pdfium-render` — adds a heavy native dep).

Approach A wins for portability. Limit per-page output for now.

## Anti-patterns to avoid

- **Don't extend the Stem AST to fit your format.** If the format has
  something Stem can't represent, surface it as a diagnostic and move
  on. The whole point of one IR is that all formats negotiate to the
  same shape.
- **Don't bypass `cook_document` on the exporter side.** The cooked
  AST is what every existing exporter sees; importers should produce
  trees that are already in cooked form (no `fill`/`source`/cascade
  rule blocks).
- **Don't introduce a new error code stage.** Use the existing stages:
  `parse`, `type`, `formula`, `cook`, `render`. For format-specific
  warnings, use `import.<format>.<short_name>` or
  `export.<format>.<short_name>` as a scoped convention.
- **Don't ship the format library as a workspace dependency.** Keep it
  optional under the feature flag, so consumers that don't need the
  format don't pay the build cost.

## Sanity checklist before opening a PR

- [ ] Module gated by a Cargo feature with the same name as the module.
- [ ] `Importer` or `Exporter` impl present with correct associated types.
- [ ] `cook_document(doc)` called at the top of `export` (exporters only).
- [ ] Integration tests in `crates/stem-{imports,exports}/tests/<format>.rs`,
      gated by `#![cfg(feature = "<format>")]`.
- [ ] At least one round-trip test if both directions exist.
- [ ] At least one byte-magic test for binary formats.
- [ ] No new top-level error stages — reuse `parse|type|formula|cook|render`
      or scope under `import.<format>.*` / `export.<format>.*`.
- [ ] No new AST types, properties, or element names just for this format.
      If you need vocabulary, propose a `docs/schema.md` change first.
- [ ] `cargo test --features <format>` passes locally.
- [ ] `cargo build --all-features` passes locally.
