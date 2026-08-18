# Current specs

Living source of truth after archive of `1-1-tauri-remake`.

These requirements describe the shipped Tauri remake of epg.monster studio (C# WinUI 3 v1.0-beta). New work goes through `openspec/changes/<name>/`.

C# source still wins if a spec and `S:\Projects\epg.monster-studio` disagree.

Parity walk 13.1–13.4 was run 2026-08-18 against the live `%LocalAppData%\epg.monster-studio` DB (3 sources, 2035 managed, all in tuner). Playlist Editor P0 is in the Tauri UI. `studio-tuner` `parity_walk` self-tests Plex/Jellyfin/Emby/IPTV (5/6/5/4) and checks `/lineup.json` does not contain provider stream URLs. Residual editor chrome vs WinUI: group rename is `window.prompt` (not an in-place popup); variant Info is a toast.
