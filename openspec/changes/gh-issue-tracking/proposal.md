# Proposal: GitHub CLI tracking for OpenSpec and leftovers

## Why

OpenSpec living specs, `ISSUES.md`, `roadmap.md`, and `docs/*.md` are monitored
by hand. Install-script work had no spec until it broke in chat. Agents and
operators need one live board: **GitHub issues**, created and listed with `gh`.

## What changes

- Each OpenSpec **change** under `openspec/changes/<name>/` gets a GitHub
  **epic issue** plus one issue per open `- [ ]` task in `tasks.md`.
- `scripts/openspec-gh.ps1` (Windows) and `scripts/openspec-gh.sh` (Unix) create
  labels, create issues, and write `openspec/changes/<name>/github.md` so reruns
  do not duplicate.
- `ISSUES.md` stays as a **historical freeze audit**. The header points at
  `gh issue list`. New leftover work is a GitHub issue, not a new P1 paragraph.
- `openspec/AGENTS.md` tells agents to open/update GitHub issues for a change
  instead of only editing local markdown.

## Out of scope

- GitHub Projects boards, Actions bots, or auto-close on merge
- Importing every historical FIXED P0 as an issue
- Making the repo public
- G-houl / provider URLs in issue bodies

## Success

- `gh issue list --label openspec` shows active changes
- A new change can be tracked with `.\scripts\openspec-gh.ps1 -Change <name>`
- Install-script follow-up is an issue under label `install-scripts`
