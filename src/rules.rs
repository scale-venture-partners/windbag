use crate::comments::CommentBlock;
use crate::config::Config;
use regex::Regex;
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
}

#[derive(Debug, Clone, Serialize)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub rule: &'static str,
    pub severity: Severity,
    pub message: String,
    pub fixable: bool,
}

const SUPPRESS_RE_SRC: &str = r"windbag:\s*ignore(?:\[([A-Za-z_,\s]+)\])?";

fn suppressed_rules(text: &str) -> Option<Option<Vec<String>>> {
    let re = Regex::new(SUPPRESS_RE_SRC).unwrap();
    let caps = re.captures(text)?;
    match caps.get(1) {
        Some(m) => Some(Some(
            m.as_str()
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .collect(),
        )),
        None => Some(None),
    }
}

fn is_rule_suppressed(text: &str, rule: &str) -> bool {
    match suppressed_rules(text) {
        None => false,
        Some(None) => true, // bare `windbag: ignore` suppresses everything on this block
        Some(Some(rules)) => rules.iter().any(|r| r == rule),
    }
}

pub fn check_block(file: &Path, block: &CommentBlock, config: &Config) -> Vec<Violation> {
    let mut violations = Vec::new();
    let file_str = file.display().to_string();

    if config.ticket_id.enabled && !is_rule_suppressed(&block.text, "TICKET_ID") {
        if let Some(v) = ticket_id_rule(&file_str, block, config) {
            violations.push(v);
        }
    }
    if config.history_narration.enabled && !is_rule_suppressed(&block.text, "HISTORY_NARRATION") {
        if let Some(v) = history_narration_rule(&file_str, block, config) {
            violations.push(v);
        }
    }
    if config.hedge_language.enabled && !is_rule_suppressed(&block.text, "HEDGE_LANGUAGE") {
        if let Some(v) = hedge_language_rule(&file_str, block, config) {
            violations.push(v);
        }
    }
    if config.cross_file_ref.enabled && !is_rule_suppressed(&block.text, "CROSS_FILE_REF") {
        if let Some(v) = cross_file_ref_rule(&file_str, block) {
            violations.push(v);
        }
    }
    if config.verbose_comment.enabled && !is_rule_suppressed(&block.text, "VERBOSE_COMMENT") {
        if let Some(v) = verbose_comment_rule(&file_str, block, config) {
            violations.push(v);
        }
    }
    if config.obvious_comment.enabled && !is_rule_suppressed(&block.text, "OBVIOUS_COMMENT") {
        if let Some(v) = obvious_comment_rule(&file_str, block, config) {
            violations.push(v);
        }
    }
    violations
}

fn ticket_id_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    let pattern = Regex::new(&config.ticket_id.pattern).ok()?;
    let m = pattern.find(&block.text)?;

    for exempt_src in &config.ticket_id.exempt {
        if let Ok(exempt_re) = Regex::new(exempt_src) {
            if exempt_re.is_match(&block.text) {
                return None;
            }
        }
    }

    if config.ticket_id.exempt_tracked_todos && precedes_with_todo_marker(&block.text, m.start()) {
        return None;
    }

    Some(Violation {
        file: file.to_string(),
        line: block.start_line,
        end_line: block.end_line,
        rule: "TICKET_ID",
        severity: Severity::Error,
        message: format!(
            "comment references a ticket ID ({}) — that context belongs in the commit message or PR description, not the code",
            m.as_str()
        ),
        fixable: false,
    })
}

const TODO_MARKER_WINDOW: usize = 20;

/// True if a `TODO`/`FIXME` marker appears in the `TODO_MARKER_WINDOW`
/// characters immediately before a ticket-ID match — the shape of a
/// forward-looking tracked task (`TODO(SCA-600): ...`, `FIXME: SCA-600 ...`)
/// rather than the backward-narrating pattern TICKET_ID targets.
fn precedes_with_todo_marker(text: &str, match_start: usize) -> bool {
    let window_start = floor_char_boundary(text, match_start.saturating_sub(TODO_MARKER_WINDOW));
    let window = text[window_start..match_start].to_lowercase();
    window.contains("todo") || window.contains("fixme")
}

fn floor_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Word-boundary-aware phrase match — a naive substring check let a
/// first-person hedge phrase fire inside an unrelated identifier ending
/// in the same two letters, found during calibration against real code.
fn contains_phrase(lower_haystack: &str, phrase: &str) -> bool {
    let pattern = format!(r"\b{}\b", regex::escape(phrase));
    Regex::new(&pattern)
        .map(|re| re.is_match(lower_haystack))
        .unwrap_or(false)
}

