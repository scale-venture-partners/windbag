//! Comment extraction for SQL files.
//!
//! The files this sees are mostly dbt and SQLMesh models: SQL wrapped in
//! Jinja, which no SQL grammar parses cleanly. A parse-error-riddled tree
//! still yields the comments, but its statement boundaries fall apart, and
//! the length rule measured against a lone `with` keyword misfires. A
//! scanner that knows the three comment forms and the four things that
//! can hide a marker gives the same comments with a statement boundary
//! that holds up.

use crate::comments::{CommentBlock, LineIndex};

/// Extracts `--`, `/* */`, and Jinja `{# #}` comments, ignoring markers
/// inside `'...'` strings, `"..."` quoted identifiers, `$$...$$` bodies,
/// and `{{ }}` / `{% %}` Jinja tags, whose contents the template emits as
/// SQL rather than reads as SQL. Comments on consecutive lines merge into
/// one block. An unterminated `/*` or `{#` is a syntax error in the file,
/// and reading it as a comment would run every rule across the rest of
/// the source, so the scan stops there.
pub fn scan_sql_comments(source: &str) -> Vec<CommentBlock> {
    let line_of = LineIndex::new(source);
    let mut blocks: Vec<CommentBlock> = Vec::new();

    for (start, end) in comment_spans(source) {
        let start_line = line_of.line(start);
        let end_line = line_of.line(end.saturating_sub(1));
        let text = &source[start..end];
        match blocks.last_mut() {
            Some(prev) if prev.end_line + 1 == start_line => {
                prev.end_line = end_line;
                prev.text.push('\n');
                prev.text.push_str(text);
            }
            _ => blocks.push(CommentBlock {
                start_line,
                end_line,
                text: text.to_string(),
                attached_code_lines: 0,
                attached_code_text: None,
                is_doc_comment: false,
                is_markup: false,
            }),
        }
    }

    let lines: Vec<&str> = source.lines().collect();
    for block in &mut blocks {
        let (count, text) = attached_statement(&lines, block.end_line);
        block.attached_code_lines = count;
        block.attached_code_text = text;
    }
    blocks
}

/// Byte spans of each comment, in source order.
fn comment_spans(source: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &source[i..];
        if rest.starts_with("--") {
            let end = rest.find('\n').map(|n| i + n).unwrap_or(bytes.len());
            spans.push((i, end));
            i = end;
        } else if rest.starts_with("/*") {
            let Some(end) = delimited_end(source, i, "/*", "*/") else {
                break;
            };
            spans.push((i, end));
            i = end;
        } else if rest.starts_with("{#") {
            let Some(end) = delimited_end(source, i, "{#", "#}") else {
                break;
            };
            spans.push((i, end));
            i = end;
        } else if rest.starts_with("{{") {
            i = delimited_end(source, i, "{{", "}}").unwrap_or(source.len());
        } else if rest.starts_with("{%") {
            i = delimited_end(source, i, "{%", "%}").unwrap_or(source.len());
        } else if rest.starts_with("$$") {
            i = delimited_end(source, i, "$$", "$$").unwrap_or(source.len());
        } else if bytes[i] == b'\'' || bytes[i] == b'"' {
            i = skip_quoted(bytes, i);
        } else {
            i += 1;
        }
    }
    spans
}

/// Offset just past the `close` matching the `open` at `at`, or `None`
/// when the source ends first.
fn delimited_end(source: &str, at: usize, open: &str, close: &str) -> Option<usize> {
    let body = at + open.len();
    source[body..].find(close).map(|n| body + n + close.len())
}

/// Offset just past the closing quote of the string or quoted identifier
/// opening at `open`. A doubled quote is an escaped quote, and so is a
/// backslash-escaped one (Snowflake and MySQL accept the latter).
fn skip_quoted(bytes: &[u8], open: usize) -> usize {
    let quote = bytes[open];
    let mut i = open + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] == quote {
            if bytes.get(i + 1) == Some(&quote) {
                i += 2;
            } else {
                return i + 1;
            }
        } else {
            i += 1;
        }
    }
    bytes.len()
}

/// The statement a comment block sits above: the non-blank lines following
/// it, through the first one that ends in `;`. Nothing when the next line
/// is blank or the block ends the file.
fn attached_statement(lines: &[&str], block_end_line: usize) -> (usize, Option<String>) {
    let mut taken: Vec<&str> = Vec::new();
    for line in lines.iter().skip(block_end_line) {
        if line.trim().is_empty() {
            break;
        }
        taken.push(line);
        if line.trim_end().ends_with(';') {
            break;
        }
    }
    if taken.is_empty() {
        (0, None)
    } else {
        (taken.len(), Some(taken.join("\n")))
    }
}
