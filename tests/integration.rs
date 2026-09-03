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

#[test]
fn hedge_language_fires_on_hedging_not_on_a_real_why_comment() {
    let source = include_str!("fixtures/hedge_language.py");
    let violations = violations_for(source, Language::Python);
    let hedge_lines: Vec<usize> = violations
        .iter()
        .filter(|v| v.rule == "HEDGE_LANGUAGE")
        .map(|v| v.line)
        .collect();

    assert!(
        hedge_lines.contains(&2),
        "expected the 'should work' / 'not sure why' comment to fire, got {:?}",
        hedge_lines
    );
    assert!(
        !hedge_lines.contains(&8),
        "a comment stating a fact with 'because' should not be treated as hedging, got {:?}",
        hedge_lines
    );
}

/// TODO(TICKET)-style forward-looking tracked tasks are the opposite of
/// the backward-narrating pattern TICKET_ID targets — Google's own style
/// guide recommends exactly this convention.
#[test]
fn ticket_id_exempts_a_tracked_todo_but_not_a_narrating_ticket() {
    let tracked_todo = "# TODO(SCA-600): revisit after Q3 pricing model ships\nx = 1\n";
    let violations = violations_for(tracked_todo, Language::Python);
    assert!(
        violations.is_empty(),
        "a tracked forward-looking TODO should not fire TICKET_ID, got {:#?}",
        violations
    );

    let narrating = "# Was missing entirely (SCA-901): silently no-ops without this.\nx = 1\n";
    let violations = violations_for(narrating, Language::Python);
    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();
    assert!(
        rules.contains(&"TICKET_ID"),
        "a backward-narrating ticket reference should still fire, got {:?}",
        rules
    );
}

#[test]
fn ticket_id_todo_exemption_can_be_disabled() {
    let mut config = Config::default();
    config.ticket_id.exempt_tracked_todos = false;
    let source = "# TODO(SCA-600): revisit after Q3 pricing model ships\nx = 1\n";
    let blocks = extract_blocks(source, Language::Python).unwrap();
    let violations: Vec<_> = blocks
        .iter()
        .flat_map(|b| check_block(Path::new("fixture"), b, &config))
        .collect();
    let rules: Vec<&str> = violations.iter().map(|v| v.rule).collect();
    assert!(
        rules.contains(&"TICKET_ID"),
        "disabling exempt_tracked_todos should let TICKET_ID fire, got {:?}",
        rules
    );
}

#[test]
fn yaml_comment_flags_ticket_history_and_cross_file_ref() {
    let source = include_str!("fixtures/slop.yml");
    let violations = violations_for(source, Language::Yaml);
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
}

/// A `#` inside a quoted YAML scalar is data, not a comment — the reason
/// this language goes through the grammar instead of a line scan.
#[test]
fn yaml_hash_inside_a_quoted_scalar_is_not_a_comment() {
    let source = include_str!("fixtures/clean.yml");
    let blocks = extract_blocks(source, Language::Yaml).unwrap();
    assert!(
        blocks.iter().all(|b| !b.text.contains("no longer works")),
        "the quoted scalar was extracted as a comment: {:#?}",
        blocks
    );

    let violations = violations_for(source, Language::Yaml);
    assert!(
        violations.is_empty(),
        "expected zero violations on clean.yml, got {:#?}",
        violations
    );
}

#[test]
fn html_comment_flags_ticket_and_history() {
    let source = include_str!("fixtures/slop.html");
    let violations = violations_for(source, Language::Html);
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
}

/// Markdown has no tree-sitter grammar wired up, so the scanner carries
/// the whole burden of telling a real `<!-- -->` from one displayed as
/// sample markup inside a fenced code block.
#[test]
fn markdown_flags_a_real_comment_but_not_one_inside_a_code_fence() {
    let source = include_str!("fixtures/slop.md");
    let blocks = extract_blocks(source, Language::Markdown).unwrap();
    assert_eq!(
        blocks.len(),
        1,
        "only the unfenced comment should be extracted, got {:#?}",
        blocks
    );
    assert_eq!(blocks[0].start_line, 3);
    assert_eq!(blocks[0].end_line, 3);

    let violations = violations_for(source, Language::Markdown);
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
        violations.iter().all(|v| v.line == 3),
        "the fenced comment must not be reported, got {:#?}",
        violations
    );
}

