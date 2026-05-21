# Stem Schema

**Status:** draft. Sibling to `grammar.md`. This document is the
source of truth for Stem's element vocabulary. The validator, LSP,
renderer fallbacks, and AI prompts all derive from it.

The document is a mix of human prose and machine-readable schema
definitions written in Stem itself. Fenced blocks tagged
`stem-schema` are extracted by tooling and loaded into the registry;
fenced blocks tagged `stem` are examples for human readers.

---

## 0. Schema-doc element vocabulary

The blocks used inside `stem-schema` fences form a tiny sub-language.
Reference:

| Element | Purpose |
|---|---|
| `element[name:X]{...}` | Declare an element named `X`. |
| `category(...)` | One or more of: `block-container`, `block-leaf`, `block-marker`, `inline`. |
| `doc-types(...)` | One or more of: `document`, `presentation`, `sheet`, `all`. |
| `body(...)` | Permitted body shapes — combination of: `text`, `block`, `none`. |
| `parents(...)` | Names of elements that may contain this one, or classes (`root`, `any-block-container`, `any-text-body`). |
| `children(...)` | For `block-container` only — names or classes (`any-block`, `text-leaf`, `inline`). |
| `property[name:X, type:T, required:Y]("description")` | A property the element accepts. |
| `reserved[KEY:VALUE]("description")` | Property values that trigger renderer-side behavior. |
| `doc(one-line summary)` | Optional summary; falls back to surrounding prose. |

Property types: `string`, `int`, `bool`, `color`, `length`,
`address`, `enum[a, b, c]`, `style` (list-marker style).

---

## 1. Universal inline elements

Available inside any text body, regardless of doc type. Marked
`doc-types(all)` so the validator accepts them everywhere.

### `text`

Styled inline text span. The most common inline element.

```stem-schema
element[name:text]{
  category(inline, block-leaf)
  doc-types(all)
  body(text)
  parents(any-text-body, any-block-container)
  property[name:color, type:color, required:false]("Foreground color (theme name or #rrggbb)")
  property[name:bg, type:color, required:false]("Background color")
  property[name:weight, type:"enum[light, regular, bold]", required:false]("Font weight")
  property[name:style, type:"enum[italic, oblique, normal]", required:false]("Font slant")
  property[name:decoration, type:"enum[none, underline, strike]", required:false]("Text decoration")
}
```

```stem
p(The @text[color:red, weight:bold](critical) issue is at hand.)
text[color:muted](standalone styled text at block position)
```

### `footnote`

Inline reference to a footnote. The renderer collects footnotes and
numbers them per-page (paged output) or per-document (web output).

```stem-schema
element[name:footnote]{
  category(inline)
  doc-types(all)
  body(text)
  parents(any-text-body)
  property[name:id, type:string, required:false]("Stable footnote id for cross-references")
}
```

```stem
p(The figure cited @footnote(Smith 2024, p.42) is current.)
```

### `code`

Inline or block monospace code. Inline form is a single-line span;
block form is a code listing.

```stem-schema
element[name:code]{
  category(inline, block-leaf)
  doc-types(all)
  body(text)
  parents(any-text-body, any-block-container)
  property[name:lang, type:string, required:false]("Source language for syntax highlight")
  property[name:numbered, type:bool, required:false]("Show line numbers (block use only)")
}
```

```stem
p(Run @code(npm install) in your terminal.)
code[lang:rust]("fn main() { println!(\"hello\"); }")
```

### `link`

Hyperlink. Display text is the body; the destination is in `to`.

```stem-schema
element[name:link]{
  category(inline)
  doc-types(all)
  body(text)
  parents(any-text-body)
  property[name:to, type:string, required:true]("Target URL or stem cross-ref (e.g. ref://section-id)")
  property[name:title, type:string, required:false]("Tooltip text")
}
```

```stem
p(See the @link[to:"https://example.com"](docs) for more.)
```

### `date`

A semantic date span. Renders as plain text; structured for tools
that want to extract dates.

