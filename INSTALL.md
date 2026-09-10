# Install and run

**epg.monster studio v3** is installed from this repo with `studio.ps1` (Windows)
or `studio.sh` (Linux / macOS). There is no Windows setup.exe and no signed
macOS `.dmg`. The scripts **are** the installer.

Data always stays in **`./data` next to the repo** (or the folder that contains
the binary). Uninstall never deletes `./data`.

Full flag semantics: `openspec/specs/install-scripts/`. Server HTTP:
`openspec/specs/server-host/`.

---

## Pick a flavor

| Flavor | What you get | When to use it |
|--------|----------------|----------------|
| **desktop** (default) | Tauri window, `epg-monster-studio.exe` / `epg-monster-studio` | This machine is where you edit and play |
| **server** | `studio-server` HTTP only; you work in a **browser** | Headless box, NAS, or a host other desktops Connect to. Play happens in the browser — the server does not spawn mpv/VLC. |

`.studio-install.json` records `flavor`. Missing flavor is treated as **desktop**.
`--makekey`, `--makepass`, and `--start headless` only work on a **server**
install. On desktop they exit:

```text
Desktop version is installed. --makekey is not a feature.
```

Do not run desktop and server against the **same** `./data` at the same time
(same SQLite files). A desktop on PC A Connecting to a server on PC B is the
intended split.

---

## Flags

| Flag | What it does |
|------|----------------|
| *(no args)* | `--install desktop` then `--start` |
| `--install` | Same as `--install desktop` |
| `--install desktop` | Node, Rust, ffmpeg; optional mpv/VLC; G-houl tools on desktop; build the windowed app |
| `--install server` | Node, Rust, ffmpeg; optional mpv/VLC; build `studio-server` + web `dist/`; **no** desktop `.exe` |
| `--start` | Desktop: run the `.exe`. Server: bind `0.0.0.0:1420` and serve the UI. Open **http://127.0.0.1:1420** (not `0.0.0.0`) |
| `--start headless` | **Server only.** API on `:1420`, no web UI. Connect from desktop with an API key |
| `--stop` | Stop the desktop app and `studio-server` |
| `--restart` | `--stop` then `--start`. `--restart headless` is valid on server |
| `--makepass` `[password]` | **Server only.** Temporary admin password for the browser login (must change at first sign-in) |
| `--makekey` `[name]` | **Server only.** Desktop API key (`epgs_…`), shown once |
| `--shortcuts` | Desktop: windowed app icon. Server: URL shortcut to `http://127.0.0.1:1420` |
| `--uninstall` | Stop, optional remove binaries/shortcuts/tools. **`./data` is kept** |
| `--help` | Flag list |

Each action writes `{flag}.log` in the repo (`install.log`, `start.log`,
`makekey.log`, …).

---

## Windows

Needs PowerShell. GNU Rust (`stable-x86_64-pc-windows-gnu`) is used so you do
not need MSVC `link.exe`.

```powershell
# Desktop (window)
.\studio.ps1 --install desktop
.\studio.ps1 --start

# Or one shot (install desktop + start)
.\studio.ps1

# Server (browser UI)
.\studio.ps1 --install server
.\studio.ps1 --makepass
.\studio.ps1 --start
# then open http://127.0.0.1:1420  (username admin, temp password from --makepass)

# Server (no browser — desktops Connect)
.\studio.ps1 --install server
.\studio.ps1 --makekey
.\studio.ps1 --start headless
```

**What `--install desktop` offers**

1. Node 22+ via **winget** `OpenJS.NodeJS.LTS` if missing  
2. Rust via **winget** `Rustlang.Rustup` if missing  
3. gcc check (MinGW). If missing, optional WinLibs page  
4. ffmpeg/ffprobe — **Scoop first**, then winget `Gyan.FFmpeg`  
5. mpv / VLC — optional Play engines (Scoop extras / winget)  
6. libmpv, GStreamer prefix, `tools\ghoul\ghoul-gst.exe` (G-houl)  
7. `npm install`, `npm run build`, `cargo build` release, copy
   `epg-monster-studio.exe` next to the repo  
