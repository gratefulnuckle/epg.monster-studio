# install-scripts Specification

## Purpose

`studio.ps1` (Windows) and `studio.sh` (Linux / macOS) are the v3 launchers. They
MUST install, record, and uninstall toolchain and media tools without guessing.
This spec is the source of truth for those scripts. Server HTTP behavior lives in
`openspec/specs/server-host/`. Operator steps: `INSTALL.md`.

Data (`./data`) is never deleted. `.studio-install.json` `flavor` is `desktop` or
`server` and SHALL gate server-only flags.

## Requirements

### Requirement: Flags

The launchers SHALL support `--install`, `--install desktop`, `--install server`,
`--makepass`, `--makekey`, `--shortcuts`, `--uninstall`, `--start`,
`--start headless`, `--stop`, `--restart`, `--help`.
No args means install desktop then start. `--install` with no flavor means desktop.

#### Scenario: Help lists flags
- GIVEN the repo root
- WHEN the operator runs `--help`
- THEN those flags are listed
- AND `--install desktop` and `--install server` are listed
- AND the operator is told `./data` is never deleted

#### Scenario: Server flavor skips the desktop shell
- GIVEN the repo root
- WHEN the operator runs `--install server`
- THEN Node, Rust, and ffmpeg are offered
- AND the desktop WebView / Tauri `.exe` is not built
- AND `studio-server` is built
- AND `.studio-install.json` `flavor` is `server`

#### Scenario: Server start is the browser UI
- GIVEN a server install
- WHEN the operator runs `--start`
- THEN `studio-server` binds `0.0.0.0:1420` and the UI is opened at `http://127.0.0.1:1420`
- AND no desktop window is opened

#### Scenario: Headless server is API only
- GIVEN a server install
- WHEN the operator runs `--start headless`
- THEN `studio-server` binds `0.0.0.0:1420` without serving the web UI
- AND a desktop API key is created if none exist and printed once
- AND no desktop window is opened

#### Scenario: Make a desktop API key
- GIVEN a server install
- WHEN the operator runs `--makekey`
- THEN a key starting with `epgs_` is printed once
- AND the hash is stored under `{data}/api-keys.json`

#### Scenario: Desktop install rejects server-only flags
- GIVEN a desktop install (`.studio-install.json` `flavor` is `desktop` or missing)
- WHEN the operator runs `--makekey`
- THEN the launcher exits without creating a key
- AND the message is `Desktop version is installed. --makekey is not a feature.`
- AND `--makepass` and `--start headless` fail the same way with their flag names

### Requirement: Per-OS package managers

| OS | Node, Rust | ffmpeg / ffprobe / mpv / VLC |
|----|------------|------------------------------|
| Windows | **winget only** (`OpenJS.NodeJS.LTS`, `Rustlang.Rustup`) | **Scoop first**, then winget (`ffmpeg`, `mpv` extras, `vlc` extras / `Gyan.FFmpeg`, `shinchiro.mpv`, `VideoLAN.VLC`) |
| Linux | distro packages or NodeSource 22 / rustup | `apt-get` / `dnf` / `pacman` (sudo, after a prompt) |
| macOS | Homebrew (`node`, `rust`) or rustup | Homebrew (`ffmpeg`, `mpv`, `vlc` cask) |

Windows MUST NOT offer Scoop for Node or Rust. Scoop MAY be installed only when a
media tool is missing.

#### Scenario: Windows Rust
- GIVEN cargo is missing on Windows
- WHEN `--install` offers Rust
- THEN the prompt is winget `Rustlang.Rustup`
- AND Scoop rustup is not offered

#### Scenario: Windows media
- GIVEN ffmpeg is missing on Windows
- WHEN `--install` offers ffmpeg
- THEN Scoop is offered first if Scoop is present or the operator agrees to install Scoop
- AND winget is the fallback

### Requirement: Install state JSON

`--install` SHALL write gitignored `{repo}/.studio-install.json` after tools are
resolved (installed **or** already present). Each tool record SHALL include:

| Field | Meaning |
|-------|---------|
| `how` | `winget` \| `scoop` \| `rustup` \| `apt` \| `dnf` \| `pacman` \| `brew` \| `brew-cask` \| `nodesource` \| `system` |
| `path` | Absolute path to the real binary (not a tiny shim if a real exe exists) |
| `cmd` | PATH command name (`node`, `cargo`, `ffmpeg`, `mpv`, `vlc`) |
| `scoopName` | Scoop app name when `how` is scoop (`nodejs` vs `nodejs-lts`, `ffmpeg`, `mpv`, `vlc`) |
| `wingetId` | winget package id when known |
| `ffprobe` | Absolute ffprobe path (ffmpeg record only) |

