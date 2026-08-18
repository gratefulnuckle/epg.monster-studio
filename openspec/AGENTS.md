# OpenSpec instructions — epg.monster-studio(tauri)

This project uses [OpenSpec](https://github.com/Fission-AI/OpenSpec) for spec-driven development.

## Default loop

```text
/opsx:propose  →  review artifacts  →  /opsx:apply  →  /opsx:archive
```

The remake change `1-1-tauri-remake` is archived. Living requirements are in `openspec/specs/`. New work uses a new change under `openspec/changes/`.

## Hard rules for any agent

- Implement **verbatim 1:1** behavior from the C# WinUI 3 app (`gratefulnuckle/epg.monster-studio`, v1.0-beta).
- Do not invent features. Do not restyle. Do not rename nav items.
- If the spec and C# source disagree, **read the C# source and fix the spec**.
- Keep secrets out of logs: no `epgm_…` keys, no provider stream URLs.
- SQLite schema and AppData path stay compatible with the C# app.
- Probes are strictly serial (one ffmpeg at a time).
- Frontend is TypeScript. Backend is Rust. Shell is Tauri v2.

## Artifact map

| File | Role |
|------|------|
| `openspec/project.md` | Conventions that never drift |
| `openspec/changes/<name>/proposal.md` | Why / scope |
| `openspec/changes/<name>/design.md` | How (Tauri/Rust/TS) |
| `openspec/changes/<name>/tasks.md` | Implementation checklist |
| `openspec/specs/*/spec.md` | Living requirements (archived remake) |
| `openspec/changes/<name>/specs/*/spec.md` | ADDED/MODIFIED/REMOVED requirements |

## Validation

```bash
npx @fission-ai/openspec validate
```
