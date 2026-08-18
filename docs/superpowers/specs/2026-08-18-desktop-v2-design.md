# epg.monster studio v2 — desktop cross-platform design

**Date:** 2026-08-18  
**Status:** Draft pending user review  
**Repo:** `epg.monster-studio-tauri` (this becomes production v2)

---

## Summary

Ship **epg.monster studio v2.0.0** as one Tauri v2 desktop app on **Windows, Linux, and macOS**, with **system and portable installers** on each OS, **in-app update** from GitHub Releases, and **bundled ffmpeg/mpv** per OS. After a short smoke on all three, **stop publishing** the C# WinUI and Avalonia apps. Do not delete those repos in this change.

**Out of scope:** iOS, Android, importing an existing C# `%LocalAppData%\epg.monster-studio` database, Apple notarization (optional later), redesigning the studio UI.

---

## Goals

1. Operators can install and run the full studio (sources, editor, EPG/logo/stream audit, output, tuners, settings) on Windows, Linux, and macOS.
2. Each OS offers **system** (admin) and **portable** (no admin) install.
3. Splash still delivers pinned **ffmpeg + ffprobe + mpv** for that OS. VLC remains optional/system.
4. Settings has **Update epg.monster studio**: check GitHub latest release, download the new bundle, install, relaunch.
5. About / splash show `v2.0.0 (build {github_run_number} · {short_sha})`. Updater and User-Agent use semver only: `epg.monster-studio/v2.0.0`.
6. After smoke, C# leaves production. This repo is the only studio.

---

## Non-goals

- Mobile (iOS/Android).
- Automatic copy of the v1 C# or current remake AppData database.
- Changing playlist, tuner, audit, or members behavior except where OS paths/binaries require it.
- Signing macOS with Apple Developer ID in the first ship (unsigned dmg/zip is allowed for smoke).
- Replacing GitHub Releases with a private update server.

---

## Product identity

| Field | Value |
|-------|--------|
| Window / UI name | `epg.monster studio` |
| Semver | `2.0.0` (bump on tagged releases) |
| Display version | `v2.0.0 (build N · abc1234)` |
| User-Agent | `epg.monster-studio/v2.0.0` |
| Product id | `epg.monster-studio` |
| License | GPL-3.0 |
| Source / releases | `gratefulnuckle/epg.monster-studio-tauri` |

Build `N` is `GITHUB_RUN_NUMBER`. Short SHA is seven characters of `GITHUB_SHA`. Dev builds without CI env show `v2.0.0 (dev)`.

---

## Data directory

One rule:

1. Let `app_dir` be the directory that contains the executable (on macOS, the directory that contains the `.app` bundle).
2. If `app_dir` is writable by the current user, data is `{app_dir}/data`.
3. Otherwise data is the OS user data folder:
   - Windows: `%LocalAppData%\epg.monster-studio`
   - Linux: `$XDG_DATA_HOME/epg.monster-studio` or `~/.local/share/epg.monster-studio`
   - macOS: `~/Library/Application Support/epg.monster-studio`

That folder holds `epg.monster-studio.db`, `auditprocess.db`, `logs/`, `logo/`, `offline-slates/`, `cache/`, `tool-cache/`. Schema stays C#-compatible so a **manual** copy of an old DB still opens. The app **must not** search for or copy `%LocalAppData%\epg.monster-studio` or `iptv-studio` on first launch.

**System install** lands in a non-writable app dir → user data folder.  
**Portable install** lands in a writable folder → `{app}/data`.

---

## Tools

Keep ToolBootstrap. Extend `tools-manifest.json` with **per-target** archives (os + arch), each with URL + SHA-256.

