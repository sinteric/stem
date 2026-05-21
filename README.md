# Stem

A small markup language designed to be **AI-friendly to generate**,
**human-friendly to read**, and **compilable** to docx/pptx/pdf/sheet
formats without manual fix-up.

```stem
[type:document, locale:ko-KR, title:"2026 Roadmap"]

section{
  h1(2026 Product Roadmap)
  h2(Strategy Team)
  date(2026.05.20)
}

section{
  h2(Background)

  p(Existing document ecosystems are @text[color:primary](falling behind)
  in the AI era. @footnote(Gartner 2025 Report))

  layout[kind:two-column]{
    col{
      h3(Problems)
      ol[style:1.]{
        li(Format fragmentation)
        li(Hard to generate with AI)
      }
    }
    col{
      h3(Opportunities)
      ol[style:가.]{
        li(Single source format)
        li(AI-native design)
      }
    }
  }

  table[border:outer]{
    row[kind:header]{
      cell(Phase)
      cell(Content)
      cell[colspan:2](Timeline)
    }
    row{
      cell(Phase 1)
      cell(Spec finalization)
      cell(2026 Q2)
      cell[bg:yellow](In Progress)
    }
  }
}
```

## Why

Existing formats like `.docx` and `.pptx` are **editor-state** formats,
not **content + layout-intent** formats. Stem separates the two: the AST
carries semantic structure (sections, tables, layouts) and the renderer
decides how that structure manifests in each output format.

## Crates

| Crate | Purpose |
|-------|---------|
| [`stem-core`](crates/stem-core) | AST, spans, diagnostics, theme types — no dependencies |
| [`stem-parser`](crates/stem-parser) | Source → AST + diagnostics, cook pass for sheet desugaring/cascade |
| [`stem-types`](crates/stem-types) | Element registry + validator (per-doc-type) |
| [`stem-render`](crates/stem-render) | Renderer trait + HTML (full) + docx/pdf (stubs); spreadsheet formula engine |
| [`stem-lsp`](crates/stem-lsp) | `tower-lsp` server: diagnostics, completion, hover, symbols, semantic tokens |
| [`stem-cli`](crates/stem-cli) | `stem` binary — `parse` / `check` / `render` / `registry` |
| [`stem-wasm`](crates/stem-wasm) | `wasm-bindgen` bindings powering the live web playground in `web/` |

Detailed design lives in [`docs/grammar.md`](docs/grammar.md) (formal
EBNF + content disambiguation rules), [`docs/schema.md`](docs/schema.md)
(element vocabulary), and [`docs/architecture.md`](docs/architecture.md).

## Playground (live web preview)

A WASM build of the parser + validator + HTML renderer drives a
live-preview playground:

```sh
./scripts/serve-playground.sh
# → http://localhost:8080
```

First run auto-installs the wasm32 target and `wasm-pack`. After that
it's fast. Source on the left, sandboxed iframe preview on the right,
diagnostics underneath — all updates on keystroke (50ms debounce).
Source is auto-persisted to `localStorage`. Double-click reset to load
the sheet demo (formulas + cascade).

## CLI usage

`stem` is a Unix-pipeline tool — it reads from stdin and writes to
stdout, so it composes with `<`, `>`, `|`, `tee`, etc.:

```sh
stem render --format html < examples/roadmap.stem > roadmap.html
stem check                 < examples/roadmap.stem
stem parse                 < examples/roadmap.stem
stem registry              # dump function registry
```

`stem render --format docx|pdf` is wired through the same renderer
trait; the docx and pdf renderers are currently stubs (return
`NotImplemented`) with full interface contracts in
`crates/stem-render/src/{docx,pdf}.rs` for a future implementation.

## LSP

`stem-lsp` is a binary; point your editor's generic LSP client at it
for `.stem` files. Capabilities:

- Diagnostics (parser + validator, merged)
- Completion (elements valid for the current document `type`)
- Hover (element doc + property list)
- Document symbols (outline of sections, slides, sheets, tables)
- Semantic tokens (block names, property keys, string values)

## Building

```sh
cargo build --workspace
cargo test --workspace
```

124 tests cover the grammar's edge cases, schema validation, sheet
desugaring/merging/cascading, formula evaluation, and HTML output.

## License

MIT OR Apache-2.0