```stem-schema
element[name:date]{
  category(inline, block-leaf)
  doc-types(all)
  body(text)
  parents(any-text-body, any-block-container)
  property[name:format, type:string, required:false]("Display format hint (e.g. iso, short, long)")
}
```

```stem
p(Filed on @date(2026.05.20) by the strategy team.)
date(2026.05.20)
```

### `mention`

Reference to a person, team, or entity. Renderer may link to a
profile.

```stem-schema
element[name:mention]{
  category(inline)
  doc-types(all)
  body(text)
  parents(any-text-body)
  property[name:handle, type:string, required:false]("Backing handle/identifier")
}
```

```stem
p(On Friday @mention("@alice") closed the Acme deal.)
```

### `math`

Inline or block math expression. Body is opaque (TeX-flavored).

```stem-schema
element[name:math]{
  category(inline, block-leaf)
  doc-types(all)
  body(text)
  parents(any-text-body, any-block-container)
  property[name:display, type:"enum[inline, block]", required:false]("Render style")
}
```

```stem
p(The bound is @math("O(n \\log n)") for sorting.)
math[display:block]("\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}")
```

---

## 2. Document type

### Structural

#### `section`

Top-level structural division of a document.

```stem-schema
element[name:section]{
  category(block-container)
  doc-types(document)
  body(block, none)
  parents(root, section)
  children(any-block)
  property[name:id, type:string, required:false]("Section identifier; auto-derived from first H1 if absent")
  property[name:title, type:string, required:false]("Display title override")
  reserved[id:toc]("Generates a table of contents at render time")
}
```

```stem
section[id:cover]{
  h1(2026 Roadmap)
  date(2026.05.20)
}

section[id:toc]
```

#### `layout`

Multi-column layout container. Children are typically `col` blocks.

```stem-schema
element[name:layout]{
  category(block-container)
  doc-types(document, presentation)
  body(block)
  parents(any-block-container)
  children(col)
  property[name:kind, type:"enum[two-column, three-column, sidebar]", required:true]("Layout variant")
  property[name:gap, type:length, required:false]("Inter-column gap")
}
```

```stem
layout[kind:two-column]{
  col{ h3(Left) }
  col{ h3(Right) }
}
```

#### `col`

One column inside a `layout`. (Different from sheet `col` — that's
in §4.)

```stem-schema
element[name:col]{
  category(block-container)
  doc-types(document, presentation)
  body(block)
  parents(layout)
  children(any-block)
  property[name:width, type:string, required:false]("Width hint (1, 2, auto, or fraction)")
}
```

#### `pagebreak`

Force a page break in paged output (PDF, docx, pptx).

```stem-schema
element[name:pagebreak]{
  category(block-marker)
  doc-types(document)
  body(none)
  parents(any-block-container)
}
```

```stem
pagebreak
```

#### `hr`

Horizontal rule between sub-sections.

```stem-schema
element[name:hr]{
  category(block-marker)
  doc-types(document)
  body(none)
  parents(any-block-container)
}
```

### Headings

#### `h1` – `h6`

Six heading levels. Identical shape; the number indicates depth in
the document outline. All accept `id` (for cross-refs) and `numbered`
(for auto-numbering schemes). Parents include `root` so memo-style
docs without sections can lead with a heading.

```stem-schema
element[name:h1]{
  category(block-leaf)
  doc-types(document)
  body(text)
  parents(root, any-block-container)
  property[name:id, type:string, required:false]("Heading id for cross-refs; auto-derived from text if absent")
  property[name:numbered, type:bool, required:false]("Include in the auto-numbering scheme")
}
```

```stem-schema
element[name:h2]{
  category(block-leaf)
  doc-types(document)
  body(text)
  parents(root, any-block-container)
  property[name:id, type:string, required:false]("Heading id for cross-refs; auto-derived from text if absent")
  property[name:numbered, type:bool, required:false]("Include in the auto-numbering scheme")
}
```

```stem-schema
element[name:h3]{
  category(block-leaf)
  doc-types(document)
  body(text)
  parents(root, any-block-container)
  property[name:id, type:string, required:false]("Heading id for cross-refs; auto-derived from text if absent")
  property[name:numbered, type:bool, required:false]("Include in the auto-numbering scheme")
}
```

