# Stem — Handoff

Resume-from-here doc. Read this, then `docs/architecture.md`, then
`docs/implementing-formats.md` if you're adding a new format module.

---

## What Stem is

A small markup language for documents, presentations, and
spreadsheets. Three load-bearing claims:

1. **AI-friendly to generate** — uniform grammar, no special cases.
2. **Human-friendly to read** — looks like the rendered output.
3. **Compilable to many formats** — one source, many targets.

The spec is settled (17 design decisions in `docs/grammar.md` §16).
The source language IS the IR — a `.stem` file IS the canonical
serialization of the AST.

## Current state

- **179 tests, all green.** `cargo test --workspace --all-features`.
- **Workspace builds clean.** `cargo build --workspace --all-features`.
- **Playground works.** `./scripts/serve-playground.sh` → http://localhost:8080
- **Format coverage** (see matrix below).

### Format coverage matrix

|         | Import        | Export        |
|---------|---------------|---------------|
| Stem    | ✅ canonical (`stem-parser`) | — |
| HTML    | —             | ✅ full        |
| Markdown| ✅ MVP        | ✅ MVP (round-trips with importer) |
| PDF     | —             | ✅ MVP (custom fonts incl. CJK family, real metrics, italic/bold runs) |
| docx    | stub          | ✅ MVP (headings, lists, inline styling, code) |
| xlsx    | stub          | ✅ MVP (cells, formulas, multi-sheet) |
| pptx    | stub          | stub          |
| hwpx    | stub          | stub          |
| image   | n/a           | stub          |

Importers and exporters live under `crates/stem-imports/` and
`crates/stem-exports/`. Each format is a feature-gated module inside
its crate. See `docs/implementing-formats.md` for the conventions and
per-format implementation notes.

### Other landmark features

- **Per-element layout.** Every Stem element lives in its own file
  under `stem-types/src/elements/<name>.rs` (vocabulary: schema +
  optional validate fn) and `stem-exports/src/html/elements/<name>.rs`
  (HTML render fn). Adding a new element is two files.
- **Custom doc types.** `DocumentType::Custom(&'static str)` lets
  embedders introduce new doc types (mindmap, whiteboard, …) at
  compile time. `Registry::resolve_doc_type` connects `type:foo`
  metadata to the registered Custom variant.
- **@math renders real MathML** via `pulldown-latex`. `notation:latex`
  is default; `notation:mathml` is pass-through. AsciiMath not yet
  implemented.
- **@formula validates at validate time.** Syntax errors surface as
  `formula.*` diagnostics before render.
- **Literal numeric cells respect `fmt`.** Same formatter path as
  formula cells.

## Repo layout

```
crates/
  stem-core/      AST, spans, diagnostics, theme, Importer/Exporter traits
  stem-parser/    Stem source → AST + cook pass
  stem-types/     schema, validator, formula parser/evaluator,
                  per-element vocabulary in elements/<name>.rs
  stem-imports/   external format → AST. one module per format,
                  feature-gated (markdown real; docx/xlsx/pptx/hwpx stubs)
  stem-exports/   AST → external format. one module per format,
                  feature-gated (html, markdown, pdf, docx, xlsx real;
                  pptx/hwpx/image stubs)
  stem-cli/       `stem` binary (stdin/stdout-only)
  stem-lsp/       `stem-lsp` binary (tower-lsp)
  stem-wasm/      wasm-bindgen wrapper for playground

docs/
  grammar.md              normative grammar reference (§16 = decisions log)
  schema.md               per-element vocabulary
  architecture.md         crate dependency diagram + design rationale
  implementing-formats.md guide for adding new importer/exporter modules

web/                      playground HTML/JS/CSS
scripts/
  serve-playground.sh     build wasm + serve
```

## Build / run / test

```sh
# Default features only
cargo build --workspace
cargo test --workspace

# Every format feature
cargo build --workspace --all-features
cargo test --workspace --all-features

# CLI smoke
echo 'h1(Hello)' | cargo run --bin stem -- parse
echo '# Hi' | cargo run --bin stem -- render --format html
echo '# Hi' | cargo run --bin stem -- render --format pdf > out.pdf
echo '# Hi' | cargo run --bin stem -- render --format docx > out.docx
echo '[type:sheet]
sheet{ cell[at:A1](42) }' | cargo run --bin stem -- render --format xlsx > out.xlsx

# Playground
./scripts/serve-playground.sh
```

## What is NOT done (deliberate v1.0 gaps)

In rough priority order. These are scope choices, not bugs.

1. **pptx exporter** — `presentation` doc type's natural target. ~1k LOC.
2. **docx tables, images, links, footnotes** — current docx covers
   headings/paragraphs/lists/inline styles/code. Tables next.
3. **xlsx cell format mapping** — `fmt:currency`/`percent`/etc. don't
   yet translate to Excel cell formats. Values land raw. ~100 LOC fix:
   register a `Format` per kind on the workbook.
4. **docx, xlsx, pptx importers** — readers are harder than writers
   (Office has compatibility quirks, theme overrides, named styles).
   Defer until the writers are stable in production.
