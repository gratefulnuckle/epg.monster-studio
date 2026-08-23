# OpenSpec instructions — epg.monster studio

This project uses [OpenSpec](https://github.com/Fission-AI/OpenSpec) for spec-driven development.

## Default loop

```text
/opsx:propose  →  review artifacts  →  /opsx:apply  →  /opsx:archive
```

The remake change `1-1-tauri-remake` is archived. Living requirements are in `openspec/specs/`. New work uses a new change under `openspec/changes/`.

**Live tracking is GitHub issues** (`gh issue list --label openspec`). After adding or changing `tasks.md`, run `.\scripts\openspec-gh.ps1 -Change <name>` (or `./scripts/openspec-gh.sh`) so each open checkbox has an issue. Mapping is `openspec/changes/<name>/github.md`. Marking a task `[x]` closes that issue on the next sync. Do not treat `ISSUES.md` as the board.

## Hard rules for any agent

- Implement **verbatim** behavior from the living specs.
- Do not invent features. Do not restyle. Do not rename nav items.
- If the spec and this tree disagree, **fix the spec or the code** after checking `openspec/specs/`.
- Keep secrets out of logs: no `epgm_…` keys, no provider stream URLs.
- SQLite schema stays compatible so an existing `epg.monster-studio.db` still opens. Data is `{launch}/data`.
- Probes are strictly serial (one ffmpeg at a time).
- Frontend is TypeScript. Backend is Rust. Shell is Tauri v2.

## Artifact map

| File | Role |
|------|------|
| `openspec/project.md` | Conventions that never drift |
| `openspec/changes/<name>/proposal.md` | Why / scope |
| `openspec/changes/<name>/design.md` | How (Tauri/Rust/TS) |
| `openspec/changes/<name>/tasks.md` | Implementation checklist |
| `openspec/changes/<name>/github.md` | Epic + task issue numbers (`gh`) |
| `openspec/specs/*/spec.md` | Living requirements (archived remake) |
| `openspec/changes/<name>/specs/*/spec.md` | ADDED/MODIFIED/REMOVED requirements |

## Validation

```bash
npx @fission-ai/openspec validate
```
