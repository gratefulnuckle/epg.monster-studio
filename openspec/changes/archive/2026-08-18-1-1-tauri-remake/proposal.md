# Proposal: 1:1 Tauri remake of epg.monster studio

## Intent

Rebuild the shipped **epg.monster studio** Windows desktop app (C# / WinUI 3 / .NET 10, v1.0-beta) as **epg.monster-studio(tauri)** — Rust backend, TypeScript frontend, Tauri v2 shell — so an operator who knows the C# app cannot tell the products apart by features, layout, copy, colors, or data.

This is a verbatim remake of **what is already implemented**, not a remake of the 2026-08-08 Phase-1 OpenSpec and not a port of the incomplete Avalonia Linux preview.

## Why

- Keep the same product on a stack the operator wants (Tauri v2).
- Preserve the existing SQLite workspace so a C# install and this remake can share `%LocalAppData%\epg.monster-studio\`.
- Avoid a second, similar tool that silently drops backups, tuner routes, or EPG habits.

## Scope

### In scope (every shipped C# surface)

- Splash (tool checks + XMLTV download %), tray minimize, About, crash report on next launch
- Nav: Add Sources, Playlist Editor, EPG Audit, Logo Audit, Stream Audit, Managed Output, TV Tuner, Settings
- Source load (file/URL + headers), virtualized browse, search all sources, play, copy URL/tvg-id, add as channel or hidden backup
- Managed playlist: edit metadata, backups with play/info/reorder/remove, now-playing, green match checks, group rename
- EPG Audit: fetch epg.monster XMLTV only, exact + fuzzy, auto-match (score + groups), catalog browse overlay, apply tvg-id/logo
- Logo Audit: missing/invalid/broken/player-reject, batch set, Clear logo, Save Logos serial PNG pack
- Stream Audit: serial ffmpeg/ffprobe, slate hash, optional blackdetect, pause/resume/`auditprocess.db`, grades A–F, Results window, weekly groups, auto-swap + undo
- Managed Output: visible vs hidden, export visible/all, tuner lineup + auto-number + collision swap, channels.json export/upload
- TV Tuner: Plex 8080 / Jellyfin 8081 / Emby 8082 / IPTV 8083, Start all, Log, Graphs, Self-test, Info, remux, SSDP/UDP 65001, live failover, Downspiral, local vs member EPG
- Settings tiles exactly as WinUI `SettingsPage.xaml`
- Members API client (ping, PUT channels, job poll, crash issues) — never log keys
- Same SQLite schema, FTS, settings keys, User-Agent, defaults

### Out of scope

- Linux Avalonia shell (source Phase 7)
- Embedded player (not shipped)
- Third configurable player (not in C# Settings)
- Authenticode / Inno Setup (follow-up packaging change)
- New ROADMAP ideas (call-sign matcher, workspace file, etc.)
- Visual redesign, new IA, renamed pages

## Approach

1. Treat `EpgMonsterStudio.Core` as the behavioral spec for Rust.
2. Treat each WinUI `Pages/*.xaml` as the layout/copy spec for TypeScript.
3. Share the C# AppData folder and schema so existing operator DBs open unchanged.
4. Implement behind Tauri commands + one Rust tuner HTTP listener, not a rewritten product model.

## Success

An operator with the live beta DB (3 sources, ~2035 managed channels) can: launch → see splash XMLTV % → browse sources → edit backups with green checks and now-playing → run EPG/Logo/Stream Audit → start IPTV on 8083 → Plex/Jellyfin/Emby Self-test pass — without thinking “the Windows app would have shown X.”