5. **hwpx import + export** — Korean-market differentiator. Use
   `quick-xml` + the OWPML spec.
6. **image exporter** — Stem → SVG / PNG. SVG-via-`resvg` is portable;
   PDF-via-`pdfium` is heavier.
7. **AsciiMath notation** in `@math[notation:asciimath]` (currently
   returns an error span).
8. **Schema extractor** — `docs/schema.md` has machine-readable
   `stem-schema` blocks; the Rust mirror in `stem-types/src/elements/`
   is hand-kept.
9. **Cross-sheet refs in formulas** — `Sheet!Range` parses but isn't
   resolved.
10. **Doc-type extension at runtime** — `DocumentType::Custom` takes
    `&'static str`, so new doc types are compile-time only.

## The 17 settled design decisions

Listed in `docs/grammar.md` §16. The short version:

1. Non-Turing-complete forever
2. Block shapes: `name`, `name[props]`, `name[props](text)`,
   `name[props]{children}` — exactly one body
3. Properties: `[k:v, k:v]` post-name only
4. Chained args dropped — id is `[id:x]`
5. Range syntax: Excel `:` inside quoted strings (`at:"B2:B4"`)
6. Inline elements use `@`-prefix
7. Comments: `//` to EOL
8. Escapes: `\(`, `\)`, `\\`, `\@`, `\u{N}` in bare text
9. Top-level same as nested (uniform)
10. Sheet `fill`/`source` are sugar → per-cell blocks
11. Cell merge: properties merge, body replaces (with warning)
12. Cascade: column → row → cell, later overrides earlier
13. AST: generic `Block { name, props, body }`
14. Doc tables stay as `table{ row{ cell } }`
15. Text body: bare + quoted both legal
16. List numbering: `ol[start:N]` + `li[at:N]`
17. `@formula(...)` is the spreadsheet embed; no `=` prefix

## Things that will trip you up

- **`Registry::get` takes `(name, doc_type)`.** Element vocabulary is
  per-doc-type — `cell` is both a document-table cell and a spreadsheet
  cell, registered twice. Always pass the doc_type.
- **Always call `stem_parser::cook_document(doc)` first in exporters.**
  Sheet desugar + cascade rules apply there. Skipping it means
  cascade rules don't fire.
- **The schema lives in two places** — `docs/schema.md` (human source)
  and `crates/stem-types/src/elements/<name>.rs` (machine mirror).
  Keep in sync.
- **wasm-pack is needed for the playground.** The serve script auto-
  installs on first run.
- **`@formula("=...")` is rejected.** The `@formula` wrapper is the
  marker; the `=` is redundant. Typed error
  `formula.unexpected_equals_prefix`. Matches xlsx storage (no `=`).
- **`Block.inline_form: bool`** is part of the AST. Set to true for
  blocks placed in `TextPiece::Inline`, false otherwise.

## Conventions in this codebase

- **Tests live next to the code they test.** Per-crate `tests/{name}.rs`
  for integration; `#[cfg(test)] mod tests` for units.
- **Stable diagnostic codes.** Format: `<stage>.<short_name>`. Stages:
  `parse`, `type`, `formula`, `cook`, `render`. Format-side: scope as
  `import.<format>.*` or `export.<format>.*`.
- **Per-crate `lib.rs` re-exports.** Public surface is everything in
  the `pub use` list; internals are `mod`.
- **No silent normalization.** Use typed errors over fix-ups. Example:
  `parse_formula` rejects leading `=` instead of stripping.
- **Feature-gate per format.** Each external format is one feature in
  `stem-imports` or `stem-exports`. Consumers compile only what they
  need.

## Pointers for AI-assisted continuation

If resuming via Claude Code, the project memory directory
(`/Users/ghatdev/.claude/projects/-Users-ghatdev-Work-sinteric-stem/memory/`)
has structured context:

- `stem_project_state.md` — current state snapshot
- `spec_locked.md` — what's settled and not to relitigate
- `design_non_tc.md`, `design_inline_extensions.md` — load-bearing
  design principles
- `feedback_small_cps.md`, `feedback_no_hidden.md`,
  `feedback_playground_verify.md` — collaboration preferences
- `elements_layout.md` — per-element file convention
- `schema_source.md`, `ref_project_layout.md` — references

These load into context automatically when relevant.

## Recent commit story (selected)

```
17600d9 pdf: font-family API for CJK (regular + bold/italic variants)
c29b61d xlsx: native exporter via rust_xlsxwriter
f33cbe3 docx: native exporter via docx-rs
657ef08 pdf: custom fonts, real metrics, inline italic/bold
f9f5d8c exports: native PDF exporter via printpdf
2d71cc0 exports: add markdown exporter (round-trip with importer)
5eac051 imports/exports: rename stem-render, add stem-imports/markdown
d1a8eef cleanup: theme threading, literal cell fmt, HANDOFF refresh
73b37c3 elements: wire validate hook, real @math, custom doc types
7e590c9 elements: complete per-element migration
95cfb89 elements: introduce per-element layout (formula, link as trials)
```
