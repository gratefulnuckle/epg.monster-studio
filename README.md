<p align="center">
  <img src="https://epg.monster/img/logo.png" alt="epg.monster">
</p>

# epg.monster studio

[![Language](https://img.shields.io/badge/language-TypeScript-3178C6?style=for-the-badge&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?style=for-the-badge&logo=tauri&logoColor=black)](https://tauri.app/)

[![Windows](https://img.shields.io/badge/Windows-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://learn.microsoft.com/windows/)
[![Linux](https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://www.kernel.org/)
[![macOS](https://img.shields.io/badge/macOS-000000?style=for-the-badge&logo=apple&logoColor=white)](https://www.apple.com/macos/)

[![License](https://img.shields.io/badge/license-GPL--3.0-blue?style=for-the-badge)](LICENSE)
[![Release](https://img.shields.io/github/v/release/gratefulnuckle/epg.monster-studio?style=for-the-badge)](https://github.com/gratefulnuckle/epg.monster-studio/releases)

**epg.monster studio** is a desktop app for curating IPTV playlists: load M3U/M3U8
sources, edit a managed list with hidden backups, match EPG ids from epg.monster,
probe streams with ffmpeg, and present the list to Plex / Jellyfin / Emby / TiviMate
as a local HDHomeRun-style tuner.

This tree is the **v2** app: **Tauri v2 + Rust + TypeScript**, on **Windows, Linux,
and macOS**. Window title is always **epg.monster studio**. Edition **2026**,
semver **2.0.2**.

This is an operator tool. Use it only with sources you have the right to use.

---

## Install (v2 testers)

| OS | v2 | v3 (later) |
|----|----|------------|
| Windows | `.\studio.ps1` (dev) | NSIS + Authenticode |
| Linux | `./studio.sh`, `.deb`, or AppImage | — |
| macOS | `./studio.sh` | signed `.dmg` |

### Dev (all platforms)

Needs **Rust** (stable) and **Node 22+**.

**Windows (PowerShell)**

```powershell
.\studio.ps1              # deps, release .exe, then start
.\studio.ps1 --stop
.\studio.ps1 --start
.\studio.ps1 --restart
.\studio.ps1 --install    # Node/Rust via winget; ffmpeg/mpv/VLC via scoop then winget; build the .exe
.\studio.ps1 --shortcuts  # Desktop + Start Menu
.\studio.ps1 --uninstall  # stop, remove shortcuts + launchable; optional tools (keeps .\data)
```

**Linux / macOS**

```bash
chmod +x studio.sh
./studio.sh               # deps, release binary, then start
./studio.sh --stop
./studio.sh --start
./studio.sh --restart
./studio.sh --install     # Node, Rust, ffmpeg, mpv/VLC (apt/dnf/pacman or brew); build
./studio.sh --shortcuts   # Desktop + applications menu
./studio.sh --uninstall   # stop, remove shortcuts + launchable; optional tools (keeps ./data)
```

The script sets `EPG_MONSTER_HOME` to the repo so SQLite, logs, and cache are
`./data`. **`--install`** checks Node, Rust, ffmpeg/ffprobe, mpv, VLC, and (on Linux)
GTK/WebKit, prompts to install anything missing, then **builds a release launchable**
next to the repo (`epg-monster-studio.exe` / `epg-monster-studio`). **`--shortcuts`**
pins that file on the Desktop and in the Start Menu (Windows) or applications menu
(Linux / macOS `~/Applications`). Splash still **checks** tools; it does not
download them. ffmpeg is required; mpv and VLC are optional Play engines (not shipped).

**`--install` package managers**

| OS | How missing tools are offered |
|----|-------------------------------|
| Windows | Scoop first (installs Scoop if you agree), then winget |
| Linux | `apt-get`, `dnf`, or `pacman` — each step asks, then `sudo` |
| macOS | Homebrew (installs brew if you agree). VLC is a brew cask |

**`--uninstall`** stops the app and, with prompts, can remove shortcuts, the copied
launchable, Node, Rust, ffmpeg, mpv, and VLC. **`./data` is never deleted.** G-houl
Player is **v3**.

Linux compile packages if you prefer apt yourself:

```bash
sudo apt install -y \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev patchelf ffmpeg
```

On Ubuntu 24.04 / Mint 22, `libappindicator3-dev` conflicts with Ayatana
(the tray library the desktop already uses). `./studio.sh --install` picks
`libayatana-appindicator3-dev` when apt has it.

macOS: accept the Homebrew prompts from `--install`, or `brew install node rust ffmpeg mpv` and `brew install --cask vlc`.

Tuner ports are **8080–8083**. UDP **65001** only if Settings → Advertise tuners is on.
GNOME tray needs an AppIndicator extension; XFCE uses Status Tray.

### Linux `.deb` and AppImage

GitHub Actions on a `v2.*` tag builds the **`.deb`** and the **AppImage**.

```bash
sudo apt install ./epg.monster-studio_*.deb
```

```bash
chmod +x epg.monster-studio_*.AppImage
./epg.monster-studio_*.AppImage
```

The `.deb` depends on `ffmpeg`, `libwebkit2gtk-4.1-0`, `libgtk-3-0`. Data still
goes next to the launch folder (`{app}/data`): the install prefix for the `.deb`,
or the folder that contains the AppImage. Prefer the script if you want everything
in the git checkout.

---

## Data folder

v2 always uses **`{launch folder}/data`** (the repo when you use `studio.ps1` / `studio.sh`,
or the directory that contains the binary). Never `%LocalAppData%`, never
`~/.local/share`, never `~/Library/Application Support`. Those locations are v3.

That folder holds `epg.monster-studio.db`, `auditprocess.db`, `logs/`, `logo/`,
`offline-slates/`, `cache/`, `tool-cache/`.

---

## Update epg.monster studio

Nav footer **Check For Updates** (above Settings):

1. Reads the latest GitHub Release tag for this repo.
2. If it is newer than the running `v2.0.2`, **Open GitHub release** installs
   the matching `.deb` / AppImage (Linux) or you pull and run `studio.ps1` / `studio.sh`.
3. If you are already current, or GitHub is unreachable / has no release yet,
   the status line says so. It does not crash. The repo is **public**.

Splash also checks when Settings → **Check for app updates on splash** is on.

Silent in-app replace + relaunch is **v3** (updater signing key). Testers use
`.\studio.ps1 --restart` / `./studio.sh --restart` or a new `.deb` / AppImage.

---

## Run from source

Same as [Install (v2 testers)](#install-v2-testers): `.\studio.ps1` / `./studio.sh`.
Equivalent: `npm run build` then `EPG_MONSTER_HOME=$(pwd) cargo run --features custom-protocol --manifest-path src-tauri/Cargo.toml`. On Windows add `--target x86_64-pc-windows-gnu`. That is the Rust `.exe` / binary with the UI in `dist/` — not a website, not `tauri dev`.

Windows GNU rustc (no MSVC `link.exe`):

```powershell
cargo +stable-x86_64-pc-windows-gnu test -p studio-core
```

Default:

```bash
cd src-tauri
cargo test -p studio-core
```

Linux `.deb` and AppImage locally:

```bash
export CARGO_TARGET_DIR="$(pwd)/src-tauri/target"
npx tauri build --bundles deb,appimage
```

Artifacts land in `src-tauri/target/release/bundle/` (`epg.monster-studio_2.0.2_amd64.deb` and `.AppImage`). GitHub Release on a `v2.*` tag attaches those files. `./studio.sh --install` is the two-pane tester flow (fixed step table on top, log below), not the package.

Windows NSIS and signed dmg are **[v3](docs/V3.md)**.

---

## GitHub Actions

- `.github/workflows/ci.yml` — `cargo test -p studio-core` on Windows, Ubuntu, macOS (`master`).
- `.github/workflows/release.yml` — on tag `v2.*` (or **Run workflow**), builds
  the Linux **`.deb`** and **AppImage** and attaches them to the GitHub Release.
  NSIS / dmg are v3. Tag `v2.0.2` is the source release (Check For Updates);
  Linux packages attach on the next `v2.*` tag after this workflow is on GitHub.

Create a release:

```bash
git tag v2.0.3
git push origin v2.0.3
```

LAN / Advertise trust model: [docs/LAN.md](docs/LAN.md).

---

## What the studio does

| Section | What it does |
|---------|----------------|
| **Add Sources** | Load file or URL playlists. Search by name and tvg-id. Add a row or a hidden backup. |
| **Playlist Editor** | Curated channels, visible stream + backups, export. |
| **EPG Audit** | epg.monster XMLTV catalog, match tvg-ids. |
| **Logo Audit** | Missing/broken logos. Save a local PNG pack. |
| **Stream Audit** | Serial ffmpeg/ffprobe probes. Auto-swap. Weekly groups. |
| **Managed Output** | Export, tuner lineup, upload channels.json (ids only — never stream URLs). |
| **TV Tuner** | Plex / Jellyfin / Emby / IPTV. Ports 8080–8083. |
| **Check For Updates** | GitHub Releases latest vs this 2026 edition build. |
| **Settings** | Players, ffmpeg, members key, remux, weekly audit, splash update check. |