```stem-schema
element[name:h4]{
  category(block-leaf)
  doc-types(document)
  body(text)
  parents(root, any-block-container)
  property[name:id, type:string, required:false]("Heading id for cross-refs; auto-derived from text if absent")
  property[name:numbered, type:bool, required:false]("Include in the auto-numbering scheme")
}
```

```stem-schema
element[name:h5]{
  category(block-leaf)
  doc-types(document)
  body(text)
  parents(root, any-block-container)
  property[name:id, type:string, required:false]("Heading id for cross-refs; auto-derived from text if absent")
  property[name:numbered, type:bool, required:false]("Include in the auto-numbering scheme")
}
```

```stem-schema
element[name:h6]{
  category(block-leaf)
  doc-types(document)
  body(text)
  parents(root, any-block-container)
  property[name:id, type:string, required:false]("Heading id for cross-refs; auto-derived from text if absent")
  property[name:numbered, type:bool, required:false]("Include in the auto-numbering scheme")
}
```

```stem
h1(2026 Product Roadmap)
h2(Strategy Team)
h3(Q1 Highlights)
```

### Block content

#### `p`

Paragraph. The most common block-leaf in any document.

```stem-schema
element[name:p]{
  category(block-leaf)
  doc-types(document, presentation)
  body(text)
  parents(any-block-container)
  property[name:align, type:"enum[left, center, right, justify]", required:false]("Horizontal alignment")
}
```

```stem
p(Existing ecosystems are falling behind in the AI era.)
```

#### `note`

A callout note. Renders as a styled block (often a side panel).

```stem-schema
element[name:note]{
  category(block-leaf)
  doc-types(document, presentation)
  body(text)
  parents(any-block-container)
  property[name:kind, type:"enum[info, warning, tip, caution]", required:false]("Visual variant")
}
```

```stem
note[kind:warning](Don't forget to commit before EOD.)
```

#### `blockquote`

Multi-line quotation block.

```stem-schema
element[name:blockquote]{
  category(block-leaf)
  doc-types(document, presentation)
  body(text)
  parents(any-block-container)
  property[name:cite, type:string, required:false]("Source URL or citation")
}
```

```stem
blockquote[cite:"Tufte 1990"]("Above all else, show the data.")
```

#### `image`

An image with required alt text (accessibility) and an optional visible
caption.

```stem-schema
element[name:image]{
  category(block-marker)
  doc-types(document, presentation)
  body(none)
  parents(any-block-container)
  property[name:src, type:string, required:true]("Image path or URL")
  property[name:alt, type:string, required:true]("Alt text for accessibility")
  property[name:w, type:length, required:false]("Width (px, %, em)")
  property[name:h, type:length, required:false]("Height")
  property[name:caption, type:string, required:false]("Caption text shown below image")
}
```

```stem
image[src:"charts/mrr.png", alt:"MRR trend chart", w:60%, caption:"Q3 revenue growth"]
```

### Lists

#### `ol`

Ordered list. Children are `li` blocks.

```stem-schema
element[name:ol]{
  category(block-container)
  doc-types(document, presentation)
  body(block)
  parents(any-block-container, li)
  children(li)
  property[name:style, type:style, required:false]("Marker style: 1., A., a., I., i., 1), 가., ①, etc.")
  property[name:start, type:int, required:false]("Starting position in the style sequence")
}
```

#### `ul`

Unordered list. Children are `li` blocks.

```stem-schema
element[name:ul]{
  category(block-container)
  doc-types(document, presentation)
  body(block)
  parents(any-block-container, li)
  children(li)
  property[name:style, type:"enum[disc, circle, square, dash, none]", required:false]("Bullet style")
}
```

#### `li`

A list item. Body may be text (one-line item) or block (item with
nested content).

```stem-schema
element[name:li]{
  category(block-leaf, block-container)
  doc-types(document, presentation)
  body(text, block)
  parents(ol, ul)
  children(any-block)
  property[name:at, type:int, required:false]("Override this item's position; counter continues from at+1")
}
```

