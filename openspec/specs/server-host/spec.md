# server-host Specification

## Purpose

v3 server flavor: `studio-server` hosts the same UI over HTTP. Desktop can stay
local or Connect to that host with an API key. Headless serves the API only.

## Requirements

### Requirement: Same app, two deliveries
`--install desktop` SHALL build the Tauri `.exe` / native binary. `--install server`
SHALL build `studio-server` and the web `dist/` (no desktop window). Both share
`{launch}/data` and the same invoke commands for playlist, EPG, audit, output,
and tuner.

#### Scenario: Server install records flavor
- GIVEN the repo root
- WHEN the operator runs `--install server`
- THEN `.studio-install.json` `flavor` is `server`
- AND `studio-server` (`.exe` on Windows) is copied next to the repo
- AND no requirement to build `epg-monster-studio.exe`

### Requirement: Web login
The web host SHALL require an admin login. `--makepass` SHALL write a temporary
admin password (hashed) under `{data}/web-auth.json` with `mustChange` true.
Username is `admin`. The first successful sign-in SHALL require a new password
before any other API use except `/api/password` and `/api/logout`.

#### Scenario: Make a temporary password
- GIVEN a server install
- WHEN the operator runs `--makepass` or `--makepass <password>`
- THEN a password is printed once (12 unambiguous chars if generated)
- AND the next web login must set a new password

#### Scenario: API locked without a session
- GIVEN the web host is running
- WHEN `/api/invoke` is called without a session cookie or API key
- THEN the response is 401

### Requirement: Desktop API keys
`--makekey` (server install only) SHALL create a key starting with `epgs_`, print
it once, and store a salted SHA-256 hash in `{data}/api-keys.json`. The web
Settings **Account** tab SHALL offer **Generate API key**, a list with prefix and
created time, and **Revoke**. Desktop Connect SHALL send `Authorization: Bearer`.

#### Scenario: Make a desktop API key
- GIVEN a server install
- WHEN the operator runs `--makekey` or Account → Generate API key
- THEN a key starting with `epgs_` is shown once
- AND later list/revoke views never show the secret

#### Scenario: Desktop install rejects server-only flags
- GIVEN a desktop install (`flavor` is `desktop` or missing)
- WHEN the operator runs `--makekey`
- THEN the launcher exits without creating a key
- AND the message is `Desktop version is installed. --makekey is not a feature.`
- AND `--makepass` and `--start headless` fail the same way with their flag names

### Requirement: Headless start
`--start headless` on a server install SHALL bind `0.0.0.0:1420` without serving
`dist/`. It SHALL NOT print a browser URL as the way to edit. If no API keys
exist, it SHALL create one and print it once. Operators edit from a desktop
Connect.

#### Scenario: Headless has no web UI
- GIVEN a server install
- WHEN the operator runs `--start headless`
- THEN GET `/` is not the studio UI
- AND `/api/health` still answers
- AND no desktop window is opened

### Requirement: Desktop is a client
When desktop Settings → This computer has a server URL and API key and Connect
succeeds, playlist and studio settings SHALL load from that host. Player paths
and Play (mpv/VLC) SHALL stay on this computer. Tuner and stream audit SHALL run
on the host. This is not two-way SQLite sync.

#### Scenario: Connect with API key
- GIVEN a running `studio-server` and a key from `--makekey`
- WHEN the operator pastes the URL and key and clicks Connect
- THEN invoke for lineup data goes to that host with the Bearer token
- AND Play still uses this computer’s mpv/VLC

### Requirement: Bind vs browser URL
`studio-server` SHALL bind `0.0.0.0:1420` (all interfaces). Operator-facing URLs
SHALL be `http://127.0.0.1:1420` (or the LAN IP). The launcher MUST NOT tell the
operator to open `http://0.0.0.0:1420`.

### Requirement: Server does not launch players
`studio-server` SHALL only serve HTTP (UI, API, tuner). Play in the web UI SHALL
open in the browser. `play_url` on the host MUST NOT spawn mpv, VLC, or libvlc.
Desktop Play (local or Connect) still uses this computer’s mpv/VLC.

#### Scenario: Web Play stays in the browser
- GIVEN a server install and the operator signed in at `http://127.0.0.1:1420`
- WHEN they click Play on a channel
- THEN a player overlay opens in that browser using mpegts.js (MPEG-TS) or hls.js (HLS)
- AND the media is fetched same-origin via `/api/stream` (so MSE is not blocked by CORS)
- AND no mpv/VLC process is started on the server host
