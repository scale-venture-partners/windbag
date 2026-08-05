use std::path::Path;
use windbag::comments::extract_blocks;
use windbag::config::Config;
use windbag::lang::Language;
use windbag::rules::{check_block, Severity};

fn violations_for(source: &str, language: Language) -> Vec<windbag::rules::Violation> {
    let config = Config::default();
    let blocks = extract_blocks(source, language).expect("parse should succeed");
    let path = Path::new("fixture");
    blocks
        .iter()
        .flat_map(|b| check_block(path, b, &config))
        .collect()
}

/// Paraphrase of the real pattern from scalevp-investment-ops/svp-infra-shared
/// PR #308 that motivated this tool: a comment narrating a ticket number,
/// the bug's prior-broken behavior, and a pointer into another file.
#[test]
fn flags_ticket_history_and_cross_file_ref_together() {
    let source = include_str!("fixtures/slop.tf");
    let violations = violations_for(source, Language::Hcl);

    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();
    assert!(
        rules.contains(&"TICKET_ID"),
        "expected TICKET_ID, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"HISTORY_NARRATION"),
        "expected HISTORY_NARRATION, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"CROSS_FILE_REF"),
        "expected CROSS_FILE_REF, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"VERBOSE_COMMENT"),
        "expected VERBOSE_COMMENT, got {:?}",
        rules
    );

    let ticket_violation = violations.iter().find(|v| v.rule == "TICKET_ID").unwrap();
    assert_eq!(ticket_violation.severity, Severity::Error);
}

/// Real Terraform patterns pulled from calibration against production infra
/// code that must NOT be flagged: a trivy suppression directive, a CVE
/// reference inside a legitimate security-rule comment, and a short
/// one-line "used by" pointer comment.
#[test]
fn does_not_flag_suppression_directives_or_security_ids() {
    let source = include_str!("fixtures/clean.tf");
    let violations = violations_for(source, Language::Hcl);
    assert!(
        violations.is_empty(),
        "expected zero violations on clean.tf, got {:#?}",
        violations
    );
}

#[test]
fn does_not_flag_short_why_comment_or_docstring() {
    let source = include_str!("fixtures/legit_why.py");
    let violations = violations_for(source, Language::Python);
    assert!(
        violations.is_empty(),
        "expected zero violations on legit_why.py, got {:#?}",
        violations
    );
}

/// Docstrings aren't `comment` nodes in tree-sitter's Python grammar at
/// all (they're string-literal expression statements), so they should
/// never even reach block extraction.
#[test]
fn python_docstrings_are_not_extracted_as_comment_blocks() {
    let source = include_str!("fixtures/legit_why.py");
    let blocks = extract_blocks(source, Language::Python).unwrap();
    assert_eq!(
        blocks.len(),
        1,
        "only the # comment should be extracted, not the docstring"
    );
}

/// JSDoc blocks are real `comment` nodes, so ticket/history/xfile rules
/// still apply to them — but the length/ratio rule must exempt them,
/// matching how Python docstrings are naturally exempt.
#[test]
fn jsdoc_slop_flags_content_rules_but_not_verbose_comment() {
    let source = include_str!("fixtures/jsdoc_slop.ts");
    let violations = violations_for(source, Language::TypeScript);
    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();

    assert!(
        rules.contains(&"TICKET_ID"),
        "expected TICKET_ID, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"HISTORY_NARRATION"),
        "expected HISTORY_NARRATION, got {:?}",
        rules
    );
    assert!(
        !rules.contains(&"VERBOSE_COMMENT"),
        "JSDoc blocks must be exempt from the length/ratio rule, got {:?}",
        rules
    );
}

#[test]
fn rust_line_comment_flags_ticket_history_and_verbose() {
    let source = include_str!("fixtures/rust_slop.rs");
    let violations = violations_for(source, Language::Rust);
    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();

    assert!(
        rules.contains(&"TICKET_ID"),
        "expected TICKET_ID, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"HISTORY_NARRATION"),
        "expected HISTORY_NARRATION, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"CROSS_FILE_REF"),
        "expected CROSS_FILE_REF, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"VERBOSE_COMMENT"),
        "expected VERBOSE_COMMENT, got {:?}",
        rules
    );
}

/// Rust's `///` doc comments are real `comment` nodes (unlike Python
/// docstrings), so content rules still apply — but they must be exempt
/// from the length/ratio rule, matching JSDoc.
#[test]
fn rust_doc_comment_flags_content_rules_but_not_verbose_comment() {
    let source = include_str!("fixtures/rust_doc_slop.rs");
    let violations = violations_for(source, Language::Rust);
    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();

    assert!(
        rules.contains(&"TICKET_ID"),
        "expected TICKET_ID, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"HISTORY_NARRATION"),
        "expected HISTORY_NARRATION, got {:?}",
        rules
    );
    assert!(
        !rules.contains(&"VERBOSE_COMMENT"),
        "Rust doc comments must be exempt from the length/ratio rule, got {:?}",
        rules
    );
}

/// Exercises the four guards OBVIOUS_COMMENT relies on to stay
/// low-noise: a clean restatement fires, a restatement-shaped comment
/// with a "because" clause is treated as a real WHY, one with extra
/// non-restatement content fails the match-ratio threshold, and a
/// multi-line block is out of scope entirely (VERBOSE_COMMENT's territory).
#[test]
fn obvious_comment_fires_on_restatements_not_on_real_why_comments() {
    let source = include_str!("fixtures/obvious_comment.rs");
    let violations = violations_for(source, Language::Rust);
    let obvious_lines: Vec<usize> = violations
        .iter()
        .filter(|v| v.rule == "OBVIOUS_COMMENT")
        .map(|v| v.line)
        .collect();

    assert!(
        obvious_lines.contains(&2),
        "expected 'increment the counter' (line 2) to fire, got {:?}",
        obvious_lines
    );
    assert!(
        obvious_lines.contains(&5),
        "expected 'return the count' (line 5) to fire, got {:?}",
        obvious_lines
    );
    assert!(
        !obvious_lines.contains(&8),
        "a comment with a 'because' clause is a real WHY, got {:?}",
        obvious_lines
    );
    assert!(
        !obvious_lines.contains(&11),
        "extra content beyond the restatement should fail the match ratio, got {:?}",
        obvious_lines
    );
    assert!(
        !obvious_lines.contains(&14),
        "multi-line blocks are out of scope for this rule, got {:?}",
        obvious_lines
    );
}

#[test]
fn inline_suppression_silences_a_specific_rule_only() {
    let source = "\
# Was missing entirely (SCA-1): windbag: ignore[TICKET_ID]
x = 1
";
    let violations = violations_for(source, Language::Python);
    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();
    assert!(
        !rules.contains(&"TICKET_ID"),
        "TICKET_ID should be suppressed, got {:?}",
        rules
    );
    assert!(
        rules.contains(&"HISTORY_NARRATION"),
        "HISTORY_NARRATION should still fire, got {:?}",
        rules
    );
}

#[test]
fn bare_inline_suppression_silences_everything_on_the_block() {
    let source = "\
# Was missing entirely (SCA-1): windbag: ignore
x = 1
";
    let violations = violations_for(source, Language::Python);
    assert!(
        violations.is_empty(),
        "bare suppression should silence all rules, got {:#?}",
        violations
    );
}
