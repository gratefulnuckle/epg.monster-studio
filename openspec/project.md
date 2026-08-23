# Project: epg.monster studio

## Purpose

**epg.monster studio** is the desktop app for curating IPTV playlists: load M3U/M3U8 sources, edit a managed list with hidden backups, match EPG ids from epg.monster, probe streams with ffmpeg, and present the list to Plex / Jellyfin / Emby / TiviMate as a local HDHomeRun-style tuner.

Stack: **Rust + TypeScript + Tauri v2**. Window title is always **epg.monster studio**.

Not a redesigned IPTV tool. Every shipped feature, layout, label, color, default, dialog, keyboard habit, SQLite column, and tuner HTTP route MUST stay with the same observable behavior.

## Identity

| Field | Value |
|-------|--------|
| Project name | `epg.monster studio` |
| Window title / UI name | `epg.monster studio` (lowercase, unchanged) |
| Product id | `epg.monster-studio` |
| GitHub | `gratefulnuckle/epg.monster-studio` |
| Edition | `2026` |
| Version string | `v2.0.2` |
| User-Agent | `epg.monster-studio/v2.0.2` |
| License | GNU GPL-3.0 |
| Source | this repository |

## Tech stack (locked)

- **Shell:** Tauri v2 desktop (Windows, Linux, macOS)
- **Backend:** Rust — parser, SQLite, audit, EPG, tuner HTTP, members API, process spawn
- **Frontend:** TypeScript — every page, dialog, and window
- **DB:** SQLite WAL, `epg.monster-studio.db` / `auditprocess.db`
- **HTTP (tuner):** user-mode TCP, ports **8080–8083**
- **Players:** external **mpv** (default) and **VLC**
- **Probes / remux:** **ffmpeg** + **ffprobe** (system tools; `--install` prompts)
- **Launchers:** `studio.ps1` / `studio.sh` — spec `openspec/specs/install-scripts/`
- **Tracking:** GitHub issues via `gh` — spec `openspec/specs/github-tracking/`

Do not introduce React-admin dashboards, Tailwind “redesigns,” or a new information architecture.

## Rules

1. **Living specs win.** `openspec/specs/` is the source of truth. Implement those surfaces.
2. **Copy is verbatim.** Button text, headers, placeholders, toasts, empty states, About strings — same characters, including the ellipsis character `…` where the UI uses it.
3. **Tokens are exact.** See visual-parity spec. `#0C0C10` chrome, `#16161E` tiles, `#32CD32` match green, `#E57373` broken logo, `#FF6D00` issue orange.
4. **Do not simplify.** If Playlist Editor has Play + Info + up/down + Remove on a backup row, keep all five.
5. **Same schema, v2 location.** Always `{launch}/data` (`EPG_MONSTER_HOME` / repo / exe dir). OS AppData is v3. Same `epg.monster-studio.db` / `auditprocess.db` schema. Do **not** auto-copy a v1 tree.
6. **Same privacy.** Never log or upload access keys (`epgm_…`) or provider stream URLs. Crash reports redact them.
7. **Serial probes.** ffmpeg audit concurrency is **1**. Default delay **6000 ms**, timeout **15000 ms**.
8. **IPTV tuner on by default** for new installs; Plex / Jellyfin / Emby off until Settings. Start still required on TV Tuner.
9. **GPL-3.0**. Bundle ffmpeg/mpv notices.

## App surfaces (complete list)

Nav (top to bottom, then footer):

1. Add Sources
2. Playlist Editor
3. EPG Audit
4. Logo Audit
5. Stream Audit
6. Managed Output
7. TV Tuner
8. IPTV Player
9. Check For Updates (footer, above Settings)
10. Settings (footer)

Plus: splash, tray, About, crash report, Add Source dialog, backup picker, EPG auto-match, catalog browse overlay, Save Logos window, Audit pick dialog, Audit results window, Tuner lineup dialog, Tuner log, Tuner graphs, Tuner info/help.

IPTV Player / G-houl stay **v3** (`docs/V3.md`) and are hidden until then.

## Out of scope unless a change says so

- New features from ROADMAP leftovers (call-sign matcher, Authenticode pipeline)
- Redesigning nav, renaming pages, or “improving” copy
- Cloud accounts beyond the existing my.epg.monster members API
- Embedded video preview (Play is external only)
- Third configurable player (Settings has mpv + VLC only)
