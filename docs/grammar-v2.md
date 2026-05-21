# Stem — Grammar (v2)

**Status:** draft. Supersedes `grammar.md` (v1) once implemented.

This document is the normative reference for the v2 surface syntax of Stem.
Every implementation (parser, formatter, LSP, AI prompts) must match it.

The design is the outcome of an iterative pass against six requirements:
**fluent, descriptive, declarative, looks-alike with result, extendable, not
verbose.** When the requirements conflict, "looks-alike" and "extendable"
win, because the language has to survive thirty years of document evolution.

---

## 1. Big picture

A Stem source file is:

1. An optional **metadata header** — `[k:v, k:v]` on one line.
2. A sequence of **blocks**.

Everything in the language is a block. A block has a name, optional
properties, and optionally a body of one of two shapes. There are no
other syntactic shapes — no chained args, no significant indent, no
markdown, no close-tag mirrors.

The grammar is context-free. The parser does not consult the schema to
decide where a block starts or ends. AST shape is fully determined by
brackets in the source.

---

## 2. Block shapes

Every block fits one of these six patterns:

```
name                            # no body, no properties
name [props]                    # no body, properties
name (text body)                # text body, no properties
name [props] (text body)        # text body, properties
name { child blocks }           # block body, no properties
name [props] { child blocks }   # block body, properties
```

- **Text body** — `( … )` — contains literal characters, escape
  sequences, and inline elements (which are themselves blocks with text
  bodies).
- **Block body** — `{ … }` — contains zero or more child blocks.
- **No body** — the block stands alone; its meaning comes from its name
  and properties only.

Body kind is **a property of the source**, not the schema. The schema
may *prefer* a particular kind (e.g. `cell` prefers text) and the
validator emits a diagnostic when the source picks the wrong one, but
the parser never guesses.

### Structural rules (enforced by the parser)

Three constraints that make the pattern list above exhaustive:

1. **Exactly one body per block.** A second `(…)` or `{…}` immediately
   after the first is a parse error: `parse.multiple_bodies`.
2. **At most one `[props]` block.** A second `[…]` is a parse error:
   `parse.multiple_property_blocks`.
3. **Properties always precede the body.** `[…]` after a body is a
   parse error: `parse.misplaced_properties`. This is the single
   canonical position; the formatter never has to choose.

These rules apply uniformly to every block — text-body, block-body,
inline, top-level. No exceptions, no "either" form. Consistency over
local ergonomics: a doc language that mostly looks like one form but
sometimes flips makes tooling brittle and AI generation noisy.

### Examples

```stem
pagebreak
section[id:toc]

h1(2026 Product Roadmap)
date(2026.05.20)

section{
  h1(Background)
  p(Some text.)
}
```

---

## 3. Properties

```
properties = '[' property ( ',' property )* ']'
property   = ident ':' value
```

- Always between the block name and the body.
- No properties before the name. No properties after the body. (One
  canonical position.)
- Keys are `[a-zA-Z][a-zA-Z0-9_-]*`. Same identifier rules as block names.
- Values are **bare** (an unquoted run of characters) or **quoted**
  (a `"…"` string).
- Whitespace around the `:` is allowed and ignored.

### Bare vs quoted values

A bare value may not contain `:`, `,`, `]`, or a newline. If the value
needs any of those characters, quote it. The same rule TOML, YAML, and
JSON use.

```stem
[color:red]                 # bare
[span:2]                    # bare
[id:cover]                  # bare
[title:"Hello, World"]      # quoted — contains a comma
[name:"4분기 매출"]         # quoted — contains a space (good practice)
[at:"B2:B4"]                # quoted — contains a colon (range syntax)
```

A bare value containing `:`, `,`, `]`, or a newline is a parse error
(`parse.bad_property_value`). The parser does not split on the first
`:` to "rescue" the value — quote it instead. This makes ambiguous
source detectable, not silently accepted.

