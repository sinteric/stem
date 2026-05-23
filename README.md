# Stem

A markup language for documents, presentations, and spreadsheets.
One source compiles to **HTML, Markdown, PDF, docx, and xlsx**.

Stem's source language IS the AST — there's no hidden intermediate
representation. A `.stem` file reads like the document it describes,
and round-trips losslessly through every supported exporter.

```stem
[type:document, title:"Hello"]

# Welcome to Stem
p(This is a paragraph with @text[weight:bold](bold), @text[style:italic](italic),
  and @link[to:"https://example.com"](a link).)

ul{
  li(One)
  li(Two)
  li(Three)
}
```

## What's it for

- **Authoring once, publishing to many formats** without writing the
  same content in five tools.
- **AI-assisted document generation** — Stem's uniform grammar (no
  special cases, no scripting layer, no macros) is designed to be
  reliably emittable by LLMs.
- **Spreadsheets in source control** — Stem sheets are plain text with
  `@formula(...)` cells, diff-able and reviewable.
- **Embedding in apps** — Stem is a Rust library; use it as the
  document layer of your editor, generator, or pipeline.

## Format coverage

|         | Import        | Export                                                          |
|---------|---------------|-----------------------------------------------------------------|
| Stem    | ✅ canonical  | —                                                               |
| HTML    | —             | ✅ full                                                          |
| Markdown| ✅ MVP        | ✅ MVP (round-trips with importer)                              |
| PDF     | —             | ✅ MVP (custom fonts incl. CJK, real metrics, italic/bold)      |
| docx    | planned       | ✅ direct OOXML emitter (`docx2` feature) — full paragraph/run/table/image/footnote/TOC support with source-level style overrides |
| xlsx    | planned       | ✅ MVP (cells, formulas, multi-sheet)                           |
| pptx    | planned       | planned                                                         |
| hwpx    | planned       | planned                                                         |
| image   | n/a           | planned (SVG / PNG)                                             |

All exporters are native Rust. No headless browsers, no shellouts.

## Status

Early. The spec is settled and tested (179 workspace tests). HTML and
Markdown round-trip cleanly; PDF/docx/xlsx exporters cover their 80%
use cases. Importers other than Markdown are not yet built. APIs are
pre-1.0 and may change.

If you ship something on top of Stem, please open an issue — early
users shape what stabilizes first.

## Try it

```sh
git clone https://github.com/sinteric/stem
cd stem

# Build and run the test suite
cargo test --workspace --all-features

# Render a Stem source to HTML
echo 'h1(Hello) p(world)' | cargo run --bin stem -- render --format html

# Or to PDF, docx, xlsx
echo 'h1(Hi)' | cargo run --bin stem -- render --format pdf > out.pdf
echo 'h1(Hi)' | cargo run --bin stem -- render --format docx > out.docx
echo '[type:sheet]
sheet{ cell[at:A1](42) cell[at:A2](@formula("A1*2")) }' \
  | cargo run --bin stem -- render --format xlsx > out.xlsx

# Live playground (auto-updates as you type)
./scripts/serve-playground.sh
# → http://localhost:8080
```

## The shape of Stem

Six block forms, uniform across all doc types:

```stem
name                          // bare
name[k:v]                     // with properties
name[k:v](text body)          // with text body
name[k:v]{ child child }      // with child blocks
@inline[k:v](inline body)     // inline element inside a text body
"quoted text"                 // string literal
```

Three built-in document types — `document`, `presentation`, `sheet` —
plus `DocumentType::Custom("…")` for embedders who want their own
(mindmap, whiteboard, diagram).

See [`docs/grammar.md`](docs/grammar.md) for the normative grammar and
[`docs/schema.md`](docs/schema.md) for the element vocabulary.

## Architecture

```
       source (.stem) ──▶ stem-parser ──▶ AST ◀── stem-imports/<format>
                                          │
                                          ▼
                                     stem-types
                                     (validate)
                                          │
                       ┌──────────────────┼──────────────────┐
                       ▼                  ▼                  ▼
              stem-exports/<format>   stem-lsp           stem-cli
```

`stem-imports/<format>` and `stem-exports/<format>` are feature-gated
modules. You compile only the formats you need:

```toml
[dependencies]
stem-exports = { version = "0.1", features = ["html", "pdf"] }
stem-imports = { version = "0.1", features = ["markdown"] }
```

See [`docs/architecture.md`](docs/architecture.md) for the diagram and
rationale, and [`docs/implementing-formats.md`](docs/implementing-formats.md)
for the format-module conventions.

## Why another markup language

Stem occupies a narrow gap:

- **Markdown** is great until you need real tables, sheets, slides,
  styling, or output to docx. Then it stops scaling.
- **LaTeX** is powerful but Turing-complete with macros — slow, opaque
  to static analysis, hard for LLMs to produce reliably.
- **AsciiDoc / reStructuredText** sit in between but accumulate special
  cases as they grow.
- **Typst** is excellent for typesetting but has a scripting layer
  (functions, control flow) that complicates the AI-generation case.

Stem's bet: a strictly declarative grammar with one uniform block
shape, no macros, no scripting, plus a single AST that's the source
language itself — readable like Markdown, expressive enough for
sheets and slides, structured enough for AI to produce.

## Contributing

The codebase is a Cargo workspace. Each format is one feature in
`stem-imports` or `stem-exports`. To add a new format, see
[`docs/implementing-formats.md`](docs/implementing-formats.md).

Most-needed contributions right now:

- pptx exporter (presentation doc type's natural target)
- docx tables, images, links, footnotes
- xlsx cell format mapping (`fmt:currency` → Excel format strings)
- docx / xlsx / pptx importers
- HWPX (Korean-market differentiator)

Run `cargo test --workspace --all-features` before sending a PR.

## License

Dual-licensed under your choice of [MIT](LICENSE-MIT) or
[Apache 2.0](LICENSE-APACHE).
