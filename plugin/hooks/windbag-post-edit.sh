#!/usr/bin/env bash
# PostToolUse hook: lint the file Claude just wrote and, on a violation, exit 2
# so the findings land back in Claude's context as a blocking error.
#
# Env:
#   WINDBAG_HOOK=off            disable without uninstalling the plugin
#   WINDBAG_HOOK_LEVEL=error    block on error-severity rules only (default: any)
#   WINDBAG_BIN=/path/to/windbag
set -uo pipefail

[[ "${WINDBAG_HOOK:-on}" == "off" ]] && exit 0
command -v jq >/dev/null 2>&1 || exit 0

payload=$(cat)
file_path=$(printf '%s' "$payload" | jq -r '.tool_input.file_path // .tool_input.notebook_path // empty')
[[ -z "$file_path" || ! -f "$file_path" ]] && exit 0

project_dir=$(printf '%s' "$payload" | jq -r '.cwd // empty')
[[ -n "$project_dir" && -d "$project_dir" ]] && cd "$project_dir" || exit 0

bin="${WINDBAG_BIN:-}"
if [[ -z "$bin" ]]; then
  for candidate in \
    "$(command -v windbag 2>/dev/null)" \
    "$HOME/.cargo/bin/windbag" \
    "${CLAUDE_PLUGIN_ROOT:-}/target/release/windbag"; do
    [[ -n "$candidate" && -x "$candidate" ]] && bin="$candidate" && break
  done
fi
[[ -z "$bin" ]] && exit 0

# Relative to the project dir so the report reads like the rest of the tool's
# output; a file outside it stays absolute.
rel_path="${file_path#"$project_dir"/}"

report=$("$bin" check --new-only --json "$rel_path" 2>/dev/null)
printf '%s' "$report" | jq -e 'type == "array" and length > 0' >/dev/null 2>&1 || exit 0

if [[ "${WINDBAG_HOOK_LEVEL:-any}" == "error" ]]; then
  printf '%s' "$report" | jq -e 'map(select(.severity == "error")) | length > 0' >/dev/null 2>&1 || exit 0
fi

{
  echo "windbag flagged comments you just wrote in ${rel_path}:"
  echo
  printf '%s' "$report" | jq -r '.[] | "  \(.file):\(.line)  \(.severity)  \(.rule)  \(.message)"'
  echo
  echo "Rewrite each flagged comment to state the constraint that makes the current code correct — or delete it if there is no such constraint. Change-history, ticket numbers, and hedging belong in the commit message, not the file. Do not add a 'windbag: ignore' suppression unless the user asked for one."
} >&2
exit 2
