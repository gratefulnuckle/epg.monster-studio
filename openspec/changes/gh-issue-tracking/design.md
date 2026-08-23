# Design: GitHub CLI tracking

## Labels

| Label | Use |
|-------|-----|
| `openspec` | Every issue created from a change |
| `v2` | In-scope for testers now |
| `v3` | Parked (`docs/V3.md`) |
| `install-scripts` | `studio.ps1` / `studio.sh` |
| `tracking` | Meta / process, not product UI |

Create with `gh label create` if missing. Do not rely on GitHub defaults alone.

## Issue shape

**Epic** (one per change folder):

- Title: `[openspec] <change-folder>`
- Body: purpose from `proposal.md`, links to `proposal.md`, `design.md`,
  `tasks.md`, and living specs
- Labels: `openspec`, `tracking`, plus `v2` or `v3`

**Task** (one per `- [ ]` in `tasks.md`):

- Title: `[<change-folder>] <task text>`
- Body: parent epic number, file path, acceptance from the task line
- Labels: `openspec` plus domain labels

## Mapping file

`openspec/changes/<name>/github.md` is gitignored **or** committed. Commit it so
clones do not recreate issues. Format:

```markdown
# GitHub mapping

epic: 12

| task | issue |
|------|-------|
| Add gh labels | 13 |
```

Rerun skips rows that already have an issue number. Checkboxes marked `[x]`
close the mapped issue (`gh issue close`) unless `--no-close`.
`--label` is passed once per label (never a comma-joined string).

## ISSUES.md

Keep the freeze narrative. Top of file:

```markdown
Live work: `gh issue list --label v2`
This file is the 2026-08 freeze audit, not the board.
```

Do not append new P1 items here. Open a GitHub issue instead.

## Privacy

Issue bodies MUST NOT include `epgm_…` keys, provider stream URLs, or G-houl
paths. Say “see `docs/V3.md`” for v3 player work.
