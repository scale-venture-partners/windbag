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

Needs a Rust toolchain. If you don't have one:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"    # and add this line to ~/.zshrc
```

Then, from a checkout:

```bash
cargo install --path .
```

That puts `windbag` in `~/.cargo/bin`, which must be on your `PATH`.

## Use

```bash
windbag init                  # write windbag.toml with generic defaults
windbag check --staged        # check what's about to be committed
windbag check --all           # check every git-tracked file in the repo
windbag check --json --staged # machine-readable output
windbag check --new-only f.py # only comments on lines the working tree adds over HEAD
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

## Claude Code plugin

Pre-commit catches slop after it's written. The plugin catches it as it's
written: a `PostToolUse` hook runs `windbag` on every file Claude edits and
exits non-zero on a violation, so the findings go straight back to Claude as a
blocking error and it rewrites the comment before moving on. A `SessionStart`
hook states the rules up front so most edits never trip the linter at all.

```
/plugin marketplace add scalevp-investment-ops/windbag
/plugin install windbag@windbag
```

The binary has to be on `PATH` too — the plugin ships the hooks, not the linter,
and they exit quietly when they can't find it. That means a Rust toolchain
(see [Install](#install)) plus:

```bash
cargo install --git https://github.com/scalevp-investment-ops/windbag
```

TODO: publish prebuilt macOS binaries from a tagged release so installing this
doesn't require a Rust toolchain. Fine while it's a couple of people; not fine
as a team-wide ask.

To turn it on for everyone working in a given repo, commit this to that repo's
`.claude/settings.json`. Anyone who opens the repo is prompted to trust the
marketplace, and the hooks apply from their next session:

```json
{
  "extraKnownMarketplaces": {
    "windbag": {
      "source": { "source": "github", "repo": "scalevp-investment-ops/windbag" }
    }
  },
  "enabledPlugins": { "windbag@windbag": true }
}
```

Working on the plugin itself? `/plugin marketplace add /path/to/windbag` points
at a local checkout instead. Either way the install copies [`plugin/`](plugin/)
into `~/.claude/plugins/cache/` — keeping it out of the repo root is what keeps
`target/` out of the copy. That copy is a snapshot: after editing a hook,
reinstall to pick up the change.

The hook needs `windbag` on `PATH` (or `WINDBAG_BIN` set) and `jq` installed;
without either it exits quietly rather than breaking the session.

| Env var | Effect |
|---|---|
| `WINDBAG_HOOK=off` | Disable both hooks without uninstalling. |
| `WINDBAG_HOOK_LEVEL=error` | Block only on `error` rules; ignore warnings. |
| `WINDBAG_BIN` | Explicit path to the binary. |

Only comments on lines the working tree adds over `HEAD` are reported, so
editing a file doesn't re-litigate comments that were already there. Claude is
told not to silence a rule with `windbag: ignore` on its own — a false positive
should surface to you, not get suppressed.

`/windbag` sweeps the whole repo and fixes what it finds.

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
