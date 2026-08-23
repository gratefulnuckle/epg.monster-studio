# Roadmap

Living list for epg.monster studio. Keys, tokens, and provider stream URLs stay out of git.

## Studio — installer / release

- **v2:** no Windows NSIS. Testers run `studio.ps1` / `studio.sh` (`--start` `--stop` `--restart` `--install` `--shortcuts`). Data is `{launch}/data`. `--install` builds a release launchable next to the repo. `--shortcuts` adds Desktop + Start Menu / applications-menu entries. Linux release artifacts are **`.deb` and AppImage**. NSIS, Authenticode, and OS AppData are **v3** (`docs/V3.md`).
- **Not shipped:** mpv and VLC. Settings path fields only.
- **ffmpeg + ffprobe:** not downloaded on splash. `studio.ps1 --install` / `studio.sh --install` checks PATH (and `tools/ffmpeg/`) and prompts to install (winget / apt / brew). Linux `.deb` still depends on distro `ffmpeg`.
- **Sync release with GitHub?** Nav **Check For Updates** (above Settings) queries GitHub Releases for this repo and opens the latest tag. Repo stays **private** until the walkthrough; unauthenticated checks 404 until it is public *and* a `v2.*` release exists. Silent in-app replace + relaunch is still **v3**.

## v3 — G-houl, NSIS, signing, OS AppData

G-houl (`ghoul.exe`), the IPTV Player nav page, GStreamer packing, Windows NSIS,
Authenticode, signed `.dmg`, in-app replace, and OS AppData are **not in v2**.
See `docs/V3.md`. G-houl stays in gitignored `./ghoul`.

See `gh issue list --label v2` for live work. `ISSUES.md` is the freeze/crash audit archive.