Top-level SHALL include `written` (local timestamp), `folder` (repo path), and
`flavor` (`desktop` | `server`). Missing `flavor` SHALL be treated as `desktop`.

Legacy files that store `"node": "scoop"` (string) MUST still parse as `{ how: "scoop" }`.

#### Scenario: Scoop Node is recorded
- GIVEN Node is `...\scoop\apps\nodejs\current\node.exe`
- WHEN `--install` finishes
- THEN `.studio-install.json` `tools.node.how` is `scoop`
- AND `tools.node.scoopName` is `nodejs`
- AND `tools.node.path` is that exe

#### Scenario: Pre-existing winget ffmpeg
- GIVEN ffmpeg is already on the machine from winget
- WHEN `--install` finds it and does not reinstall
- THEN the JSON still records `how`, `path`, and `wingetId` `Gyan.FFmpeg`

### Requirement: Uninstall uses the JSON first

`--uninstall` SHALL prompt **[y/N]** for Node, Rust, ffmpeg, mpv, and VLC when
the JSON has a record **or** the real binary is still present. Confirming
remove-app SHALL delete `epg-monster-studio` **and** `studio-server` launchables
when present, plus shortcuts and the pid file. `./data` (including
`web-auth.json` and `api-keys.json`) is never deleted.

Uninstall SHALL use recorded `how` / `scoopName` / `wingetId` / `path` in that
order, then fall back to the other manager. It MUST NOT treat “Scoop package
not found” as success while a winget/rustup copy remains.

Rust on Windows: try winget `Rustlang.Rustup`, then `rustup self uninstall`,
then Scoop `rustup`. Errors go to the action log; the row is `failed` if
`cargo` or `rustup` is still present.

#### Scenario: Uninstall Scoop Node
- GIVEN `tools.node.scoopName` is `nodejs`
- WHEN the operator confirms Uninstall Node.js
- THEN the script runs `scoop uninstall nodejs` (not only `nodejs-lts`)

#### Scenario: Uninstall winget ffmpeg recorded as missing how
- GIVEN ffmpeg was on the machine before `--install` and `how` is `system` or empty
- WHEN the operator confirms Uninstall ffmpeg
- THEN winget **and** Scoop are both tried until the real binaries are gone

### Requirement: Action logs

Each flag SHALL write `{repo}/<flag>.log` (`install.log`, `uninstall.log`,
`start.log`, `stop.log`, `shortcuts.log`, `makepass.log`, `makekey.log`).
Package-manager stdout/stderr MUST be appended. Native `info:` / rustup sync
MUST NOT abort the script.

#### Scenario: cargo stderr
- GIVEN `$ErrorActionPreference` is Stop
- WHEN `cargo --version` prints `info: syncing channel updates...` to stderr
- THEN install continues
- AND that line is in `install.log` and the bottom log pane

### Requirement: UI

`--install` / `--uninstall` on Windows, Linux, and macOS SHALL use two panes:
static top (header box **epg.monster studio**, edition line, a pre-seeded step
table, Y/n, `CTRL+Q to QUIT`) and a bottom stdout pane separated by a line.
Header box is double-line `╔═╗` / `╚═╝`, one color, sized to the title text.
Step rows are a fixed grid (tag, 12-char name, 16-char state, clipped detail).
Every painted line is one terminal row: no wrap, and log output MUST NOT scroll
the header off the screen.

`CTRL+Q` quits. `./data` is never deleted.

#### Scenario: Linux install grid is fixed
- GIVEN a Linux `--install`
- WHEN the first paint runs
- THEN the top pane already lists Node.js, Rust, cc, WebKitGTK, ffmpeg, ffprobe,
  mpv, VLC, npm, data, UI build, cargo, and launchable
- AND later tools update those rows instead of inserting new ones

### Requirement: Linux and macOS

`studio.sh --install` SHALL prompt before sudo (`apt-get` / `dnf` / `pacman`) or
Homebrew. Missing Homebrew MAY be installed after a prompt. The same
`.studio-install.json` shape SHALL be written so uninstall can remove the
recorded packages.

#### Scenario: Linux ffmpeg
- GIVEN Ubuntu and ffmpeg missing
- WHEN `--install` offers ffmpeg
- THEN the prompt names `apt-get` and sudo
- AND success records `how: apt` and `path` of `ffmpeg`
