# Stem — Architecture

```
                        ┌─────────────────┐
                        │ stem-parser     │   canonical: Stem source → AST
                        └────────┬────────┘
                                 │
   ┌─────────────────┐  source   │
   │  source (*.stem)│───────────┘
   └─────────────────┘
                                 │
   ┌─────────────────┐  external │
   │  stem-imports   │  formats  │
   │  ─ markdown     ├───────────┤
   │  ─ docx (stub)  │           │
   │  ─ xlsx (stub)  │           │
   │  ─ pptx (stub)  │           │
   │  ─ hwpx (stub)  │           │
   └─────────────────┘           ▼
                          ┌────────────┐
                          │   AST      │ ◀── one IR, source = canonical form
                          └─────┬──────┘
                                │
                                ▼
                          ┌─────────────────┐
                          │  stem-types     │ validate, schemas,
                          │  (validate)     │ formula parser/eval,
                          └────┬────────────┘ per-element vocabulary
                               │ diagnostics
        ┌──────────────────────┼────────────────────────┐
        ▼                      ▼                        ▼
   ┌──────────────┐    ┌──────────────┐         ┌────────────┐
   │ stem-exports │    │   stem-lsp   │         │  stem-cli  │
   │  ─ html      │    │ diagnostics, │         │ parse/check│
   │  ─ markdown  │    │ completion   │         │   render   │
   │  ─ pdf       │    └──────────────┘         └────────────┘
   │  ─ docx      │
   │  ─ xlsx      │
   │  ─ pptx (stub)│
   │  ─ hwpx (stub)│
   │  ─ image (stub)│
   └──────────────┘
```

## Crates

| Crate | Purpose | Public surface |
|-------|---------|----------------|
| `stem-core` | AST, spans, diagnostics, theme, `Importer`/`Exporter` traits | `ast`, `span`, `diagnostic`, `theme`, `io::{Importer, Exporter}` |
| `stem-parser` | Stem source → AST + diagnostics. Canonical Stem-language reader (kept separate from `stem-imports`). | `parse(src) -> ParseResult`, `cook_document` |
| `stem-types` | Schema registry, validator, per-element vocabulary, formula parser/evaluator | `validate`, `default_registry`, `Registry`, `DocumentType`, `ElementDef`, `elements::*`, `formula` |
| `stem-imports` | External format → Stem AST. One feature-gated module per format. | `markdown::MarkdownImporter` (real), plus stub features for `docx`/`xlsx`/`pptx`/`hwpx` |
| `stem-exports` | Stem AST → external format. One feature-gated module per format. | `HtmlExporter`, `MarkdownExporter`, `PdfExporter`, `DocxExporter`, `XlsxExporter` (real); stubs for the rest |
| `stem-lsp` | LSP server binary | `stem-lsp` executable |
| `stem-cli` | User-facing CLI | `stem` executable |
| `stem-wasm` | WebAssembly bindings for the playground | `render(src) -> JsValue` |

## The Importer / Exporter pair

```rust
pub trait Importer {
    type Input;
    type Error: std::error::Error + Send + Sync + 'static;
    fn import(&self, input: Self::Input) -> Result<Document, Self::Error>;
}

pub trait Exporter {
    type Output;
    type Error: std::error::Error + Send + Sync + 'static;
    fn export(&self, doc: &Document, theme: &Theme) -> Result<Self::Output, Self::Error>;
}
```

Both live in `stem-core::io`. External formats implement one or both
direction in `stem-imports::<format>` / `stem-exports::<format>`. Each
module is gated by a Cargo feature with the same name as the module —
consumers compile only what they need.

Why the import/export split into one module per format, not separate
crates: until 2+ embedders complain about download size or independent
versioning, modules-with-features are cleaner than a workspace of
14 micro-crates. The boundary is easy to extract later if needed.

## Why this split

- **Parser is renderer-agnostic and validator-agnostic.** It produces
  a faithful tree of what the author wrote. Unknown elements and bad
  property values are not parse errors — they surface during validation.
  This is what lets the LSP keep syntax-highlighting and completing
  while the author is mid-type.

- **Validation is a separate pass over the AST**, parameterised by the
  document type. The same registry powers LSP diagnostics, LSP
  completion, and the CLI `check` command — there is no second source
  of truth.

- **Importers/exporters consume the same AST.** No transcoding layer
  in the middle. An importer's output is what a parser's output would
  look like; an exporter just walks the same tree shape.

- **The AST is the canonical IR — and the source language IS the IR.**
  A `.stem` file isn't a higher-level language that compiles down to
  some intermediate form; it's a textual serialization of the AST that
  humans and LLMs can read and write. Round-trip stays lossless across
  parse → emit → parse for any valid input.

## Theme system

A `Theme` (in `stem-core::theme`) is a small, stable struct: named
colors, named font roles, a baseline grid, default margins. Each
exporter maps those names to its native concept — hex codes in HTML
and PDF, named styles in docx, format objects in xlsx. Themes are
NOT serialised through the AST; they sit beside it during export.

## Diagnostics

Everything that produces diagnostics produces the same `Diagnostic`
type from `stem-core`. Parser, validator, and importers all emit into
the same channel, so the LSP merges them trivially and the CLI can
pretty-print them uniformly.

Stable code format: `<stage>.<short_name>`. Stages: `parse`, `type`,
`formula`, `cook`, `render`, `import.<format>`.

## Per-element vocabulary layout

Each element name has its own file in two places:

- `stem-types/src/elements/<name>.rs` — the `ElementDef` (schema +
  optional semantic validator).
- `stem-exports/src/html/elements/<name>.rs` — the HTML render fn.

Adding a new element name = creating these two files and registering
them in the corresponding `ALL` slice. No edits to `schema.rs` or
`html.rs`. See `docs/implementing-formats.md` for the analogous
pattern when adding format-side coverage to importers/exporters.
