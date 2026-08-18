# Project: epg.monster-studio(tauri)

## Purpose

Exact 1:1 remake of **epg.monster studio** (C# / WinUI 3 / .NET 10, v1.0-beta) using **Rust + TypeScript + Tauri v2**.

Not a spiritual port. Not a redesigned IPTV tool. Every shipped feature, layout, label, color, default, dialog, keyboard habit, SQLite column, and tuner HTTP route from the C# app MUST exist here with the same observable behavior.

## Identity

| Field | Value |
|-------|--------|
| Project name | `epg.monster-studio(tauri)` |
| Window title / UI name | `epg.monster studio` (lowercase, unchanged) |
| Product id / AppData folder | `epg.monster-studio` |
| Version string (parity target) | `v1.0-beta` until a later change bumps it |
| User-Agent | `epg.monster-studio/v1.0-beta` |
| License | GNU GPL-3.0 |
| Source oracle | https://github.com/gratefulnuckle/epg.monster-studio |

## Tech stack (locked)

- **Shell:** Tauri v2 desktop (Windows first; Linux later only if a follow-up change says so)
- **Backend:** Rust — parser, SQLite, audit, EPG, tuner HTTP, members API, process spawn
- **Frontend:** TypeScript — every WinUI page/dialog/window
- **DB:** SQLite WAL, schema-compatible with the C# `SqliteStore`
- **HTTP (tuner):** user-mode TCP (no HTTP.sys), ports **8080–8083**
- **Players:** external **mpv** (default) and **VLC**
- **Probes / remux:** bundled **ffmpeg** + **ffprobe**

Do not introduce React-admin dashboards, Tailwind “redesigns,” or a new information architecture.

## 1:1 remake rules

1. **C# shipped code wins.** Historical `docs/openspec/OpenSpec.md` in the source is Phase-1-era and incomplete. Implement v1.0-beta WinUI + Core.
2. **Copy is verbatim.** Button text, headers, placeholders, toasts, empty states, About strings — same characters, including the ellipsis character `…` where the source uses it.
3. **Tokens are exact.** See visual-parity spec. `#0C0C10` chrome, `#16161E` tiles, `#32CD32` match green, `#E57373` broken logo, `#FF6D00` issue orange.
4. **Do not simplify.** If Windows has Play + Info + up/down + Remove on a backup row, the remake has all five. Linux Avalonia shortcuts are **not** the target.
5. **Same schema, v2 location.** Writable app folder → `{app}/data`. Otherwise OS user data (`epg.monster-studio`). Same `epg.monster-studio.db` / `auditprocess.db` schema as C#. Do **not** auto-copy `%LocalAppData%\epg.monster-studio` or `iptv-studio`.
6. **Same privacy.** Never log or upload access keys (`epgm_…`) or provider stream URLs. Crash reports redact them.
7. **Serial probes.** ffmpeg audit concurrency is **1**. Default delay **6000 ms**, timeout **15000 ms**.
8. **IPTV tuner on by default** for new installs; Plex / Jellyfin / Emby off until Settings. Start still required on TV Tuner.
9. **GPL-3.0** for the remake. Bundle ffmpeg/mpv notices.

## App surfaces (complete list)

Nav (top to bottom, then footer):

1. Add Sources
2. Playlist Editor
3. EPG Audit
4. Logo Audit
5. Stream Audit
6. Managed Output
7. TV Tuner
8. Settings (footer)

Plus: splash, tray, About, crash report, Add Source dialog, backup picker, EPG auto-match, catalog browse overlay, Save Logos window, Audit pick dialog, Audit results window, Tuner lineup dialog, Tuner log, Tuner graphs, Tuner info/help.

## Out of scope for `1-1-tauri-remake`

- New features from ROADMAP leftovers (call-sign matcher, Authenticode pipeline, Linux Avalonia shell)
- Redesigning nav, renaming pages, or “improving” copy
- Cloud accounts beyond the existing my.epg.monster members API
- Embedded video preview (not shipped in C# v1.0-beta; Play is external only)
- Third configurable player (C# Settings only has mpv + VLC; match that)

## Reference paths (C# oracle)

```
src/EpgMonsterStudio/                 WinUI shell, pages, dialogs
src/EpgMonsterStudio.Core/            domain, SQLite, services
src/EpgMonsterStudio.Tests/           fixtures and invariants
docs/openspec/linux-parity.md         UI nuance inventory (Windows column)
docs/openspec/handoff-members-api.md  members API contract
AUDIT.md / ROADMAP.md / ISSUES.md     shipped behavior notes
```
