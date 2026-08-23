# Current specs

Living source of truth after archive of `1-1-tauri-remake`. G-houl Player is v3 (`docs/V3.md`).

These requirements describe the shipped **epg.monster studio** (Tauri v2) and the
v2 launchers (`studio.ps1` / `studio.sh`). New work goes through
`openspec/changes/<name>/`.

Launcher install/uninstall (Windows winget + Scoop, Linux apt/dnf/pacman, macOS
Homebrew, `.studio-install.json`) lives in `install-scripts/`. The app specs do
not cover those scripts.

Live work is GitHub issues (`gh issue list --label openspec`). Sync a change with
`.\scripts\openspec-gh.ps1 -Change <name>`. Spec: `github-tracking/`.

`studio-tuner` `parity_walk` self-tests Plex/Jellyfin/Emby/IPTV (5/6/5/4) and checks `/lineup.json` does not contain provider stream URLs. Group rename is an in-place popup; variant Info is a dialog.
