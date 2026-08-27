use anyhow::{bail, Context};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(repo_root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run `git {}`", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn repo_root(start: &Path) -> anyhow::Result<PathBuf> {
    let out = run(start, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(out.trim()))
}

/// Files staged for commit (added/copied/modified), relative to repo root.
pub fn staged_files(repo_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let out = run(
        repo_root,
        &["diff", "--cached", "--name-only", "--diff-filter=ACM"],
    )?;
    Ok(out.lines().map(PathBuf::from).collect())
}

/// All git-tracked files, relative to repo root. Deliberately avoids a raw
/// directory walk: vendored dependencies and stray worktrees are normally
/// gitignored and this sidesteps them for free.
pub fn all_files(repo_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let out = run(repo_root, &["ls-files"])?;
    Ok(out.lines().map(PathBuf::from).collect())
}

/// The staged (index) content of `file`, i.e. what would actually be
/// committed — not the working-tree copy, which may have unstaged edits.
pub fn staged_content(repo_root: &Path, file: &Path) -> anyhow::Result<String> {
    let spec = format!(":{}", file.to_string_lossy());
    run(repo_root, &["show", &spec])
}

/// 1-indexed line numbers added by the staged diff for `file`, i.e. the
/// lines this commit is actually introducing. Used so `--staged` mode
/// flags only new comment blocks, not every pre-existing comment in a
/// file that happened to get touched.
pub fn added_lines(repo_root: &Path, file: &Path) -> anyhow::Result<HashSet<usize>> {
    let out = run(
        repo_root,
        &[
            "diff",
            "--cached",
            "-U0",
            "--no-color",
            "--",
            &file.to_string_lossy(),
        ],
    )?;
    Ok(parse_added_lines(&out))
}

/// 1-indexed line numbers the working tree adds on top of `HEAD` for `file`,
/// staged or not. `None` means "no baseline to diff against" — an untracked
/// file is new in its entirety, and so is every line in it.
pub fn working_tree_added_lines(
    repo_root: &Path,
    file: &Path,
) -> anyhow::Result<Option<HashSet<usize>>> {
    let spec = file.to_string_lossy().into_owned();
    let tracked = Command::new("git")
        .current_dir(repo_root)
        .args(["ls-files", "--error-unmatch", "--", &spec])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !tracked {
        return Ok(None);
    }
    let out = run(
        repo_root,
        &["diff", "HEAD", "-U0", "--no-color", "--", &spec],
    )?;
    Ok(Some(parse_added_lines(&out)))
}

#[aristo::intent(
    "returns the 1-indexed line numbers of every `+` line in a unified diff, tracked via each hunk's `+start,count` header",
    verify = "neural",
    id = "windbag_diff_added_lines",
)]
fn parse_added_lines(diff: &str) -> HashSet<usize> {
    let mut lines = HashSet::new();
    let mut current: Option<usize> = None;
    for raw_line in diff.lines() {
        if let Some(rest) = raw_line.strip_prefix("@@ ") {
            // rest looks like "-a,b +c,d @@ ..." — parse the `+c,d` side.
            if let Some(plus_idx) = rest.find('+') {
                let after_plus = &rest[plus_idx + 1..];
                let end = after_plus.find(' ').unwrap_or(after_plus.len());
                let spec = &after_plus[..end];
                let start: usize = spec.split(',').next().unwrap_or("0").parse().unwrap_or(0);
                current = Some(start);
            }
            continue;
        }
        if raw_line.starts_with("+++") {
            continue;
        }
        if raw_line.starts_with('+') {
            if let Some(line_no) = current {
                lines.insert(line_no);
                current = Some(line_no + 1);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_hunk() {
        let diff = "\
diff --git a/foo.py b/foo.py
index abc..def 100644
--- a/foo.py
+++ b/foo.py
@@ -10,0 +11,3 @@ def foo():
+# line one
+# line two
+# line three
";
        let lines = parse_added_lines(diff);
        assert_eq!(lines, HashSet::from([11, 12, 13]));
    }

    #[test]
    fn parses_multiple_hunks() {
        let diff = "\
@@ -1,0 +2,1 @@
+added at 2
@@ -20,0 +25,2 @@
+added at 25
+added at 26
";
        let lines = parse_added_lines(diff);
        assert_eq!(lines, HashSet::from([2, 25, 26]));
    }
}