```stem
ol[style:1.]{
  li(First item)
  li(Second item)
  li[at:10](Item 10)            // override
  li(Item 11)                   // continues from 10
}

ul{
  li(Top-level text item)       // text-body form
  li{                           // block-body form (for multi-block items)
    p(Item with its own paragraph)
    ol{
      li(Nested item)
    }
  }
}
```

### Tables (document/presentation)

These are document-style tables — distinct from spreadsheet sheets
(§4). Document tables are row-oriented and don't support cell
addressing or formulas.

#### `table`

Table container. Children are `row` blocks.

```stem-schema
element[name:table]{
  category(block-container)
  doc-types(document, presentation)
  body(block)
  parents(any-block-container)
  children(row)
  property[name:border, type:"enum[none, outer, all]", required:false]("Border policy")
  property[name:stripe, type:bool, required:false]("Alternate row backgrounds")
  property[name:caption, type:string, required:false]("Caption shown above the table")
}
```

#### `row`

A table row. Children are `cell` blocks.

```stem-schema
element[name:row]{
  category(block-container)
  doc-types(document, presentation)
  body(block)
  parents(table)
  children(cell)
  property[name:kind, type:"enum[data, header, footer]", required:false]("Row role")
}
```

#### `cell` (document)

A table cell.

```stem-schema
element[name:cell]{
  category(block-leaf)
  doc-types(document, presentation)
  body(text)
  parents(row)
  property[name:colspan, type:int, required:false]("Column span")
  property[name:rowspan, type:int, required:false]("Row span")
  property[name:bg, type:color, required:false]("Background color")
  property[name:align, type:"enum[left, center, right, justify]", required:false]("Horizontal alignment")
  property[name:valign, type:"enum[top, middle, bottom]", required:false]("Vertical alignment")
}
```

```stem
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
```

---

## 3. Presentation type

### `slide`

Top-level container for a single slide.

```stem-schema
element[name:slide]{
  category(block-container)
  doc-types(presentation)
  body(block)
  parents(root)
  children(any-block, title, bullets, speaker-note, transition)
  property[name:id, type:string, required:false]("Slide identifier")
  property[name:layout, type:string, required:false]("Layout name from the theme (e.g. title-bullets, title-only)")
  property[name:background, type:color, required:false]("Slide background color")
}
```

```stem
slide[id:intro, layout:title-bullets]{
  title(Welcome)
  bullets{
    item(First point)
    item(Second point)
  }
}
```

### `title`

The slide's main title.

```stem-schema
element[name:title]{
  category(block-leaf)
  doc-types(presentation)
  body(text)
  parents(slide)
  property[name:size, type:length, required:false]("Override default title size")
}
```

### `bullets`

A list of bullet points specific to slides — equivalent to `ul`
but with presentation defaults (larger spacing, slide-style
markers).

```stem-schema
element[name:bullets]{
  category(block-container)
  doc-types(presentation)
  body(block)
  parents(slide, col)
  children(item)
  property[name:style, type:"enum[disc, dash, arrow, number]", required:false]("Marker style")
}
```

### `item`

A bullet item inside `bullets`.

```stem-schema
element[name:item]{
  category(block-leaf, block-container)
  doc-types(presentation)
  body(text, block)
  parents(bullets)
  children(bullets)
}
```

```stem
slide{
  title(Q3 Wins)
  bullets{
    item(Closed Acme)
    item(Renewed BigCo){
      bullets{
        item(Three-year term)
        item(15% expansion)
      }
    }
  }
}
```

### `speaker-note`

Notes attached to the surrounding slide. Not shown in the rendered
deck; visible in speaker view.

```stem-schema
element[name:speaker-note]{
  category(block-leaf)
  doc-types(presentation)
  body(text)
  parents(slide)
}
```

```stem
speaker-note(Remember to mention the Q4 forecast.)
```

### `transition`

Transition between slides.

