# Stem — Architecture

```
┌─────────────────┐    ┌──────────────┐    ┌──────────────┐
│ source (*.stem) │───▶│  stem-parser │───▶│   AST        │
└─────────────────┘    └──────────────┘    └──────┬───────┘
                                                   │
                                                   ▼
                                          ┌──────────────────┐
                                          │  stem-types      │
                                          │  (validate)      │
                                          └──────┬───────────┘
                                                 │ diagnostics
                       ┌─────────────────────────┼─────────────┐
                       ▼                         ▼             ▼
              ┌─────────────────┐       ┌─────────────┐  ┌────────────┐
              │  stem-render    │       │  stem-lsp   │  │ stem-cli   │
              │  HTML / docx /  │       │  diagnostics│  │ parse/check│
              │  pdf (stubbed)  │       │  completion │  │ render     │
              └─────────────────┘       └─────────────┘  └────────────┘
```

## Crates

| Crate | Purpose | Public surface |
|-------|---------|----------------|
| `stem-core` | AST, spans, diagnostics, theme types | `ast`, `span`, `diagnostic`, `theme` |
| `stem-parser` | Source → AST + diagnostics | `parse(src) -> ParseResult` |
| `stem-types` | Function/property registry, validation | `Registry::default()`, `validate(doc)` |
| `stem-render` | Renderer trait + impls | `Renderer`, `HtmlRenderer`, `DocxRenderer`, `PdfRenderer` |
| `stem-lsp` | LSP server binary | `stem-lsp` executable |
| `stem-cli` | User-facing CLI | `stem` executable |

## Why this split

- Parser is renderer-agnostic and validator-agnostic — it only produces a
  faithful tree of what the author wrote. Unknown functions and bad
  property values are *not* parse errors. This is what lets the LSP keep
  syntax-highlighting and completing while the author is mid-type.
- Validation is a separate pass over the AST, parameterised by the
  document type from the metadata header. The same registry powers
  LSP diagnostics, LSP completion, and the CLI `check` command — there
  is no second source of truth.
- Renderers consume the AST + theme. They do **not** see the source text
  or the registry. This forces renderers to think in terms of *layout
  intent*, which is the whole point of Stem.

## Theme system

A `Theme` (in `stem-core::theme`) is a small, stable struct: named colors,
named font roles, a baseline grid, default margins. Each renderer maps
those names to its native concept — hex codes in HTML/PDF, named styles
in docx. Themes are NOT serialised through the AST; they sit beside it
during render.

The renderer trait:

```rust
pub trait Renderer {
    type Output;
    type Error;
    fn render(&self, doc: &Document, theme: &Theme) -> Result<Self::Output, Self::Error>;
}
```

`Document` is the validated AST. `Output` is whatever the renderer
produces — `String` for HTML, `Vec<u8>` for docx/pdf.

## Diagnostics

Everything that produces diagnostics produces the same `Diagnostic` type
from `stem-core`. Parser and validator both emit into the same channel,
so the LSP merges them trivially and the CLI can pretty-print them
uniformly.