8. Write `.studio-install.json` with `flavor: desktop`

**What `--install server` offers**

Same Node / Rust / ffmpeg (mpv/VLC still offered if missing). Skips gcc
prompt, GStreamer, and the desktop `.exe`. Builds web `dist/` with
`STUDIO_WEB=1` and copies `studio-server.exe`. Sets `flavor: server`.

CTRL+Q quits the installer UI.

---

## Linux / macOS

```bash
chmod +x studio.sh
./studio.sh --install desktop    # or: --install server
./studio.sh --start
```

Package managers (each step asks first):

| OS | Node / Rust | ffmpeg / mpv / VLC |
|----|-------------|---------------------|
| Debian/Ubuntu | NodeSource 22 / rustup or distro | `apt-get` + sudo |
| Fedora | distro or rustup | `dnf` + sudo |
| Arch | distro or rustup | `pacman` + sudo |
| macOS | Homebrew `node` `rust`, or rustup | Homebrew; VLC is a cask |

Linux desktop also needs GTK/WebKit build libs (`libwebkit2gtk-4.1-dev`, …).
`--install` prompts for those.

`./studio.sh` with no args is install desktop + start, same as Windows.

---

## First run — server

1. `--install server`  
2. `--makepass` — print a temp password (or `--makepass password` to set it)  
3. `--start`  
4. Browser: **http://127.0.0.1:1420** — sign in as `admin`, set a real password  
5. Settings → **Account** → **Generate API key** (or `--makekey`) for desktops  

Headless host (no browser on that machine):

1. `--install server`  
2. `--makekey` — copy the `epgs_…` line  
3. `--start headless`  
4. On another PC with a **desktop** install: Settings → This computer → URL
   `http://HOST:1420` + the key → **Connect**

If headless starts with **no** keys yet, `studio-server` creates one named
`headless` and prints it once on stderr.

---

## First run — desktop

1. `--install desktop`  
2. `--start` (or `--shortcuts` then the icon)  
3. Play needs mpv or VLC (install or set paths in Settings → This computer)  
4. Stream Audit needs ffmpeg + ffprobe  

To attach this desktop to a remote studio: paste the server URL and `epgs_`
key, **Connect**. Lineup/settings live on the host; Play still uses this PC.

---

## Uninstall

```powershell
.\studio.ps1 --uninstall
```

```bash
./studio.sh --uninstall
```

Prompts (default **N** for tools):

- Remove studio binaries (desktop + studio-server) and shortcuts? (stops the app first)  
- Uninstall Node / Rust / ffmpeg / mpv / VLC?  
- Desktop: staged libmpv, G-houl sidecar, GStreamer prefix  

**Never deleted:** `./data` (databases, logs, `web-auth.json`, `api-keys.json`,
cache). Remove that folder yourself if you want a clean slate.

Confirming remove-app deletes **both** the desktop launchable and `studio-server`,
plus shortcuts. Flavor in `.studio-install.json` is cleared.

---

## After install — files next to the repo

| File | Flavor |
|------|--------|
| `epg-monster-studio.exe` / `epg-monster-studio` | desktop |
| `studio-server.exe` / `studio-server` | server |
| `dist/` | UI (desktop embeds it; server serves it unless headless) |
| `.studio-install.json` | which tools were used + `flavor` |
| `.studio-dev.pid` | last start pid |
| `./data/` | SQLite, logs, auth, keys |

Environment the launchers set:

- `EPG_MONSTER_HOME` = repo root  
- `STUDIO_BIND` = `0.0.0.0:1420` (server)  
- `STUDIO_UI_DIR` = `dist` (server, not headless)  
- `STUDIO_HEADLESS=1` (headless start)  

---

## Switching flavors

Installing one flavor **removes** the other launchable in this folder so you
cannot double-click the leftover. `--start` follows **`flavor`**. If the other
flavor is already running, `--start` stops it and starts the requested one
(including switching web UI ↔ headless).
