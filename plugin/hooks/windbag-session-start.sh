#!/usr/bin/env bash
# SessionStart hook: state the comment rules up front so Claude writes clean
# comments in the first place. The PostToolUse hook is the backstop, not the
# only line of defence — a caught violation costs an extra edit round-trip.
set -uo pipefail

[[ "${WINDBAG_HOOK:-on}" == "off" ]] && exit 0

read -r -d '' context <<'EOF'
This project lints comments with `windbag`, and a PostToolUse hook will block
your edits when it finds a violation. Write comments accordingly:

- A comment documents the constraint that makes the current code correct. It
  does not narrate the change that produced it.
- No ticket IDs in comments (`SCA-533`). `TODO(SCA-600):` for a forward-looking
  tracked task is fine.
- No change history: "was missing", "used to be", "no longer", "this fix",
  "silently swallows".
- No hedging: "should work", "not sure why", "I believe", "hopefully". If you
  are unsure whether code is correct, say so to the user instead.
- No comment that restates the line below it.
- No pointers to another file and line — they rot.
- Keep comment blocks short relative to the code they document.

If a rule fires on a comment that is genuinely correct, say so and ask before
suppressing it with `windbag: ignore[RULE]`.
EOF

jq -n --arg ctx "$context" \
  '{hookSpecificOutput: {hookEventName: "SessionStart", additionalContext: $ctx}}' 2>/dev/null \
  || printf '%s\n' "$context"