```stem-schema
element[name:transition]{
  category(block-marker)
  doc-types(presentation)
  body(none)
  parents(slide)
  property[name:kind, type:"enum[none, fade, slide-left, slide-right, zoom]", required:true]("Transition type")
  property[name:duration, type:length, required:false]("Duration in seconds (0.5s default)")
}
```

---

## 4. Sheet type

Address-based — every value lives at a cell address. See
`grammar.md` §10 for cascade semantics and `fill`/`source`
desugaring.

### `sheet`

Top-level container for one tab in a workbook.

```stem-schema
element[name:sheet]{
  category(block-container)
  doc-types(sheet)
  body(block)
  parents(root)
  children(cell, col, row, fill, source, named, format, chart)
  property[name:id, type:string, required:false]("Sheet identifier, used in cross-sheet refs (Sheet!Range)")
  property[name:name, type:string, required:false]("Display name (often localized)")
  property[name:freeze, type:address, required:false]("Freeze pane at this address (rows above + columns left are frozen)")
}
```

### `col` (sheet)

Spreadsheet column-level properties. Distinct from layout `col`.

```stem-schema
element[name:col]{
  category(block-marker)
  doc-types(sheet)
  body(none)
  parents(sheet)
  property[name:at, type:address, required:true]("Column letter (A, B, AA) or range (B..D)")
  property[name:width, type:length, required:false]("Column width")
  property[name:fmt, type:"enum[number, currency, percent, date, datetime, text]", required:false]("Default number format")
}
```

### `row` (sheet)

Spreadsheet row-level properties.

```stem-schema
element[name:row]{
  category(block-marker)
  doc-types(sheet)
  body(none)
  parents(sheet)
  property[name:at, type:address, required:true]("Row number (1, 2) or range (1..5)")
  property[name:height, type:length, required:false]("Row height")
  property[name:weight, type:"enum[light, regular, bold]", required:false]("Font weight for the row")
  property[name:bg, type:color, required:false]("Background color")
}
```

### `cell` (sheet)

A single cell. May provide a value (text body) or just apply
properties to an existing cell (no body — override form).

```stem-schema
element[name:cell]{
  category(block-leaf, block-marker)
  doc-types(sheet)
  body(text, none)
  parents(sheet)
  property[name:at, type:address, required:true]("Cell address (A1, B5) — single cell only")
  property[name:fmt, type:"enum[number, currency, percent, date, datetime, text]", required:false]("Number format")
  property[name:bg, type:color, required:false]("Background color")
  property[name:align, type:"enum[left, center, right, justify]", required:false]("Horizontal alignment")
  property[name:weight, type:"enum[light, regular, bold]", required:false]("Font weight")
}
```

```stem
cell[at:A1](Item)
cell[at:B5]("=SUM(B2:B4)")              // quoted: formula has parens
cell[at:C5, bg:yellow]                  // override: properties merge, value preserved
```

### `fill`

Bulk inline data. Quoted body parsed as CSV at validate time.
Desugars to individual `cell` blocks.

```stem-schema
element[name:fill]{
  category(block-leaf)
  doc-types(sheet)
  body(text)
  parents(sheet)
  property[name:at, type:address, required:true]("Top-left anchor (single cell, e.g. A1)")
  property[name:sep, type:string, required:false]("Cell separator (default ,; use \\t for TSV)")
  property[name:has-header, type:bool, required:false]("Treat first row as a header (applies header row formatting)")
}
```

```stem
fill[at:A1]("
  Item,     Revenue,        Margin
  Widget,   42000,          0.35
  Total,    =SUM(B2:B4),    =AVERAGE(C2:C4)
")
```

### `source`

External CSV reference. Desugars to per-cell blocks at typed-tree time.

```stem-schema
element[name:source]{
  category(block-marker)
  doc-types(sheet)
  body(none)
  parents(sheet)
  property[name:file, type:string, required:true]("Path to CSV file relative to the source doc")
  property[name:at, type:address, required:true]("Top-left anchor for the imported data")
  property[name:sep, type:string, required:false]("Cell separator")
  property[name:has-header, type:bool, required:false]("Treat first row as a header")
  property[name:encoding, type:string, required:false]("Source file encoding (default utf-8)")
}
```

