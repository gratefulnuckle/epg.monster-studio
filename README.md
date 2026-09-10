<p align="center">
  <img src="https://i.imgur.com/TY3OgHP.png" alt="epg.monster">
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

**epg.monster studio** is a desktop (and optional server) app for curating IPTV
playlists: load M3U/M3U8 sources, edit a managed list with hidden backups, match
EPG ids from epg.monster, probe streams with ffmpeg, and present the list to
Plex / Jellyfin / Emby / TiviMate as a local HDHomeRun-style tuner.

This tree is **v3**: **Tauri v2 + Rust + TypeScript**, on **Windows, Linux, and
macOS**. Window title is always **epg.monster studio**. Edition **2026**,
semver **3.0.0**. Desktop window, or `studio-server` in a browser / headless API.
A desktop can **Connect** to a remote studio with an `epgs_` API key.

This is an operator tool. Use it only with sources you have the right to use.

---

## Install (v3)

**Full walkthrough:** [INSTALL.md](INSTALL.md) (flavors, flags, first run, uninstall).

Install is **`studio.ps1` / `studio.sh`** ([INSTALL.md](INSTALL.md)). There is no
Windows setup.exe and no signed macOS `.dmg`. Clone the repo; data stays in
`./data`. Linux **`.deb` / AppImage** may attach on a version tag; they are
optional, not the Windows/macOS path.

Needs **Rust** (stable) and **Node 22+**.

**Windows (PowerShell)**

```powershell
.\studio.ps1 --install desktop   # windowed app
.\studio.ps1 --install server    # browser UI / headless API (no desktop window)
.\studio.ps1 --start             # desktop: .exe · server: http://127.0.0.1:1420
.\studio.ps1 --start headless    # server only: API, no web UI
.\studio.ps1 --makepass          # server: temp admin password (must change at login)
.\studio.ps1 --makekey           # server: desktop API key (shown once)
.\studio.ps1 --stop
.\studio.ps1 --restart
.\studio.ps1 --shortcuts         # Desktop + Start Menu (desktop binary)
.\studio.ps1 --uninstall         # keeps .\data
```

**Linux / macOS**

```bash
chmod +x studio.sh
./studio.sh --install desktop
./studio.sh --install server
./studio.sh --start
./studio.sh --start headless
./studio.sh --makepass
./studio.sh --makekey
./studio.sh --stop
./studio.sh --restart
./studio.sh --shortcuts
./studio.sh --uninstall          # keeps ./data
```

No args (`.\studio.ps1` / `./studio.sh`) is **install desktop then start**.

The script sets `EPG_MONSTER_HOME` to the repo so SQLite, logs, cache, web login,
and API keys are `./data`. ffmpeg is required; mpv and VLC are optional Play
engines. **`--makekey` / `--makepass` / `--start headless` refuse a desktop
install.** Open the web UI at **http://127.0.0.1:1420**, never `http://0.0.0.0:1420`.

**`--install` package managers**

| OS | How missing tools are offered |
|----|-------------------------------|
| Windows | Node/Rust: winget. ffmpeg/mpv/VLC: Scoop first, then winget |
| Linux | `apt-get`, `dnf`, or `pacman` — each step asks, then `sudo` |
| macOS | Homebrew (installs brew if you agree). VLC is a brew cask |

**`--uninstall`** stops the app and, with prompts, can remove shortcuts and
launchables. **`./data` is never deleted** (including `web-auth.json` and
`api-keys.json`).

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

GitHub Actions on a `v2.*` or `v3.*` tag builds the **`.deb`** and the **AppImage**.

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

Installs always use **`{launch folder}/data`** (the repo when you use
`studio.ps1` / `studio.sh`, or the directory that contains the binary). Never
`%LocalAppData%`, never `~/.local/share`, never `~/Library/Application Support`.

That folder holds `epg.monster-studio.db`, `auditprocess.db`, `logs/`, `logo/`,
`offline-slates/`, `cache/`, `tool-cache/`, and on a server host `web-auth.json`
and `api-keys.json`.

---

## Update epg.monster studio

Nav footer **Check For Updates** (above Settings):

1. Reads the latest GitHub Release tag for this repo.
2. If it is newer than the running `v3.0.0`, **Open GitHub release** installs
   the matching `.deb` / AppImage (Linux) or you pull and run `studio.ps1` / `studio.sh`.
3. If you are already current, or GitHub is unreachable / has no release yet,
   the status line says so. It does not crash. The repo is **public**.

Splash also checks when Settings → **Check for app updates on splash** is on.

**Install and relaunch** (Check For Updates) replaces the binary from a GitHub
Release when that release has a matching file. Otherwise use
`.\studio.ps1 --restart` / `./studio.sh --restart`.

---

## Run from source

Same as [Install (v3)](#install-v3): `.\studio.ps1` / `./studio.sh`. See [INSTALL.md](INSTALL.md).
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

Linux `.deb` and AppImage locally (optional):

```bash
export CARGO_TARGET_DIR="$(pwd)/src-tauri/target"
npx tauri build --bundles deb,appimage
```


---

## GitHub Actions

- `.github/workflows/ci.yml` — `cargo test -p studio-core` on Windows, Ubuntu, macOS (`master`).
- `.github/workflows/release.yml` — on tag `v3.*` (or **Run workflow**), builds
  the Linux **`.deb`** and **AppImage** and Windows portable `epg-monster-studio.exe` /
  `studio-server.exe` / `studio-web-dist.zip` for **Install and relaunch**.

Create a release:

```bash
git tag v3.0.0
git push origin v3.0.0
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
| **IPTV Player** | G-houl embed (desktop). |
| **Check For Updates** | GitHub Releases latest vs this 2026 edition build. |
| **Settings** | This computer / This host, Studio, Account (web login, API keys), Advanced. Desktop can Connect to a remote studio. |