### Property value escapes (inside quoted strings)

`"\""`, `"\\"`, `"\n"`, `"\t"`, `"\r"`, `"\u{XXXX}"` (any Unicode
codepoint, 1–6 hex digits in braces). No other escapes.

Use `\u{N}` for invisible characters (zero-width joiner `\u{200D}`,
object replacement `\u{FFFC}`), PUA characters for legacy CJK
compatibility, or any codepoint that doesn't survive editor or
source-control round-tripping. For visible characters (한글, 漢字,
emoji), paste literal UTF-8 — the escape is for the cases UTF-8 can't
handle reliably.

---

## 4. Text body

A text body is delimited by `( … )` and comes in one of two forms.
Authors pick whichever reads better for the situation; the formatter
preserves the choice.

### (a) Bare form

Literal text plus inline elements:

```stem
p(some prose with @text[color:red](styling) inside)
```

- Most characters are literal, including bare identifiers.
- To write a literal `(`, `)`, `\`, or `@`, use an escape sequence:
  `\(`, `\)`, `\\`, `\@`. `\u{XXXX}` inserts any Unicode codepoint
  (1–6 hex digits in braces) — for invisible characters or codepoints
  that don't survive UTF-8 round-tripping. No other escapes; a stray
  `\x` for any other `x` is an error.
- An **inline element** starts with `@` directly followed by an
  identifier, then `(`, `[`, or both. An inline element must have at
  least one of a properties block `[…]` or a body `(…)` — bare
  `@ident` alone is a parse error (`parse.bodyless_inline_required`).
  For markers that need to appear mid-document but have no body
  (e.g. `pagebreak`), close the surrounding text body and use the
  block form on its own line.
- Newlines are preserved as content. The renderer decides whether to
  treat them as soft-wrap, hard breaks, or spaces.

### (b) Quoted form

Verbatim text, opaque to the Stem parser:

```stem
cell[at:B5]("=SUM(B2:B4)")
p("He said (suddenly) ""no"".")
```

- `"…"` encloses the text. Any character is literal inside, including
  `(`, `)`, `\`, and `@`.
- A literal `"` is written as `""` (doubled). Same RFC 4180 rule as
  the `fill` CSV body.
- `\u{XXXX}` Unicode escape works inside quoted form as well, for
  consistency with bare-form and property-value escapes.
- **No inline elements.** The quoted form disables inline parsing —
  `"@text(red)"` is the literal text `@text(red)`, not a styled span.
- Newlines are preserved.

### When to pick which

Roughly:

- Short text with one or two literal specials → bare with escapes
  (`p(He paid \(in cash\) at noon)`)
- Longer text, formulas, or anything with many parens → quoted
  (`cell[at:B5]("=SUM(B2:B4)")`)
- You want inline styling → must use bare
- You want pure verbatim (disable inline parsing) → must use quoted

### Why `@` for inline?

Two reasons:

1. **Visual distinction in prose.** `@text(...)` is unmissable; the
   styled content doesn't blend into surrounding words.
2. **No accidental parses.** Real sentences like "the alert(1)
   function" parse as literal text, not as an inline call to a
   function named `alert` (the `(` is escaped because bare bodies
   require it, and you wouldn't write `\@` accidentally either).

The `@`-prefix applies *only* inside a bare text body. At block
position the same elements are bare:

```stem
section{
  text[color:red](some standalone text)            // block use, bare
  p(some prose with @text[color:red](red) inside)  // inline use, @-prefix
}
```

To write a literal `@` in a bare body, use `\@`. In a quoted body, `@`
is just text.

---

## 5. Block body

A block body is delimited by `{ … }`. Inside a block body:

- Only child blocks, separated by whitespace and newlines.
- No raw text directly inside `{ … }`. If you need a paragraph, use a
  leaf block: `p(text)`. A bare `(text)` at block position is a parse
  error.
- Empty block bodies are legal (`section{}`) and emit no diagnostic at
  the grammar level — they're useful for stubbing during authoring.
  Per-element schemas may warn when an element that normally requires
  children has none (see §12).
- Empty text bodies `name()` are legal but emit a hint diagnostic:
  `parse.empty_text_body` — "did you mean `name` (no body) or `name{}`
  (block body)?"

### Example

```stem
layout[kind:two-column]{
  col{
    h3(Problems)
    ol[style:1.]{
      li(Format fragmentation)
      li(Hard to generate)
    }
  }
  col{
    h3(Opportunities)
    ol[style:가.]{
      li(Single source format)
    }
  }
}
```

---

## 6. Metadata header

```
[k:v, k:v, k:v]
```

- If present, must be the first non-comment, non-blank thing in the file.
- One line, no trailing content. Newlines allowed inside the `[ ]` for
  readability.
- Reserved keys: `type` (`document` | `presentation` | `sheet`),
  `encoding`, `locale`, `theme`, `title`. Other keys are allowed and
  passed to the renderer as document-level properties.

```stem
[type:document, locale:ko-KR, title:"2026 Roadmap"]
```

---

## 7. Comments

```
// to end of line
```

- Allowed anywhere whitespace is allowed: between blocks, inside a
  block body, on a line by themselves.
- Not allowed inside a text body `( … )` or inside a quoted string.
- No block comments.

```stem
// Quarterly metrics doc
section{
  h1(Q3 Results)
  // skip the executive summary for now
  table[border:outer]{
    // rows to be filled in
  }
}
```

---

## 8. Identifiers

```
ident = /[a-zA-Z][a-zA-Z0-9_-]*/
```

- ASCII only.
- Case-sensitive (`Cell` ≠ `cell`).
- Convention: kebab-case for multi-word names (`speaker-note`,
  `two-column`).

---

## 9. Document type conventions

The grammar is uniform across doc types. Each type ships with its own
element schema; elements that don't exist in the schema are unknown to
the validator (warning) but still parse cleanly.

### `document`

`section`, `h1`–`h6`, `p`, `ol`, `ul`, `li`, `table`, `row`, `cell`,
`layout`, `col`, `image`, `code`, `note`, `pagebreak`, `hr`. See
`docs/schema.md` for each element's properties and body shape.

### `presentation`

`slide`, `title`, `bullets`, `item`, `image`, `speaker-note`,
`transition`. `slide` is the top-level container; everything else is
per-slide.

### `sheet`

`sheet`, `col`, `row`, `cell`, `fill`, `source`, `named`, `format`,
`chart`. Sheet semantics are address-based (see §10).

### Universal inline elements

Available inside any text body, regardless of doc type:

`text` (styled span), `footnote`, `date`, `code`, `link`, `mention`,
`math`. These are doc-type-agnostic because prose is the same shape
everywhere.

### Extensibility

The element vocabulary is **open**. The parser does not consult a
schema — it parses `foo[bar:baz](qux)` exactly like `section{...}`.
Unknown elements still produce a valid AST.

What changes between known and unknown:

- **Validator** emits `type.unknown_function` (warning, not error)
  when an element isn't in the registry for the current doc type.
- **Renderer** falls back to a generic representation for unknown
  elements (e.g., HTML wraps them in `<div data-stem="name">`).
- **LSP** completion lists only registered names; unknown names
  hover with "no documentation."

v2.0 ships a built-in registry covering the elements listed above.
Future versions add elements without breaking existing docs (additive
only). User-defined elements are valid Stem today via the warning
path; **first-class custom registries** (e.g.
`[type:document, registry:"./my-schema.toml"]`) are reserved for
v2.1 — see §14.

### List numbering (document, presentation)

Both `ol` and `ul` accept a `start` property; their `li` children
accept an `at` override:

```stem
ol[style:1., start:5]{
  li(Fifth item)            // "5."
  li(Sixth item)            // "6."
  li[at:10](Tenth item)     // "10." (override)
  li(Eleventh item)         // "11." (continues from 10)
}
```

`start` and `at` are **positional indices** into the list's style
sequence — `start:5` with `style:가.` renders as `마.`, with `style:I.`
as `V.`, with `style:①` as `⑤`. After an `[at:N]` override, the
counter continues from N+1.

---

## 10. Sheet-specific patterns

### Mental model

A sheet is **address-based**: every value lives at a cell address
(`A1`, `B5`). Properties cascade column → row → cell, later overriding
earlier. The primary form is one block per cell — `fill` and `source`
are sugar that desugars into per-cell blocks.

### Addresses

Bare for simple positions, quoted for ranges.

```stem
cell[at:A1]            // single cell
col[at:B]              // whole column
row[at:5]              // whole row
cell[at:"B2:B4"]       // range (vertical)
format[at:"A1:C5"]     // range (rectangle)
```

### Primary form — per-cell blocks

```stem
sheet[id:Q4-2026]{
  col[at:B, fmt:currency]
  col[at:C, fmt:percent]
  row[at:1, weight:bold, bg:gray]

  cell[at:A1](Item)        cell[at:B1](Revenue)         cell[at:C1](Margin)
  cell[at:A2](Widget)      cell[at:B2](42000)           cell[at:C2](0.35)
  cell[at:A3](Gadget)      cell[at:B3](38500)           cell[at:C3](0.42)
  cell[at:A5](Total)       cell[at:B5]("=SUM(B2:B4)")   cell[at:C5]("=AVERAGE(C2:C4)")
  cell[at:C5, bg:yellow]   // override layered on top
}
```

Every cell is independently addressable, independently styleable. The
LSP can "go to cell B5"; source-control diffs are per-cell.

### Cascade

Properties cascade column → row → cell, with later overriding earlier.

```stem
col[at:B, fmt:currency]   // all of column B is currency
row[at:1, weight:bold]    // row 1 is bold
cell[at:B1]               // this cell is bold AND currency
cell[at:B5, bg:yellow]    // plus yellow background
```

### Cell merge semantics

Multiple `cell` blocks may target the same address — typically a
`fill`-supplied value plus a later override. The typed-tree pass
merges them in source order:

1. Resolve all `fill` and `source` blocks into per-cell blocks first.
2. Walk subsequent `cell[at:X]` blocks in source order:
   - **If no cell exists at X:** insert the new cell.
   - **If a cell already exists at X:**
     - Properties merge per key, later wins on conflicts.
     - If the new cell has a body, it replaces the existing body.
     - If the new cell has no body, the existing body is preserved.
3. Apply column- and row-level cascades to the merged cell set.

Practically: `cell[at:C5, bg:yellow]` (no body) added after a `fill`
keeps the formula but adds a yellow background — matches how
formatting an existing Excel cell works. Diagnostic
`type.duplicate_cell_body` warns when two consecutive `cell[at:X]`
blocks both supply a body (probably author error).

### Sugar — `fill` for bulk inline data

`fill` bodies use the **quoted** text-body form (§4b) because CSV
content almost always contains literal parens (in formulas) and
sometimes literal commas (in quoted CSV cells):

```stem
fill[at:A1]("
  Item,     Revenue,        Margin
  Widget,   42000,          0.35
  Total,    =SUM(B2:B4),    =AVERAGE(C2:C4)
")
```

The body's content is then parsed at **validate time** as CSV. Each
line is a row; the top-left cell is `[at:_]`. **Default separator is
`,`**, follows RFC 4180 quoting rules (cells with `,` or `"` use
`"…"` and `""` for embedded quotes). Override with `[sep:"\t"]` for
TSV or `[sep:";"]` for semicolon-style.

`fill` is sugar: at typed-tree time it **desugars into per-cell blocks**.
Renderers and validators never see `fill` — they see cells.

### Sugar — `source` for external data

```stem
source[file:"data/q4.csv", at:A1, has-header:true]
```

For real-world large sheets, reference an external CSV. Same address
conventions. Like `fill`, `source` is sugar — desugars to per-cell
blocks at typed-tree time after the file is loaded.

### Formulas inside cell values

Formulas live inside cell text bodies. The formula DSL is
Excel-flavored — `=` prefix, parens for grouping, `:` for ranges:
`=SUM(B2:B4)`. Two ways to write them:

- **Quoted** (recommended): `cell[at:B5]("=SUM(B2:B4)")` — the
  formula's parens and `:` are verbatim, no escaping needed.
- **Bare with escapes**: `cell[at:B5](=SUM\(B2:B4\))` — works but
  noisy for formulas with multiple parens.

Both produce the same AST. Either form is fine — quoted is what
the `stem fmt` formatter emits by default for formula cells.

---

## 11. AST mapping

The parser emits a generic AST:

```rust
struct Block {
  name: String,
  name_span: Span,
  properties: Vec<Property>,
  body: Body,
  span: Span,
}

enum Body {
  None,
  Text(Vec<TextPiece>),         // (…)
  Children(Vec<Block>),         // {…}
}

enum TextPiece {
  Literal(String, Span),
  Inline(Block),                // a Block in inline position
}
```

A second **typed-tree** pass produces strongly-typed nodes per element
kind (`Section`, `Heading`, `Table`, `SheetCell`, …) that renderers
consume. The typed tree is what makes renderers ergonomic; the generic
tree is what makes the parser robust and the validator/LSP simple.

---

## 12. Diagnostics

The parser is error-recovering. On a syntax error:

1. Emit a `Diagnostic { severity, code, message, span }`.
2. Recover at the nearest of: matching close character, newline at
   depth 0, end of file.
3. Continue parsing.

Diagnostic codes:

- `parse.*` — syntax-level (`parse.unclosed_paren`, `parse.bad_escape`,
  `parse.unterminated_string`, `parse.multiple_bodies`,
  `parse.multiple_property_blocks`, `parse.misplaced_properties`,
  `parse.empty_text_body`, `parse.top_level_text`,
  `parse.bodyless_inline_required`, `parse.bad_property_value`,
  `parse.invalid_codepoint`, ...).
- `type.*` — schema-level (`type.unknown_function`,
  `type.wrong_body_kind`, `type.bad_property_value`, ...).
- `render.*` — render-time issues (`render.missing_image`,
  `render.csv_parse_failed`, ...).

Codes are stable — third-party tools depend on them.

---

## 13. Migration from v1

| v1 | v2 |
|---|---|
| `name(content)[props]` (block call) | `name[props]{ children }` (block body) or `name[props](text)` (text body) — props always pre-body |
| `name(content)[props]` (inline use) | `@name[props](text)` — `@`-prefix marks inline use, props still pre-body |
| `section(cover)(body)` (chained args) | `section[id:cover]{ body }` |
| `# Heading` (markdown) | `h1(Heading)` (explicit) |
| `- item` (markdown list) | `ol{ li(item) }` or `ul{ li(item) }` |
| `**bold**` (markdown) | `@text[weight:bold](bold)` (inline) |
| `cell(value)[span:3]` | `cell[colspan:3](value)` |
| `cell(value)[bg:yellow]` (post-body) | `cell[bg:yellow](value)` (pre-body, only canonical position) |
| `(some text)` at top level | parse error — wrap in `p(some text)` |

The v2 parser does **not** accept v1 source. A `stem migrate` CLI
command is planned for transcoding v1 → v2; until it ships, v1 docs
must be hand-converted.

---

## 14. Reserved-for-future appendix

Things deliberately out of scope for v2.0, listed here so future
versions don't accidentally paint into a corner:

- **Computed features:** cross-references, auto-numbering, theme
  variable references in property values. All decidable, all
  declarative when added — never a scripting layer.
- **Custom document types.** Currently the registry is built-in;
  later, a `[type:custom-id]` with a separately-registered schema.
- **Floats and lists as property value types.** Today only string,
  int, bool, enum, color. Adding more is additive.
- **`stem fmt` formatter.** Mechanical: walks the AST, emits canonical
  Stem with conventional 2-space indent. Will be the first thing built
  after v2 ships.

---

## 15. Worked examples

### A. Document — the 2026 Roadmap

```stem
[type:document, locale:ko-KR, title:"2026 Roadmap"]

section{
  h1(2026 Product Roadmap)
  h2(Strategy Team)
  date(2026.05.20)
}

section[id:toc]

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
        li(Manual conversion work)
      }
    }
    col{
      h3(Opportunities)
      ol[style:가.]{
        li(Single source format)
        li(AI-native design)
        li(Auto conversion)
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

section{
  h1(Thank you)
  note(Contact: contact@example.com)
}
```

### B. Presentation — three slides

```stem
[type:presentation, theme:corporate]

slide[id:intro, layout:title-bullets]{
  title(Welcome to Stem)
  bullets{
    item(Markup that AI can generate cleanly)
    item(Compiles to pptx, docx, pdf, sheet)
    item(Single source, many outputs)
  }
  speaker-note(remember the AI-generation benchmark numbers)
}

slide[id:metrics, layout:two-column]{
  title(Q3 Metrics)
  col{
    h3(KPIs)
    bullets{
      item(MRR @text[color:green](up 12%))
      item(NPS @text[weight:bold](72))
    }
  }
  col{
    image[src:"charts/mrr.png", w:100%, alt:"MRR trend chart"]
  }
}

slide[id:close, layout:title-only]{
  title(Thank you)
  speaker-note(Q&A starts here)
}
```

### C. Sheet — small structured

```stem
[type:sheet, locale:ko-KR]

sheet[id:Q4-2026, name:"4분기 매출"]{
  col[at:A, width:120]
  col[at:B, width:100, fmt:currency]
  col[at:C, width:80,  fmt:percent]

  row[at:1, weight:bold, bg:gray]
  row[at:5, weight:bold]

  fill[at:A1]("
    Item,     Revenue,        Margin
    Widget,   42000,          0.35
    Gadget,   38500,          0.42
    Sprocket, 19200,          0.28
    Total,    =SUM(B2:B4),    =AVERAGE(C2:C4)
  ")

  cell[at:C5, bg:yellow]
  named[name:Revenue, at:"B2:B4"]
  format[at:"A1:C1", align:center]
}

sheet[id:Charts]{
  chart[type:bar, data:"Q4-2026!B2:B4", title:"분기별 매출"]
}
```

---

## 16. Consolidated EBNF

The grammar in one block. Fragments throughout earlier sections
are normative; this appendix is convenience for implementers.

```ebnf
(* Top level *)
document        = metadata? block_list
metadata        = '[' property (',' property)* ']' nl

block_list      = (block | comment | ws_or_nl)*

(* Block *)
block           = ident properties? body?
body            = text_body | block_body
text_body       = '(' (bare_text | quoted_text) ')'
block_body      = '{' block_list '}'

(* Properties *)
properties      = '[' property (',' property)* ']'
property        = ident ws* ':' ws* value
value           = quoted_string | bare_value
bare_value      = /[^,\]:\n]+/   (* no ':' ',' ']' newline; trim trailing ws *)
quoted_string   = '"' (string_char | string_escape | '""')* '"'
string_char     = /[^"\\]/
string_escape   = '\\' ('"' | '\\' | 'n' | 't' | 'r')
                | '\\u{' /[0-9A-Fa-f]+/ '}'

(* Text body — bare form *)
bare_text       = (literal_char | text_escape | inline_element)*
literal_char    = /[^()\\@]/
text_escape     = '\\(' | '\\)' | '\\\\' | '\\@'
                | '\\u{' /[0-9A-Fa-f]+/ '}'
inline_element  = '@' ident properties? text_body?
                  (* at least one of properties or text_body must be present *)

(* Text body — quoted form *)
quoted_text     = '"' (quoted_char | quoted_escape | '""')* '"'
quoted_char     = /[^"\\]/
quoted_escape   = '\\u{' /[0-9A-Fa-f]+/ '}'

(* Comments and whitespace *)
comment         = '//' /[^\n]*/ nl
ws              = ' ' | '\t'
nl              = '\n' | '\r\n'
ws_or_nl        = ws | nl | comment

(* Identifiers *)
ident           = /[a-zA-Z][a-zA-Z0-9_-]*/
```

Constraints not expressible in the grammar (enforced by parser
post-pass with corresponding diagnostics):

- A block has at most one `body` and at most one `properties` group.
  Source order is `ident → properties? → body?` only; `[…]` after
  body or a second `(…)`/`{…}` is a parse error.
- An `inline_element` must have at least one of `properties` or
  `text_body` — bare `@ident` is `parse.bodyless_inline_required`.
- A bare top-level `(text)` (text body without a preceding ident) is
  `parse.top_level_text`.
- `\u{N}` codepoints in `[0xD800–0xDFFF]` (surrogate halves) or
  beyond `0x10FFFF` are `parse.invalid_codepoint`.

---

## 17. Decisions log (settled)

The design pass behind this spec settled the following calls. They
appear here so future readers know not to relitigate them.

1. **`fill` separator default = `,`** with `[sep:"<char>"]` override.
   RFC 4180 quoting rules for cells containing `,` or `"`.
2. **Empty `{}` block body**: silent at grammar level. Per-element
   schemas declare body requirements; the validator warns when an
   element that needs content has none.
3. **Empty `()` text body**: hint diagnostic — author probably meant
   no body or `{}`.
4. **Bare-name block** (`pagebreak`, `hr`, `section[id:toc]`): silent
   at grammar level. Per-element schemas declare expected body shape
   (`none` / `text-required` / `text-preferred` / `block-required` /
   `any`); validator warns on mismatch.
5. **Bare `(text)` at top level**: parse error in v2.0. AST stays
   uniform — every block has a name. Implicit-`p` sugar may be added
   in v2.1 as a non-breaking ergonomic change.
6. **Inline-styling element name**: `@text[props](content)` — `@`-prefix
   for inline use only, no prefix for block use, properties always
   pre-body. The `@` removes prose-collision risk and visually
   distinguishes inline insertions.
7. **Property position**: always pre-body (`name[props](text)` or
   `name[props]{children}`). Single canonical position; post-body
   `[props]` is a parse error. Exactly one body per block; exactly one
   `[props]` block per block. Consistency over local ergonomics —
   text-body inline reads slightly less naturally than post-body would,
   but uniformity matters more.
8. **`fill`/`source` are sugar.** Desugars to per-cell blocks at
   typed-tree time. The renderer and downstream tools only see cells.
9. **Range addresses use Excel `:`** inside quoted strings:
   `at:"B2:B4"`. Single cells/columns/rows stay bare: `at:A1`, `at:B`,
   `at:5`. Formulas inside cell bodies are opaque to Stem.
10. **Text body has two forms**: bare with explicit escapes (`\(`,
    `\)`, `\\`, `\@`), or quoted (`"…"` with `""` for literal `"`).
    Quoted form is verbatim — no inline element parsing. Authors pick
    based on context; formatter preserves the choice. Decision was
    flipped from "explicit escapes only" after seeing the cost on
    spreadsheet formulas.
11. **List numbering**: `ol[start:N]` for list-level start;
    `li[at:N]` for per-item override. Both `start` and `at` are
    positional indices into the list's `[style:]` sequence
    (style-independent). After `li[at:N]`, counter continues from N+1.
