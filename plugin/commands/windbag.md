---
description: Run windbag across the repo and clean up what it flags
allowed-tools: Bash(windbag:*), Read, Edit
---

Run `windbag check --all` in the project root.

For each violation, rewrite the comment to state the constraint that makes the
current code correct, or delete it if there is no such constraint. Never add a
`windbag: ignore` suppression without asking first.

Report what you changed, grouped by rule.