fn history_narration_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    let lower = block.text.to_lowercase();
    let phrase = config
        .history_narration
        .phrases
        .iter()
        .find(|p| contains_phrase(&lower, p))?;

    Some(Violation {
        file: file.to_string(),
        line: block.start_line,
        end_line: block.end_line,
        rule: "HISTORY_NARRATION",
        severity: Severity::Error,
        message: format!(
            "comment narrates the change (\"{}\") instead of the current state — describe the constraint, not the history",
            phrase
        ),
        fixable: false,
    })
}

fn hedge_language_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    let lower = block.text.to_lowercase();
    let phrase = config
        .hedge_language
        .phrases
        .iter()
        .find(|p| contains_phrase(&lower, p))?;

    Some(Violation {
        file: file.to_string(),
        line: block.start_line,
        end_line: block.end_line,
        rule: "HEDGE_LANGUAGE",
        severity: Severity::Error,
        message: format!(
            "comment hedges (\"{}\") instead of stating a fact about the code — resolve the uncertainty or leave it out",
            phrase
        ),
        fixable: false,
    })
}

fn cross_file_ref_rule(file: &str, block: &CommentBlock) -> Option<Violation> {
    let re =
        Regex::new(
            // Longest extension first: alternation is leftmost-first, so a
            // shorter prefix listed earlier would truncate the reported path.
            r"[A-Za-z0-9_./-]+\.(tfvars|yaml|html|jsx|tsx|hcl|htm|yml|py|tf|ts|js|rs|md)(:\d+)?(::[A-Za-z_][A-Za-z0-9_]*)?",
        )
            .unwrap();
    let m = re
        .find_iter(&block.text)
        .find(|m| !is_inside_url(&block.text, m.start()))?;

    Some(Violation {
        file: file.to_string(),
        line: block.start_line,
        end_line: block.end_line,
        rule: "CROSS_FILE_REF",
        severity: Severity::Warn,
        message: format!(
            "comment points at another file/location ({}) — fine for a real pointer, but often a sign the explanation belongs elsewhere",
            m.as_str()
        ),
        fixable: false,
    })
}

/// True when a path-shaped match sits inside a URL. Documentation links
/// ending in `.html`, `.md`, or a CDN script's `.js` are references to the
/// wider world, not the in-repo pointers this rule is about.
fn is_inside_url(text: &str, match_start: usize) -> bool {
    // The whole whitespace-delimited token, not just what precedes the
    // match: a path match inside `https://host/a.md` starts at the `//`,
    // leaving only the scheme behind it.
    let start = text[..match_start]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let end = text[match_start..]
        .find(char::is_whitespace)
        .map(|i| match_start + i)
        .unwrap_or(text.len());
    let token = &text[start..end];
    token.contains("://") || token.contains("www.")
}

fn verbose_comment_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    if block.is_doc_comment || block.is_markup {
        return None;
    }
    let line_count = block.end_line - block.start_line + 1;
    let too_long = line_count > config.verbose_comment.max_lines;
    let ratio = if block.attached_code_lines > 0 {
        Some(line_count as f64 / block.attached_code_lines as f64)
    } else {
        None
    };
    let too_dense = ratio
        .map(|r| r > config.verbose_comment.max_ratio)
        .unwrap_or(false);

    if !too_long && !too_dense {
        return None;
    }

    let detail = match ratio {
        Some(r) => format!(
            "{} comment lines, {:.1}x the {} attached code line(s)",
            line_count, r, block.attached_code_lines
        ),
        None => format!("{} comment lines with no attached code", line_count),
    };

    Some(Violation {
        file: file.to_string(),
        line: block.start_line,
        end_line: block.end_line,
        rule: "VERBOSE_COMMENT",
        severity: Severity::Warn,
        message: format!(
            "comment block is long relative to what it documents ({})",
            detail
        ),
        fixable: false,
    })
}

