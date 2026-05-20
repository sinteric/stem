# Stem — Formal Grammar

This document is the normative reference for the Stem syntax. The parser in
`crates/stem-parser` is implemented against this grammar. Examples in the
language design doc (`README.md` and `examples/`) MUST round-trip through
this grammar.

## Notation

EBNF, with these conventions:
- `'...'` literal character / string
- `?` optional
- `*` zero or more
- `+` one or more
- `|` choice
- `( )` grouping
- `/regex/` regex shorthand for character classes

Whitespace handling is explicit and tracked per production — Stem is **not**
freely whitespace-tolerant inside identifiers, but content runs are
whitespace-preserving.

## Top level

```ebnf
document       = ws_or_nl* metadata? ws_or_nl* top_content
top_content    = node*

metadata       = '[' property (',' property)* ']' (ws* nl)
```

The metadata header, if present, must be the first non-blank token in the
file and must terminate with a newline.

## Nodes

```ebnf
node           = function_call | inline_text_run

function_call  = ident arg_group+ properties?
arg_group      = '(' content_run ')'

properties     = '[' property (',' property)* ']'
property       = ident ws* ':' ws* value
value          = quoted_string | bare_value
bare_value     = /[^,\]]+/    ; trimmed of surrounding ws
quoted_string  = '"' (string_escape | /[^"\\]/)* '"'
string_escape  = '\\' ( '"' | '\\' | 'n' | 't' | 'r' )

ident          = /[a-zA-Z][a-zA-Z0-9_-]*/
```

A function call has one or more **argument groups**. Most calls use one
group (`cell(value)`); some use two so the first acts as a tag or id and
the second is the body:

```
section(cover)(
  # Title
)
```

The validator decides how each function interprets its argument list —
the grammar is permissive. Renderer-friendly access lives on `FunctionCall`
via `body()` (last group) and `header()` (the leading group when chained).

Properties parse the same way at the metadata header and at any function
call site.

## Block vs inline disambiguation

A function call is classified **after** its body is read:

- If the body contains at least one literal `\n` outside of a nested
  function body, the call is a `block`.
- Otherwise it is an `inline`.

This means the syntax `cell(foo)` is always inline, while
```
section(
  hello
)
```
is always a block. Authors don't pick — the layout picks for them.

## Body content

Both block and inline bodies are *content runs*: a mix of literal text,
nested function calls, and (for blocks) markdown-style structural lines.

```ebnf
block_body     = content_run
inline_body    = content_run    ; same productions, no newlines allowed in practice

content_run    = (nested_call | literal_char)*
nested_call    = ident '('       ; trigger condition — see below
                  content_run
                ')'
                properties?
literal_char   = /[^()]/ | balanced_parens
balanced_parens = '(' content_run ')'    ; only when not preceded by ident
```

### Disambiguating `(` inside content

The character `(` inside a body is interpreted as the **start of a nested
function call** if and only if:

1. The immediately preceding characters form an identifier (`ident`), AND
2. There is no whitespace between the identifier and the `(`.

Otherwise `(` is treated as a literal character. To keep `)` matching
predictable, the parser tracks a **literal paren depth** for the current
content run; a literal `)` only closes the enclosing function when the
literal depth is zero.

Worked examples (the underline marks what the parser treats as a nested
call):

```
cell(foo (bar) baz)             → text: "foo (bar) baz"           no nested call
cell(foo bar(baz) qux)          → text "foo ", call bar(baz), text " qux"
text(red words)[color:red]      → call text(...), properties [color:red]
note(see (ISO 8601))            → text "see (ISO 8601)"
```

### Escaping

A literal `(`, `)`, or `\` that would otherwise trigger a parse can be
escaped with `\`:

```
note(weight \(kg\))             → text: "weight (kg)"
```

Backslash-newline at the end of a line in a content run is a continuation
(line break is swallowed).

## Markdown-flavored content

Inside block bodies (only), each line is additionally interpreted as
markdown-flavored structural content during a *second pass*. The first
pass produces a tree of Stem nodes interleaved with raw text spans; the
second pass walks the raw text spans and lifts:

- `#`, `##`, `###` → headings (level 1/2/3)
- `- ` / `* ` / `1. ` line prefixes → list items
- `**bold**`, `*italic*`, `` `code` `` → inline emphasis
- Blank line → paragraph break

Inline functions encountered during the first pass survive the second pass
unchanged — they live inside whichever paragraph or list-item span the
markdown layer assigned them to. This split keeps the parser simple and
guarantees the same content prefers Stem semantics over markdown
semantics.

## Errors and recovery

The parser is **error-recovering**: on a syntax error it emits a
`Diagnostic` with line/column and continues at the next sensible
boundary. The boundaries, in order of preference:

1. The next top-level newline at literal paren depth zero
2. The closing `)` that matches the current opening `(`
3. End of file

Recovery never drops content — text up to the recovery point is preserved
as a `Content::Text` span so downstream tools (LSP, renderers) can still
see what the author wrote.

## Reserved characters

Only `(`, `)`, `[`, `]`, `,`, `:`, `\`, and a leading identifier are
syntactically meaningful. Every other byte is content.

## Stability

Identifiers are validated against a per-document-type registry
(`crates/stem-types`) at a higher level. The grammar itself accepts any
identifier — unknown functions are a *semantic* error, not a *syntactic*
one, so an editor can still highlight and an outline can still build
while the author is typing.