```stem
source[file:"data/q4-revenue.csv", at:A1, has-header:true]
```

### `named`

Declare a named range for use in formulas.

```stem-schema
element[name:named]{
  category(block-marker)
  doc-types(sheet)
  body(none)
  parents(sheet)
  property[name:name, type:string, required:true]("Name (referenced as e.g. =SUM(Revenue))")
  property[name:at, type:address, required:true]("Range, e.g. \"B2:B100\"")
}
```

```stem
named[name:Revenue, at:"B2:B100"]
```

### `format`

Apply formatting to a range without setting values.

```stem-schema
element[name:format]{
  category(block-marker)
  doc-types(sheet)
  body(none)
  parents(sheet)
  property[name:at, type:address, required:true]("Range (single cell, row, column, or rectangle)")
  property[name:bg, type:color, required:false]("Background color")
  property[name:weight, type:"enum[light, regular, bold]", required:false]("Font weight")
  property[name:align, type:"enum[left, center, right, justify]", required:false]("Horizontal alignment")
  property[name:fmt, type:"enum[number, currency, percent, date, datetime, text]", required:false]("Number format")
}
```

```stem
format[at:"A1:C1", weight:bold, bg:gray, align:center]
format[at:"B2:B100", fmt:currency]
```

### `chart`

Chart rendered from a data range. The renderer reads the range and
produces a chart image or interactive embed.

```stem-schema
element[name:chart]{
  category(block-marker)
  doc-types(document, presentation, sheet)
  body(none)
  parents(any-block-container)
  property[name:type, type:"enum[bar, line, pie, scatter, area]", required:true]("Chart type")
  property[name:data, type:string, required:true]("Range reference, e.g. \"Q4-2026!B2:C5\"")
  property[name:title, type:string, required:false]("Chart title")
  property[name:x-axis, type:string, required:false]("X-axis label")
  property[name:y-axis, type:string, required:false]("Y-axis label")
}
```

```stem
chart[type:bar, data:"Q4-2026!B2:B4", title:"분기별 매출"]
```

---

## 5. Notes for implementers

### Address parsing (sheet doc type)

`address` values use Excel-style: `A1`, `B`, `5`, ranges as
`"B2:B4"` (must quote when value contains `:`). The validator
must distinguish:
- Single cell: `A1` (letter-then-digit)
- Whole column: `B` (letter only)
- Whole row: `5` (digit only)
- Range: `"B2:B4"` or `"A1:C5"` (must be quoted)

### Body shape conflicts

When source body doesn't match the schema's `body(...)` declaration,
the validator emits `type.wrong_body_kind`. Example:

```stem
section(some text)            // schema says body(block, none), not text → warning
```

### Required properties

Missing a `required:true` property emits `type.missing_property`.

### Unknown properties

A property not declared in the schema emits `type.unknown_property`
(warning, not error) — schema may extend in future versions.

### Cell merge semantics (sheet only)

See `grammar.md` §10 "Cell merge semantics" — multiple `cell[at:X]`
blocks at the same address merge properties; later body replaces
earlier body if both supply one.

### List numbering walk

`ol[start:N]` sets the initial counter; each `li` advances it.
`li[at:N]` jumps the counter to N; subsequent items continue from N+1.
The counter is style-independent — formatted via the list's
`[style:]` at render time.

---

## 6. Reserved for future

Schema features deliberately out of scope for 1.0:

- **Inheritance / mixins** — elements that "extend" other elements.
- **Custom registries** — `[type:custom-id, registry:"./my.md"]` to
  load user-defined schemas. The current parser already accepts
  unknown elements; this just promotes them to first-class.
- **Conditional properties** — properties that are required only
  when another property has a certain value.
- **Computed property values** — references to theme tokens, doc
  metadata, or other elements' properties (`color: $theme.primary`).
- **Schema-level documentation generation** — auto-build a
  searchable element browser from this file.
