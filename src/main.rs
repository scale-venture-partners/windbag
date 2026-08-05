use clap::{Parser, Subcommand};
use regex::Regex;
use std::path::{Path, PathBuf};
use windbag::config::Config;
use windbag::lang::Language;
use windbag::rules::{Severity, Violation};
use windbag::{comments, git, output, rules};

#[derive(Parser)]
#[command(
    name = "windbag",
    version,
    about = "Catches AI-slop comments that narrate change history instead of a real constraint"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check files for slop violations
    Check {
        /// Explicit files to check (bypasses git discovery entirely)
        paths: Vec<PathBuf>,
        /// Check files staged for commit — the default for a pre-commit hook.
        /// Only newly added comment lines are flagged, not pre-existing ones.
        #[arg(long)]
        staged: bool,
        /// Check every git-tracked file in the repo
        #[arg(long)]
        all: bool,
        /// Emit machine-readable JSON instead of human-readable text
        #[arg(long)]
        json: bool,
        /// Path to config file
        #[arg(long, default_value = "windbag.toml")]
        config: PathBuf,
    },
    /// Write a default windbag.toml to the current directory
    Init,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Check {
            paths,
            staged,
            all,
            json,
            config,
        } => run_check(paths, staged, all, json, &config),
        Commands::Init => run_init(),
    }
}

fn run_init() -> anyhow::Result<()> {
    let path = PathBuf::from("windbag.toml");
    if path.exists() {
        anyhow::bail!("windbag.toml already exists");
    }
    std::fs::write(&path, DEFAULT_CONFIG_TEMPLATE)?;
    println!("wrote {}", path.display());
    Ok(())
}

const DEFAULT_CONFIG_TEMPLATE: &str = r#"[windbag]
# Glob patterns (relative to repo root) to skip entirely.
exclude = []

[windbag.ticket_id]
enabled = true
# Generic ticket-shape pattern. Narrow this to your own tracker's prefix
# (e.g. '\bSCA-\d+\b') for higher precision.
pattern = '\b[A-Z]{2,10}-\d{2,6}\b'

[windbag.history_narration]
enabled = true

[windbag.cross_file_ref]
enabled = true

[windbag.verbose_comment]
enabled = true
max_lines = 6
max_ratio = 2.0

[windbag.obvious_comment]
enabled = true
max_words = 12
min_match_ratio = 0.85
"#;

fn run_check(
    paths: Vec<PathBuf>,
    staged: bool,
    all: bool,
    json: bool,
    config_path: &Path,
) -> anyhow::Result<()> {
    let modes = [!paths.is_empty(), staged, all]
        .iter()
        .filter(|b| **b)
        .count();
    if modes == 0 {
        anyhow::bail!("specify one of: explicit paths, --staged, or --all");
    }
    if modes > 1 {
        anyhow::bail!("--staged, --all, and explicit paths are mutually exclusive");
    }

    let config = Config::load(config_path)?;
    let exclude_patterns = compile_globs(&config.exclude)?;

    let mut violations = Vec::new();

    if !paths.is_empty() {
        for path in &paths {
            if is_excluded(path, &exclude_patterns) {
                continue;
            }
            let Some(language) = Language::from_path(path) else {
                continue;
            };
            let source = std::fs::read_to_string(path)?;
            violations.extend(check_source(path, &source, language, &config, None));
        }
    } else {
        let cwd = std::env::current_dir()?;
        let root = git::repo_root(&cwd)?;

        if staged {
            for rel_path in git::staged_files(&root)? {
                if is_excluded(&rel_path, &exclude_patterns) {
                    continue;
                }
                let Some(language) = Language::from_path(&rel_path) else {
                    continue;
                };
                let source = match git::staged_content(&root, &rel_path) {
                    Ok(s) => s,
                    Err(_) => continue, // e.g. deleted-then-staged edge cases
                };
                let added = git::added_lines(&root, &rel_path)?;
                violations.extend(check_source(
                    &rel_path,
                    &source,
                    language,
                    &config,
                    Some(&added),
                ));
            }
        } else {
            debug_assert!(all);
            for rel_path in git::all_files(&root)? {
                if is_excluded(&rel_path, &exclude_patterns) {
                    continue;
                }
                let Some(language) = Language::from_path(&rel_path) else {
                    continue;
                };
                let full_path = root.join(&rel_path);
                let Ok(source) = std::fs::read_to_string(&full_path) else {
                    continue; // binary or non-UTF8 file
                };
                violations.extend(check_source(&rel_path, &source, language, &config, None));
            }
        }
    }

    if json {
        output::print_json(&violations)?;
    } else {
        output::print_human(&violations);
    }

    let has_errors = violations.iter().any(|v| v.severity == Severity::Error);
    if has_errors {
        std::process::exit(1);
    }
    Ok(())
}

/// Extracts and checks comment blocks in `source`. When `added_lines` is
/// `Some`, only blocks that overlap at least one added line are reported —
/// this is what makes `--staged` mode flag new slop without re-litigating
/// every pre-existing comment in a file that merely got touched.
fn check_source(
    path: &Path,
    source: &str,
    language: Language,
    config: &Config,
    added_lines: Option<&std::collections::HashSet<usize>>,
) -> Vec<Violation> {
    let blocks = match comments::extract_blocks(source, language) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for block in &blocks {
        if let Some(added) = added_lines {
            let overlaps = (block.start_line..=block.end_line).any(|l| added.contains(&l));
            if !overlaps {
                continue;
            }
        }
        out.extend(rules::check_block(path, block, config));
    }
    out
}

fn compile_globs(patterns: &[String]) -> anyhow::Result<Vec<Regex>> {
    patterns
        .iter()
        .map(|p| Regex::new(&glob_to_regex(p)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(anyhow::Error::from)
}

fn glob_to_regex(glob: &str) -> String {
    let mut out = String::from("^");
    for c in glob.chars() {
        match c {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            c if r"\.+()|[]{}^$".contains(c) => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push('$');
    out
}

fn is_excluded(path: &Path, patterns: &[Regex]) -> bool {
    let path_str = path.to_string_lossy();
    patterns.iter().any(|re| re.is_match(&path_str))
}
