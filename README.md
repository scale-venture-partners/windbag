# windbag

A pre-commit linter that catches comments narrating a change — a ticket number, what the code used to do, hedging about whether it works — instead of documenting why the current code is the way it is. Checks Python, JavaScript/TypeScript, Terraform/HCL, and Rust.

## What it detects

| Rule | Severity | Flags |
|---|---|---|
| `TICKET_ID` | error | A ticket ID in a comment (`SCA-533`). Exempts `TODO(SCA-600)`-style tracked tasks and security-advisory IDs (`CVE-`, `GHSA-`, ...). |
| `HISTORY_NARRATION` | error | "was missing", "used to be", "no longer", "silently swallows", and similar. |
| `HEDGE_LANGUAGE` | error | "should work", "hopefully", "not sure why", "i believe", and similar. |
| `CROSS_FILE_REF` | warn | A pointer to another file/line (`handler.py:147`). |
| `VERBOSE_COMMENT` | warn | A comment that's long relative to what it documents. |
| `OBVIOUS_COMMENT` | warn | A comment that just restates the line below it (`// increment the counter` above `counter += 1`). |

`error` rules fail the check; `warn` rules are reported but don't block.

## Install

```bash
cargo install --path .
```

## Use

```bash
windbag init                  # write windbag.toml with generic defaults
windbag check --staged        # check what's about to be committed
windbag check --all           # check every git-tracked file in the repo
windbag check --json --staged # machine-readable output
```

Pre-commit:

```yaml
- repo: local
  hooks:
    - id: windbag
      name: windbag
      entry: windbag check --staged
      language: system
      pass_filenames: false
      types_or: [python, javascript, jsx, ts, tsx, terraform, rust]
```

(`windbag` needs to already be on `PATH` — `language: system` doesn't install it for you.)

Suppress a false positive inline:

```python
# Was missing until v2 (LEGACY-1) — kept for the changelog.  windbag: ignore[TICKET_ID]
```

Config lives in `windbag.toml`; see [`examples/scalevp.toml`](examples/scalevp.toml) for narrowing `TICKET_ID` to a real tracker prefix instead of the generic default.

## Examples

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

```python
# Was missing entirely (SCA-533): this used to silently no-op.  <- TICKET_ID, HISTORY_NARRATION
value = fetch_value()

# This should work but I'm not sure why it fails sometimes.       <- HEDGE_LANGUAGE
retry(fetch_value)

# increment the counter                                           <- OBVIOUS_COMMENT
counter += 1

# TODO(SCA-600): revisit after Q3 pricing model ships              <- clean, exempt
schedule_followup()

# Sorted DESC because the caller assumes the first row is newest.  <- clean, real WHY
return sorted(items, reverse=True)
```

## License

MIT