/// A multi-line comment keeps its true span, and two on consecutive lines
/// read as one block — the same merge the tree-sitter path performs.
#[test]
fn markdown_spans_multiple_lines_and_merges_adjacent_comments() {
    let source = "<!-- first\nstill first -->\n<!-- second -->\n\n<!-- separate -->\n";
    let blocks = extract_blocks(source, Language::Markdown).unwrap();
    assert_eq!(blocks.len(), 2, "got {:#?}", blocks);
    assert_eq!((blocks[0].start_line, blocks[0].end_line), (1, 3));
    assert_eq!((blocks[1].start_line, blocks[1].end_line), (5, 5));
}

/// Calibration against eight production repos found the length rule firing
/// on the idiomatic config shape — a few lines of explanation above a
/// one-line key — for 35 of 46 markup findings, while every real slop
/// comment it sat on was already caught by a content rule.
#[test]
fn markup_is_exempt_from_the_length_rule_that_still_fires_on_code() {
    let source = "# one\n# two\n# three\nkey = 1\n";

    let python: Vec<&str> = violations_for(source, Language::Python)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(
        python.contains(&"VERBOSE_COMMENT"),
        "the same shape must still fire on code, got {:?}",
        python
    );

    let yaml: Vec<&str> = violations_for(source, Language::Yaml)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(
        !yaml.contains(&"VERBOSE_COMMENT"),
        "markup must be exempt from the length rule, got {:?}",
        yaml
    );
}

/// A documentation link ending in `.html` or `.md` points at the wider
/// world, not at a path in this repo that can rot.
#[test]
fn cross_file_ref_skips_a_url_but_still_flags_a_repo_path() {
    let url = "# See https://circleci.com/docs/2.0/Executor%20Reference.html for the shape.\nversion: 2.1\n";
    let rules: Vec<&str> = violations_for(url, Language::Yaml)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(
        !rules.contains(&"CROSS_FILE_REF"),
        "a documentation URL is not a repo pointer, got {:?}",
        rules
    );

    // A URL with no character outside the path charset is the harder case:
    // the path match starts at the `//`, leaving only `https:` behind it.
    let plain = "# See https://example.com/docs/guide.md for the shape.\nversion: 2.1\n";
    let rules: Vec<&str> = violations_for(plain, Language::Yaml)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(
        !rules.contains(&"CROSS_FILE_REF"),
        "a plain documentation URL is not a repo pointer, got {:?}",
        rules
    );

    let path = "# Mirrors the bucket policy in modules/legacy/iam.tf.\nversion: 2.1\n";
    let rules: Vec<&str> = violations_for(path, Language::Yaml)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(
        rules.contains(&"CROSS_FILE_REF"),
        "a real in-repo path must still fire, got {:?}",
        rules
    );
}

/// Documentation about Markdown names the comment marker in prose. Reading
/// an unterminated `<!--` as a comment would run the phrase rules across
/// every line after it, turning ordinary prose into blocking errors.
#[test]
fn markdown_ignores_an_unterminated_comment_marker() {
    let source =
        "Use the `<!--` marker to open a comment.\n\nThe exporter no longer runs nightly.\n";
    let blocks = extract_blocks(source, Language::Markdown).unwrap();
    assert!(
        blocks.is_empty(),
        "an unterminated marker is prose, got {:#?}",
        blocks
    );
    assert!(
        violations_for(source, Language::Markdown).is_empty(),
        "prose after an unterminated marker must not be checked"
    );
}

#[test]
fn sql_extension_maps_to_the_sql_language() {
    assert_eq!(
        Language::from_path(Path::new("models/x.sql")),
        Some(Language::Sql)
    );
}

