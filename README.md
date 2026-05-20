# Stem

A small markup language designed to be **AI-friendly to generate**,
**human-friendly to read**, and **compilable** to docx/pptx/pdf/sheet
formats without manual fix-up.

```stem
[type:document, encoding:utf-8, locale:ko-KR, theme:corporate]

section(cover)(
  # 2026 Product Roadmap
  date(2026.05.20)
)

section(body)(
  ## Background

  Existing document ecosystems are text(falling behind)[color:primary]
  in the AI era. footnote(Gartner 2025 Report)

  layout(two-column)(
    col(
      ### Problems
      - Format fragmentation
      - Hard to generate with AI
    )
    col(
      ### Opportunities
      - Single source format
      - AI-native design
    )
  )

  table[border:outer](
    row(header)(
      cell(Phase)
      cell(Timeline)[span:2]
    )
    row(
      cell(Phase 1)
      cell(2026 Q2)
      cell(In Progress)[bg:yellow]
    )
  )
)
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
| [`stem-parser`](crates/stem-parser) | Source → AST + diagnostics, with markdown-flavored cooking |
| [`stem-types`](crates/stem-types) | Function & property registry; validator |
| [`stem-render`](crates/stem-render) | Renderer trait + HTML (full) + docx/pdf (stubbed contracts) |
| [`stem-lsp`](crates/stem-lsp) | `tower-lsp` server: diagnostics, completion, hover, symbols, semantic tokens |
| [`stem-cli`](crates/stem-cli) | `stem` binary — `parse` / `check` / `render` / `registry` |
| [`stem-wasm`](crates/stem-wasm) | `wasm-bindgen` bindings powering the live web playground in `web/` |

Detailed design lives in [`docs/grammar.md`](docs/grammar.md) (formal
EBNF + content disambiguation rules) and
[`docs/architecture.md`](docs/architecture.md).

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
Source is auto-persisted to `localStorage`.

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
- Completion (functions valid for the current document `type`, with
  snippet insertion matching the function's arity)
- Hover (function doc + property list)
- Document symbols (outline of sections/slides/tables)
- Semantic tokens (function names, property keys, string values)

## Building

```sh
cargo build --workspace
cargo test --workspace
```

31 tests across parser, validator, and renderer cover the grammar's
edge cases (chained args, escapes, balanced/nested parens, Unicode,
unclosed-paren recovery) and HTML output correctness.

## License

MIT OR Apache-2.0
