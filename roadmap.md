# Roadmap

Living list for epg.monster studio. Keys, tokens, and provider stream URLs stay out of git.

**v3.0.0 is complete** for this tree. Install is **`studio.ps1` / `studio.sh` only**.
See `INSTALL.md`.

## Shipped

- Desktop and server flavors (`--install desktop` | `--install server`)
- `--start`, `--start headless`, `--stop`, `--restart`, `--makepass`, `--makekey`, `--shortcuts`, `--uninstall`
- Web login; desktop Connect with `epgs_` API key (client of the host, not a second database)
- Settings tabs (This computer / This host, Studio, Account, Advanced); auto-save
- IPTV Player / G-houl on desktop
- Web Play in the browser (mpegts.js / hls.js); server does not spawn mpv/VLC
- Check For Updates + **Install and relaunch** (needs matching files on the GitHub release)
- ffmpeg via `--install`; mpv/VLC not bundled (desktop Settings paths)
- Data in `{launch}/data` only
- Linux `.deb` / AppImage optional on tags; Windows and macOS use the scripts

## Not doing

- Windows NSIS / `-setup.exe`
- Authenticode / Azure Artifact Signing
- macOS Developer ID, notarization, signed `.dmg`
- OS AppData (`%LocalAppData%`, XDG, Application Support)
- Two-way SQLite sync between two studios
- Call-sign matcher (EPG Audit uses the catalog)

## After a release tag

Install and relaunch only works once a `v3.*` tag has uploaded
`epg-monster-studio.exe`, `studio-server.exe`, and `studio-web-dist.zip`
(see `.github/workflows/release.yml`). That is an operations step, not open product work.