/// dbt and SQLMesh models are Jinja-templated SQL: `--` and `/* */`
/// comments sit next to `{# #}` Jinja comments, and all three carry the
/// same kind of slop.
#[test]
fn sql_flags_slop_in_line_block_and_jinja_comments() {
    let source = include_str!("fixtures/slop.sql");
    let blocks = extract_blocks(source, Language::Sql).unwrap();
    let lines: Vec<usize> = blocks.iter().map(|b| b.start_line).collect();
    assert_eq!(lines, vec![3, 5, 18, 23], "got {:#?}", blocks);

    let violations = violations_for(source, Language::Sql);
    let at = |line: usize| -> Vec<&str> {
        violations
            .iter()
            .filter(|v| v.line == line)
            .map(|v| v.rule)
            .collect()
    };
    for line in [3, 5] {
        assert!(
            at(line).contains(&"TICKET_ID"),
            "line {line}: {:?}",
            at(line)
        );
        assert!(
            at(line).contains(&"HISTORY_NARRATION"),
            "line {line}: {:?}",
            at(line)
        );
    }
    assert!(at(18).contains(&"HEDGE_LANGUAGE"), "line 18: {:?}", at(18));
    assert!(
        at(23).is_empty(),
        "the real WHY comment must be clean, got {:?}",
        at(23)
    );
}

/// A `--` or `/* */` inside a string literal, or inside a `{{ }}` Jinja
/// expression, is data the template emits, not a comment.
#[test]
fn sql_comment_markers_inside_strings_and_jinja_are_not_comments() {
    let source = include_str!("fixtures/slop.sql");
    let blocks = extract_blocks(source, Language::Sql).unwrap();
    assert!(
        blocks.iter().all(|b| !b.text.contains("not a comment")),
        "a literal was extracted as a comment: {:#?}",
        blocks
    );

    let dollar = "select $$ -- not a comment $$ as s;\n";
    let blocks = extract_blocks(dollar, Language::Sql).unwrap();
    assert!(blocks.is_empty(), "got {:#?}", blocks);

    let escaped = "select 'it''s -- not a comment' as s;\n";
    let blocks = extract_blocks(escaped, Language::Sql).unwrap();
    assert!(blocks.is_empty(), "got {:#?}", blocks);
}

/// SQL has no braces to mislead a line scan, and a statement ends at a
/// blank line or a `;`, so that is the code a comment is measured against.
#[test]
fn sql_attached_code_runs_to_a_blank_line_or_semicolon() {
    let to_semicolon = "-- why\nselect 1\nfrom t\nwhere x = 1;\nselect 2\n";
    let blocks = extract_blocks(to_semicolon, Language::Sql).unwrap();
    assert_eq!(blocks.len(), 1, "got {:#?}", blocks);
    assert_eq!(blocks[0].attached_code_lines, 3);
    assert_eq!(
        blocks[0].attached_code_text.as_deref(),
        Some("select 1\nfrom t\nwhere x = 1;")
    );

    let to_blank = "-- why\nselect 1\nfrom t\n\nselect 2\n";
    let blocks = extract_blocks(to_blank, Language::Sql).unwrap();
    assert_eq!(blocks[0].attached_code_lines, 2);

    let floating = "-- why\n\nselect 1\n";
    let blocks = extract_blocks(floating, Language::Sql).unwrap();
    assert_eq!(blocks[0].attached_code_lines, 0);
    assert_eq!(blocks[0].attached_code_text, None);
}

/// SQL is code, not markup: the length rule and the restatement rule both
/// apply, the way they do for Python.
#[test]
fn sql_is_subject_to_the_length_and_restatement_rules() {
    let verbose = "-- one\n-- two\n-- three\nselect 1\n";
    let rules: Vec<&str> = violations_for(verbose, Language::Sql)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(rules.contains(&"VERBOSE_COMMENT"), "got {:?}", rules);

    let obvious = "-- create the users table\ncreate table users (id int)\n";
    let rules: Vec<&str> = violations_for(obvious, Language::Sql)
        .iter()
        .map(|v| v.rule)
        .collect();
    assert!(rules.contains(&"OBVIOUS_COMMENT"), "got {:?}", rules);
}

/// A block comment keeps its true span, and adjacent comments of any
/// flavor on consecutive lines merge into one block.
#[test]
fn sql_multiline_block_comment_merges_with_an_adjacent_line_comment() {
    let source = "/* first\nstill first */\n-- second\n{# third #}\n\n-- separate\nselect 1\n";
    let blocks = extract_blocks(source, Language::Sql).unwrap();
    assert_eq!(blocks.len(), 2, "got {:#?}", blocks);
    assert_eq!((blocks[0].start_line, blocks[0].end_line), (1, 4));
    assert_eq!((blocks[1].start_line, blocks[1].end_line), (6, 6));
}
