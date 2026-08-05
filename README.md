# windbag

**Catches comments that talk too much about the wrong thing.**

A pre-commit linter for a specific failure mode of AI-generated code: comments that narrate the *change* — a ticket number, what the code used to do, a pointer into another file explaining the history — instead of documenting the *constraint* that makes the current code non-obvious. That narration belongs in a commit message or PR description, where it can rot gracefully. Left in the code, it rots in place and misleads the next reader who has no idea a fix ever happened.

```
main.tf:218  error TICKET_ID          comment references a ticket ID (SCA-533) — that
                                       context belongs in the commit message or PR
                                       description, not the code
main.tf:218  error HISTORY_NARRATION  comment narrates the change ("was missing")
                                       instead of the current state — describe the
                                       constraint, not the history
main.tf:218  warn  VERBOSE_COMMENT    comment block is long relative to what it
                                       documents (7 comment lines, 7.0x the 1 attached
                                       code line(s))
```

## Why

Tools like [grain](https://github.com/mmartoccia/grain) and [sloppylint](https://github.com/rsionnach/sloppylint) catch AI-slop patterns in Python — bare excepts, mutable defaults, restated docstrings. Neither covers JS/TS or Terraform, and neither targets this specific pattern: a comment that reads like a changelog entry instead of documentation. It's an easy failure mode for an LLM to fall into (it just finished narrating the fix to you, so it writes that narration into the diff), and it's distinct from "comment restates the next line" — the comments this catches are often technically *true* and well-written. They're just in the wrong place.

windbag checks Python, JavaScript/TypeScript, Terraform/HCL, and Rust. It's built to run as a pre-commit hook: by default it only looks at what a commit is actually introducing, not every pre-existing comment in a file that happened to get touched.

## What it detects

| Rule | Severity | What it flags |
|---|---|---|
| `TICKET_ID` | error | A ticket/issue ID (`SCA-533`, `JIRA-1234`, ...) inside a comment. Exempts security-advisory IDs (`CVE-`, `CWE-`, `GHSA-`, `AVD-`, ...) and tool-suppression directives (`trivy:ignore`, `# noqa`, `# nosec`, ...), which collide with a generic ticket-shape pattern in any real infra codebase. |
| `HISTORY_NARRATION` | error | Phrases that narrate a change rather than describe current behavior: "was missing", "used to be", "no longer", "silently swallows", "root-caused", and similar. |
| `HEDGE_LANGUAGE` | error | Phrases that signal unreviewed uncertainty rather than a stated fact: "should work", "hopefully", "not sure why", "i believe", "for some reason", and similar. All phrase matching (this rule and `HISTORY_NARRATION`) is word-boundary aware — "i believe" won't fire inside an unrelated identifier that happens to end the same way. |
| `CROSS_FILE_REF` | warn | A pointer to another file/line/symbol (`handler.py:147`, `Class::method`). Often legitimate, sometimes a sign the real explanation lives somewhere else entirely (a PR description) and got summarized into a pointer instead. |
| `VERBOSE_COMMENT` | warn | A comment block that's long — either in absolute lines, or relative to the single statement it's attached to. Docstrings, JSDoc, and Rust doc comments (`///`, `//!`, `/**`, `/*!`) are exempt from length; free-floating blocks (section headers, file banners) are exempt from the ratio, since there's no "attached code" to be disproportionate to. |
| `OBVIOUS_COMMENT` | warn | A single-line comment that just restates the statement it's attached to — `// increment the counter` above `counter += 1`. Requires both a stock restatement verb (increment, return, call, loop, ...) and an identifier echoed from the code, and backs off the moment the comment contains real explanatory language ("because", "to avoid", "otherwise", ...). |

`TICKET_ID`, `HISTORY_NARRATION`, and `HEDGE_LANGUAGE` fail the check (non-zero exit). `CROSS_FILE_REF`, `VERBOSE_COMMENT`, and `OBVIOUS_COMMENT` are reported but don't block — they're corroborating signal, not independently reliable enough to gate a commit on.

`TICKET_ID` exempts a ticket referenced inside a `TODO`/`FIXME` marker (`TODO(SCA-600): revisit after Q3 pricing model ships`) by default — that's a forward-looking tracked task, the opposite of the backward-narrating pattern this rule targets. `HISTORY_NARRATION` still fires if the TODO also narrates a past fix. Disable via `exempt_tracked_todos = false`.

A short, non-obvious WHY comment — "sorted DESC because the caller assumes the first row is newest" — triggers nothing. That's the case this tool is designed to leave alone.

## Quick start

```bash
cargo install --path .
windbag init                 # writes windbag.toml with generic defaults
windbag check --staged       # check what's about to be committed
windbag check --all          # check every git-tracked file in the repo
windbag check --json --staged   # machine-readable output
```

Wire it into [pre-commit](https://pre-commit.com/):

```yaml
- repo: https://github.com/scalevp-investment-ops/windbag
  rev: <tag>
  hooks:
    - id: windbag
```

This currently assumes `windbag` is already on `PATH` (`language: system`) — see [Known limitations](#known-limitations).

## Design notes

- **Comment extraction uses tree-sitter**, not a hand-rolled scanner — Python triple-quoted strings, JS template literals, and HCL heredocs all have real escaping rules, and a real grammar gets them right for free.
- **File discovery goes through git** (`git ls-files`, `git diff --cached`), never a raw directory walk. Vendored dependencies and stray build artifacts are normally gitignored, so this sidesteps them entirely rather than needing a maintained exclude list.
- **`--staged` only reports comment blocks that overlap an added line** in the staged diff. Touching one line of a large file doesn't re-litigate every comment already in it.
- **Markers first, length second.** A long comment can be a legitimate explanation of a real constraint — most team style guides explicitly allow that. What's never legitimate in-code is a ticket number or "this used to be broken." Those are the high-precision, low-noise signal; length/ratio is corroboration only.

## Configuration

`windbag init` writes a `windbag.toml` with generic defaults. See [`examples/scalevp.toml`](examples/scalevp.toml) for how we narrow `TICKET_ID` to our own tracker's prefix — the built-in pattern is intentionally generic (`[A-Z]{2,10}-\d{2,6}`) so it works out of the box on any repo, but a real project prefix (`SCA-\d+`, `PROJ-\d+`) will always be more precise than the generic shape.

```toml
[windbag]
exclude = ["vendor/*"]

[windbag.ticket_id]
enabled = true
pattern = '\b[A-Z]{2,10}-\d{2,6}\b'

[windbag.verbose_comment]
max_lines = 6
max_ratio = 2.0
```

## Suppressing a false positive

Add `windbag: ignore` anywhere inside the comment block to suppress every rule on it, or scope it to specific rules:

```python
# Was missing until v2 (LEGACY-1) — kept for the changelog.  windbag: ignore[TICKET_ID]
```

## Comparison

|  | windbag | grain | sloppylint |
|---|---|---|---|
| Languages | Python, JS/TS, Terraform | Python, Markdown, commit messages | Python |
| Focus | Narrative/verbose comments | Broad AI-slop catalog (naked except, hedge words, vague TODOs, ...) | Broad AI-slop catalog (hallucinated imports, cross-language leakage, over-engineering, ...) |
| Diff-aware | Yes (`--staged`) | No | No |

Run windbag alongside grain/sloppylint if you're on Python — they check different things.

## Known limitations

- **Distribution.** v0.1 requires a local Rust toolchain (`cargo install --path .`). A prebuilt-binary-in-a-wheel distribution (the [ruff](https://github.com/astral-sh/ruff) approach — `language: python` in `.pre-commit-hooks.yaml`, no toolchain required on the consumer's machine) is the plan before this goes further than internal use.
- **Scope.** v0.1 is the comment-narration rule family only, on purpose — see [Why](#why). Broader AI-slop catalogs already exist for Python; this isn't trying to re-cover that ground.
- **Config path is resolved relative to cwd, not repo root.** Fine under pre-commit (always runs from root); running `windbag check` manually from a subdirectory silently falls back to defaults instead of finding `windbag.toml`.
- **`exclude` globs match the full relative path, anchored end-to-end.** `vendor/*` matches `vendor/x.py` but not `sub/vendor/x.py` — use `**/vendor/*` for a pattern that should match at any depth.

## License

MIT