| OS | ffmpeg / ffprobe | mpv |
|----|------------------|-----|
| Windows x64 | Existing gyan.dev essentials zip (same pin as C#) | Existing mpv windows zip (same pin as C#) |
| Linux x64 | Pinned static/shared linux build (official or BtbN/gyan equivalent with a recorded SHA) | Pinned linux mpv or mpv + libs archive |
| macOS x64 + arm64 | Pinned macOS ffmpeg (universal or arch-specific) | Pinned macOS mpv |

Binary names: Windows `ffmpeg.exe` / `mpv.exe`; Unix `ffmpeg` / `mpv`. Detection and Settings labels stay “ffmpeg.exe path” / “mpv.exe path” on Windows; on Unix the same fields accept a path without `.exe` (placeholder can show `ffmpeg` / `mpv`). Do not invent a third player.

VLC: look at OS-typical locations (`/usr/bin/vlc`, `/Applications/VLC.app`, Program Files). Still optional.

---

## OS surface area (runtime)

Abstract only what is OS-specific:

- Data dir (rule above)
- Open folder (Windows `explorer`, Linux `xdg-open`, macOS `open`)
- Tool names and bootstrap URLs
- Player argv (mpv/VLC flags already mostly portable)
- Tray + close-to-tray (Tauri tray on all three desktops)
- Frameless custom titlebar stays; it already works in WebView2 / WebKit / WebKitGTK

Tuner HTTP (8080–8083), SSDP, UDP 65001, remux, serial ffmpeg audit stay the same. Linux/macOS may need firewall/permission notes in README; do not change ports.

---

## Installers

GitHub Actions on tag `v2.*` produces:

| OS | System | Portable |
|----|--------|----------|
| Windows x64 | NSIS (admin → Program Files) | Zip of the app folder (data created as `data/` on first run). NSIS also offers current-user install under `%LocalAppData%\Programs\epg.monster-studio` |
| Linux x64 | `.deb` to `/opt/epg.monster-studio` + desktop entry | AppImage |
| macOS | `.dmg` that offers `/Applications` | Zip of `epg.monster studio.app` |

macOS first ship may be **unsigned**. Document Gatekeeper right-click Open. Notarization is a later change when a Developer ID exists.

Windows current-user NSIS is writable → `{installDir}/data`. Admin Program Files is not → LocalAppData product folder.

---

## Versioning, GitHub, updater

- GitHub repository: **`gratefulnuckle/epg.monster-studio-tauri`** (this remake has no `git remote` today; creating and pushing it is part of the work). Release titles may say “epg.monster studio v2”. Do not overwrite the C# repo in this change.
- Release tags `v2.0.0`, `v2.0.1`, …
- Actions: matrix Windows / Linux / macOS, build installers, sign the **Tauri updater** artifacts with a repo secret (`TAURI_SIGNING_PRIVATE_KEY`), upload to the GitHub Release.
- About and splash read version + `build` + `gitSha` from a generate-time or compile-time file (`src-tauri/gen/version.json` or rustc `env!`).
- Settings **Diagnostics** tile gets button **Update epg.monster studio** (next to Open logs / Open crash reports).
- Click: query the GitHub Releases latest tag for `gratefulnuckle/epg.monster-studio-tauri` (first v2 repo name so the C# repo can stay published as history). If latest semver `>` running semver, download the matching OS bundle and invoke the Tauri updater install + relaunch. If already current, status text says so. If offline or 404, show the error in `#set-status` — no crash.
- “Fetch the diff” means **the new release payload**, not a source patch. The running binary is replaced wholesale.

Public repo is required for unsigned GitHub latest checks unless a token is added. Prefer public releases for v2.

---

## Smoke gate (before C# leaves production)

On each of Windows, Linux, macOS (portable or system):

1. Installer runs; app launches; splash bootstraps tools or reports a clear miss.
2. Empty workspace opens (new `{data}` or user data folder).
3. Enable/start IPTV tuner; Self-test that host (4 IPTV checks).
4. Settings → Update epg.monster studio reports current vs latest (latest may be this same tag).

Then unpublish C# WinUI and Avalonia **releases** (and download links). Leave source archives up if you want history. Do not delete those trees in this change.

The existing remake parity walk (13.1–13.4) is still useful on Windows before calling v2 done; it is not a substitute for the three-OS smoke.

---

## Implementation order

1. Version identity `2.0.0` + display metadata hook.
2. Data-dir resolver + tests (writable app dir vs fallback). Remove legacy `iptv-studio` auto-copy.
3. Unix tool names; per-OS bootstrap manifest + splash.
4. Open-folder / player path detection for Linux and macOS.
5. Settings **Update epg.monster studio** + `tauri-plugin-updater`.
6. Tauri bundle targets (NSIS, deb, AppImage, dmg).
7. GitHub repo + Actions release workflow + README install notes.
8. Three-OS smoke; then C# unpublish (manual / release notes).

---

## Risks

- **No Mac on this PC.** macOS artifacts come only from GitHub `macos-latest` runners. First unsigned build may be blocked by Gatekeeper until the operator overrides.
- **Linux WebKitGTK** on the runner vs an operator’s distro can differ. Document Ubuntu-class deps.
- **Portable AppImage** `app_dir` is often a mount; data should use the AppImage’s parent directory (`APPIMAGE` env) if that parent is writable, else XDG.
- **Updater keys** must be created once and stored as repo secrets; losing the private key means a new updater identity.

---

## Success

An operator on Windows, Ubuntu, or macOS can install (system or portable), curate a lineup, start the IPTV tuner, and update from Settings — without the C# app. That is v2.
