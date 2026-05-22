//! String-based XML serializer for OOXML parts.
//!
//! Goals:
//! - Zero allocation in the hot path beyond the `String` we're
//!   building into.
//! - Correct escaping for element text and attribute values.
//! - Writes whichever child order the caller specifies — no
//!   normalization, no reordering. Callers are responsible for
//!   emitting the schema-required order.
//!
//! Task 2 in the migration plan replaces this stub with the full
//! builder API used by every part. For task 1 we only need a couple
//! of small helpers to produce the minimal empty docx.

/// Escape `&`, `<`, `>` in element text content. Quotes are left
/// alone (they are only special inside attribute values).
pub fn escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Escape `&`, `<`, `>`, `"`, and `'` for use inside double-quoted
/// XML attribute values.
pub fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_text_handles_specials() {
        assert_eq!(escape_text("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn escape_attr_handles_quotes() {
        assert_eq!(
            escape_attr(r#"he said "hi" & 'bye'"#),
            "he said &quot;hi&quot; &amp; &apos;bye&apos;"
        );
    }
}