const STOCK_VERBS: &[&str] = &[
    "increment",
    "increments",
    "decrement",
    "decrements",
    "initialize",
    "initializes",
    "init",
    "declare",
    "declares",
    "define",
    "defines",
    "create",
    "creates",
    "creating",
    "set",
    "sets",
    "setting",
    "assign",
    "assigns",
    "assigning",
    "update",
    "updates",
    "updating",
    "return",
    "returns",
    "returning",
    "call",
    "calls",
    "calling",
    "invoke",
    "invokes",
    "check",
    "checks",
    "checking",
    "loop",
    "loops",
    "looping",
    "iterate",
    "iterates",
    "iterating",
    "import",
    "imports",
    "importing",
    "print",
    "prints",
    "printing",
    "log",
    "logs",
    "logging",
    "append",
    "appends",
    "appending",
    "add",
    "adds",
    "adding",
    "remove",
    "removes",
    "removing",
    "delete",
    "deletes",
    "deleting",
    "get",
    "gets",
    "getting",
    "fetch",
    "fetches",
    "fetching",
    "retrieve",
    "retrieves",
    "retrieving",
    "open",
    "opens",
    "opening",
    "close",
    "closes",
    "closing",
];

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "to", "of", "for", "this", "that", "on", "by", "in", "and", "it", "its",
    "with", "from", "as", "is", "are", "our", "we",
];

/// Phrases that indicate the comment is explaining WHY rather than
/// restating WHAT — legitimate even when short, so they veto this rule.
const REASON_MARKERS: &[&str] = &[
    "because",
    "so that",
    "in order to",
    "to avoid",
    "to prevent",
    "otherwise",
    "workaround",
    "note:",
    "warning:",
    "important:",
];

fn obvious_comment_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    if block.is_doc_comment {
        return None;
    }
    // Multi-line blocks are either a real explanation or already caught
    // by VERBOSE_COMMENT — this rule targets the single-line "// increment
    // the counter" case specifically.
    if block.start_line != block.end_line {
        return None;
    }
    if !(1..=2).contains(&block.attached_code_lines) {
        return None;
    }
    let code_text = block.attached_code_text.as_ref()?;

    let body = strip_comment_markers(&block.text);
    let lower = body.to_lowercase();
    if REASON_MARKERS.iter().any(|m| lower.contains(m)) {
        return None;
    }

    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() || words.len() > config.obvious_comment.max_words {
        return None;
    }

    let code_words = identifier_words(code_text);
    let mut has_verb = false;
    let mut has_identifier_echo = false;
    let mut significant = 0usize;
    for w in &words {
        if STOCK_VERBS.contains(w) {
            has_verb = true;
            significant += 1;
        } else if STOPWORDS.contains(w) {
            significant += 1;
        } else if word_matches_identifier(w, &code_words) {
            has_identifier_echo = true;
            significant += 1;
        }
    }

    // Require both a verb naming the action and an identifier echoed from
    // the code: either alone is too weak a signal (e.g. "get the value"
    // with no name, or a comment that just happens to mention a variable
    // while explaining something else).
    if !has_verb || !has_identifier_echo {
        return None;
    }
    let ratio = significant as f64 / words.len() as f64;
    if ratio < config.obvious_comment.min_match_ratio {
        return None;
    }

    Some(Violation {
        file: file.to_string(),
        line: block.start_line,
        end_line: block.end_line,
        rule: "OBVIOUS_COMMENT",
        severity: Severity::Warn,
        message: "comment appears to just restate the line it's attached to — keep it only if it explains a non-obvious why".to_string(),
        fixable: false,
    })
}

fn strip_comment_markers(text: &str) -> String {
    let t = text.trim();
    let t = if let Some(rest) = t.strip_prefix("//") {
        rest
    } else if let Some(rest) = t.strip_prefix('#') {
        rest
    } else if let Some(rest) = t.strip_prefix("/*") {
        rest.strip_suffix("*/").unwrap_or(rest)
    } else if let Some(rest) = t.strip_prefix("<!--") {
        rest.strip_suffix("-->").unwrap_or(rest)
    } else {
        t
    };
    t.trim().to_string()
}

fn identifier_words(code: &str) -> HashSet<String> {
    let re = Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap();
    let mut out = HashSet::new();
    for m in re.find_iter(code) {
        for w in split_identifier(m.as_str()) {
            if w.len() > 1 {
                out.insert(w);
            }
        }
    }
    out
}

/// Splits a snake_case or camelCase identifier into lowercase words, so
/// `page_counter` and `pageCounter` both yield ["page", "counter"].
fn split_identifier(id: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;
    for c in id.chars() {
        if c == '_' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            prev_lower = false;
            continue;
        }
        if c.is_uppercase() && prev_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        current.push(c.to_ascii_lowercase());
        prev_lower = c.is_lowercase();
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn word_matches_identifier(word: &str, code_words: &HashSet<String>) -> bool {
    if code_words.contains(word) {
        return true;
    }
    if let Some(singular) = word.strip_suffix('s') {
        if code_words.contains(singular) {
            return true;
        }
    }
    false
}
