# Stem — Handoff

A concise resume-from-here doc. Read this, then `docs/grammar.md`
§16 (Decisions log), then `docs/schema.md`.

---

## What Stem is

A small markup language for documents, presentations, and
spreadsheets that aims to be:

1. **AI-friendly to generate** — uniform grammar, no special cases.
2. **Human-friendly to read** — looks like the rendered output.
3. **Compilable to many formats** — docx, pdf, pptx, sheet, html
   from one source.

The spec is settled. The HTML renderer is fully implemented. The
playground is live. docx/pdf renderers are stubbed.

## Current state

- **124 tests, all green.** `cargo test --workspace`.
- **Workspace builds clean.** `cargo build --workspace`.
- **Playground works.** `./scripts/serve-playground.sh` → http://localhost:8080
- **No v1.** A grammar v1 existed during the design pass and was
  deleted in commit `?` (most recent). The current grammar is the
  only grammar — there's nothing to migrate.

## Repo layout (quick)

```
crates/
  stem-core/     AST, spans, diagnostics, theme
  stem-parser/   text → AST, cook pass (sheet desugar/merge/cascade)
  stem-types/    schema + per-(name, doc-type) validator
  stem-render/   Renderer trait, HTML renderer, formula evaluator
  stem-cli/      `stem` binary (stdin/stdout-only)
  stem-lsp/      `stem-lsp` binary (tower-lsp)
  stem-wasm/     wasm-bindgen wrapper for playground
docs/
  grammar.md     normative grammar reference (§16 = decisions log)
  schema.md      per-element vocabulary
  architecture.md
web/             playground HTML/JS/CSS (loads web/pkg/stem_wasm.js)
scripts/
  serve-playground.sh   build wasm + serve
```

## Build / run / test

```sh
cargo build --workspace
cargo test --workspace
./scripts/serve-playground.sh
echo 'h1(Hello)' | cargo run --bin stem parse
```

## The 17 settled design decisions

Listed at `docs/grammar.md` §16. The short version:

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

## What is NOT done (deliberate v1.0 gaps)

These are not bugs — they're scope choices for a future version:

- **docx renderer** (`crates/stem-render/src/docx.rs`) — stub with
  implementation contract. `docx-rs` would be the obvious crate.
- **pdf renderer** (`crates/stem-render/src/pdf.rs`) — stub. Two
  paths: HTML→headless-Chromium, or native via Typst.
- **Formula validate-time checking** — `@formula(...)` errors today
  only surface at render time. Could be added by having
  `stem-types::validate` call into `stem_render::formula::parse_formula`
  to catch syntax errors before render.
- **Schema extractor** — `docs/schema.md` has machine-readable
  `stem-schema` fenced blocks intended for tooling extraction. The
  Rust mirror in `stem-types/src/schema.rs` is currently hand-kept.
- **Cross-sheet refs in formulas** — `Sheet!Range` syntax in formula
  bodies parses as an ident-without-cell-ref; not resolved.
- **Numeric formatting on literal cells** — `cell[at:B2,
  fmt:currency](42000)` renders raw "42000". Formula cells DO get
  formatted (`@formula("SUM(B1:B5)")` → "$X,XXX.XX"). Literal cells
  would need the same formatter path.
- **Korean/CJK list markers in HTML output** — schema declares
  `ol[style:가.]` as valid, but HTML's `<ol>` doesn't render Korean
  ordinals natively. Would need CSS counter-style rules in the
  rendered output.

## Suggested next moves

In rough priority order. Each is independent.

1. **Wire formula errors into the validate pass.** Smallest concrete
   win: catch `@formula("BOGUS(1)")` typos before the renderer runs.
   Two files: `stem-types/src/validator.rs` walks `@formula` inline
   elements; calls `stem_render::formula::parse_formula`; emits
   `formula.*` codes. (Requires `stem-types` to depend on
   `stem-render` OR extracting `formula` to its own crate first.)

2. **Numeric formatting on literal cells.** Symmetry with formula
   cells. `stem-render/src/html.rs::render_sheet_cell` already has
   the formatter (`crate::formula::format_value`); just call it for
   `CellSource::Literal(n)` where `n` parses as a number.

3. **`stem-math` embed.** Recognise `@math("\\frac{a}{b}")` in
   `stem-render/src/html.rs`, emit MathML. KaTeX-server-side would
   work but adds a JS dependency; native MathML is sufficient for
   modern browsers (Firefox 140+, Chrome 109+, Safari).

4. **docx renderer.** `docx-rs` crate has a clean API. Section §11
   of `crates/stem-render/src/docx.rs` has the implementation
   contract.

5. **Schema extractor script.** Read `stem-schema` fenced blocks
   from `docs/schema.md`; emit `crates/stem-types/src/schema.rs`
   contents. Run in `build.rs` or as `scripts/gen-schema.sh`.

6. **AI generation benchmark.** Pin a prompt template + 20 doc
   tasks; run against 3-5 LLMs; measure parse-rate / validate-rate /
   render-clean-rate. This is the metric for the "AI-friendly"
   claim — without it, the claim isn't real.

## Things that will trip you up

- **`Registry::get` takes `(name, doc_type)`.** Element vocabulary
  is per-doc-type (e.g., `cell` is BOTH a document-table cell AND a
  spreadsheet cell, registered twice). Always pass the doc_type.
- **The schema lives in two places** (`docs/schema.md` is the human
  source; `crates/stem-types/src/schema.rs` is the machine mirror).
  Keep them in sync.
- **wasm-pack is needed for playground.** The serve script
  auto-installs it on first run. Subsequent builds are fast.
- **The cooked AST drops cascade rule blocks.** After `cook_document`,
  `col[at:...]`/`row[at:...]`/`format[at:...]` are CONSUMED — they
  applied their properties to cells and are no longer in the output
  tree. Renderers walk cells, not rule blocks.
- **`@formula("=...")` is rejected.** The `@formula` wrapper is the
  marker; the `=` is redundant. Typed error
  `formula.unexpected_equals_prefix`. Matches xlsx storage (which
  also has no `=`).

## Conventions in this codebase

- **Tests live next to the code they test.** Per-crate
  `tests/{name}.rs` for integration; `#[cfg(test)] mod tests` for
  units.
- **Stable diagnostic codes.** Format: `<stage>.<short_name>`.
  Stages: `parse`, `type`, `formula`, `cook`, `render`.
- **Per-crate `lib.rs` re-exports.** Public surface is everything in
  the `pub use` list; internals are `mod`.
- **No silent normalization.** See `feedback-no-hidden` in the Claude
  project memory or by example: `parse_formula` rejects leading `=`
  with a typed error rather than stripping it.

## Pointers for AI-assisted continuation

If resuming via Claude Code, the project memory directory
(`/Users/ghatdev/.claude/projects/-Users-ghatdev-Work-sinteric-stem/memory/`)
has structured context:

- `stem_project_state.md` — current state snapshot
- `spec_locked.md` — what's settled and not to relitigate
- `design_non_tc.md`, `design_inline_extensions.md` — load-bearing
  design principles
- `feedback_small_cps.md`, `feedback_no_hidden.md`,
  `feedback_playground_verify.md` — user's collaboration preferences
- `schema_source.md`, `ref_project_layout.md` — references

These are loaded into context automatically when relevant.
