//! Shared helpers for the ASCII-table output used by `query`, `sample`, and
//! `exec`. The single job here is bounding cell width: Rust stores a dynamic
//! format width as a `u16`, so feeding `format!("{:<width$}", v)` a width past
//! `u16::MAX` panics with "Formatting argument out of range". A single wide
//! value (e.g. a 300 KB jsonb cell) would otherwise crash the whole command,
//! so every renderer clips cells through `table_cell` before measuring or
//! padding them.

/// Per-column display cap for ASCII tables. Wide cells are clipped to this many
/// characters with a trailing ellipsis. Kept far below `u16::MAX` so column
/// widths can never reach the format-width panic boundary.
pub const MAX_COL_WIDTH: usize = 200;

/// Clip `value` to at most `max` characters, appending `…` when truncated.
/// `max == 0` returns the value unclipped. Truncation is by `char`, never
/// mid-byte, so the result is always valid UTF-8.
pub fn clip_cell(value: &str, max: usize) -> String {
    if max == 0 {
        return value.to_string();
    }
    match value.char_indices().nth(max) {
        Some((byte_idx, _)) => {
            let mut out = String::with_capacity(byte_idx + '…'.len_utf8());
            out.push_str(&value[..byte_idx]);
            out.push('…');
            out
        }
        None => value.to_string(),
    }
}

/// Prepare a cell for ASCII-table display: clip to [`MAX_COL_WIDTH`], then
/// collapse newlines, carriage returns, and tabs to spaces so one logical row
/// always renders on one physical line. For faithful multi-line/structured
/// values use a machine format (`--format csv|json`) or `export` instead.
pub fn table_cell(value: &str) -> String {
    let clipped = clip_cell(value, MAX_COL_WIDTH);
    clipped
        .chars()
        .map(|c| {
            if matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// RFC 4180-style field quoting shared by `query --format csv|tsv` and `export`:
/// wrap in double quotes and double any embedded quote when the value contains
/// the delimiter, a quote, or a line break.
pub fn delimited_field(value: &str, delim: char) -> String {
    let needs_quote = value.contains(delim)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r');
    if needs_quote {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_value_unchanged() {
        assert_eq!(clip_cell("hello", MAX_COL_WIDTH), "hello");
        assert_eq!(table_cell("hello"), "hello");
    }

    #[test]
    fn wide_value_is_clipped_with_ellipsis() {
        let wide = "A".repeat(300_000);
        let out = table_cell(&wide);
        assert_eq!(out.chars().count(), MAX_COL_WIDTH + 1);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn clipped_width_never_panics_as_format_arg() {
        // The whole point: width fed to `format!("{:<w$}")` must stay under
        // u16::MAX. 300k chars clipped to ~200 means the format call is safe.
        let wide = "x".repeat(300_000);
        let w = table_cell(&wide).chars().count();
        let _ = format!("{:<w$}", table_cell(&wide), w = w);
    }

    #[test]
    fn delimited_field_quotes_when_needed() {
        assert_eq!(delimited_field("plain", ','), "plain");
        assert_eq!(delimited_field("a,b", ','), "\"a,b\"");
        assert_eq!(delimited_field("a\tb", '\t'), "\"a\tb\"");
        assert_eq!(delimited_field("he\"llo", ','), "\"he\"\"llo\"");
        assert_eq!(delimited_field("line\nbreak", ','), "\"line\nbreak\"");
        // A tab in comma-delimited output does not force quoting.
        assert_eq!(delimited_field("a\tb", ','), "a\tb");
    }

    #[test]
    fn newlines_and_tabs_collapse_to_spaces() {
        assert_eq!(table_cell("a\nb\tc\rd"), "a b c d");
    }

    #[test]
    fn clip_on_char_boundary_for_multibyte() {
        let s = "é".repeat(300); // 2 bytes each
        let out = clip_cell(&s, 10);
        assert_eq!(out.chars().count(), 11); // 10 + ellipsis
        assert!(out.ends_with('…'));
    }

    #[test]
    fn clip_max_zero_returns_unclipped() {
        assert_eq!(clip_cell("anything", 0), "anything");
    }
}
