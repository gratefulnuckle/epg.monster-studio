# Current specs

Living source of truth after archive of `1-1-tauri-remake`. This is **v3**
(`v3.0.0`): desktop and server flavors, web login, API keys, headless, desktop
Connect. Install is `studio.ps1` / `studio.sh` only (no NSIS, no macOS Dev ID). See `docs/V3.md` for in-app replace.

These requirements describe the shipped **epg.monster studio** (Tauri v2 shell)
and the v3 launchers (`studio.ps1` / `studio.sh`). New work goes through
`openspec/changes/<name>/`. Operator install steps: `INSTALL.md`.

Launcher install/uninstall lives in `install-scripts/`. Server HTTP, keys, and
remote Connect live in `server-host/`.

Live work is GitHub issues (`gh issue list --label openspec`). Sync a change with
`.\scripts\openspec-gh.ps1 -Change <name>`. Spec: `github-tracking/`.

`studio-tuner` `parity_walk` self-tests Plex/Jellyfin/Emby/IPTV (5/6/5/4) and checks `/lineup.json` does not contain provider stream URLs. Group rename is an in-place popup; variant Info is a dialog.
