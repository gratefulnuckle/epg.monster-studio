# epg.monster-studio(tauri)

Exact **1:1 remake** of [gratefulnuckle/epg.monster-studio](https://github.com/gratefulnuckle/epg.monster-studio) (C# / .NET 10 / WinUI 3, **v1.0-beta**) as **Rust + TypeScript + Tauri v2**.

This is not a “similar playlist tool.” Every shipped surface, label, color, dialog, default, HTTP route, and SQLite row must match the Windows studio. The C# repo is the behavior oracle.

**Product window title:** `epg.monster studio`  
**Repo / project name:** `epg.monster-studio(tauri)`  
**Folder:** `epg.monster-studio-tauri`  
**License:** GNU GPL-3.0 (same as the source; ffmpeg/mpv notices in `THIRD_PARTY_NOTICES.md`)

## Status

OpenSpec written. Application code is **not** implemented yet.

Work the change:

```text
openspec/changes/1-1-tauri-remake/
```

Then `/opsx:apply` (or implement `tasks.md` by hand) and `/opsx:archive` when the remake matches the C# app.

## Source of truth

| What | Where |
|------|--------|
| Shipped Windows app | `C:\Users\jonat\projects\epg.monster-studio` · [github.com/gratefulnuckle/epg.monster-studio](https://github.com/gratefulnuckle/epg.monster-studio) |
| Behavior contract | `openspec/changes/1-1-tauri-remake/` |
| Project conventions | `openspec/project.md` |
| Historical C# spec (outdated vs v1.0-beta) | source `docs/openspec/OpenSpec.md` |
| Linux parity notes (useful UI inventory) | source `docs/openspec/linux-parity.md` |

If a spec and the C# source disagree, **the shipped WinUI + Core code wins**. Update the spec; do not “improve” the product.

## What the studio does

Windows desktop tool for curating IPTV playlists: load M3U/M3U8 sources, edit a managed list with hidden backups, match EPG ids from epg.monster XMLTV, probe streams with ffmpeg, and present the list to Plex / Jellyfin / Emby / TiviMate as a local HDHomeRun-style tuner.

## Stack (locked)

| Layer | Choice |
|-------|--------|
| Shell | Tauri v2 |
| Backend | Rust (port of `EpgMonsterStudio.Core`) |
| Frontend | TypeScript |
| Database | Same SQLite schema + same AppData path |
| Tools | Bundled portable ffmpeg + mpv (Windows) |

## Data compatibility

The remake **must** open an existing `%LocalAppData%\epg.monster-studio\epg.monster-studio.db` from the C# app without migration. Same folder, same tables, same settings JSON keys.

## License

GNU GPL v3.0. A 1:1 remake of a GPL-3.0 application is GPL-3.0.
