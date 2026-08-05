use crate::comments::CommentBlock;
use crate::config::Config;
use regex::Regex;
use serde::Serialize;
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

fn history_narration_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    let lower = block.text.to_lowercase();
    let phrase = config
        .history_narration
        .phrases
        .iter()
        .find(|p| lower.contains(p.as_str()))?;

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

fn cross_file_ref_rule(file: &str, block: &CommentBlock) -> Option<Violation> {
    let re =
        Regex::new(r"[A-Za-z0-9_./-]+\.(py|tf|ts|js|jsx|tsx|rs)(:\d+)?(::[A-Za-z_][A-Za-z0-9_]*)?")
            .unwrap();
    let m = re.find(&block.text)?;

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

fn verbose_comment_rule(file: &str, block: &CommentBlock, config: &Config) -> Option<Violation> {
    if block.is_doc_comment {
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
