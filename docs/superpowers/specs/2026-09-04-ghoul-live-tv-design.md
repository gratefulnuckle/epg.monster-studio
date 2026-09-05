# G-houl Live TV player

Date: 2026-09-04
Status: approved for implementation planning
Edition: v3 (does not ship in v2)

G-houl is the Live TV sidecar for epg.monster studio: a TiviMate-style EPG guide plus a video pane, live channels only. It is based on the IPTV Player / Live TV surface of [ynoTV](https://github.com/tbeezy/ynotv), not the rest of that app.

This document supersedes the parked `ghoul/openspec/specs/ghoul-player` model in which the studio IPTV Player page only launched a detached `ghoul.exe`. Studio now **embeds** the same Live TV UI. Standalone `ghoul.exe` still exists so the player can run without studio, after the operator loads an M3U and XMLTV.

## Goal

Play the curated live lineup with a core EPG grid, inside studio or as `ghoul.exe`. Three playback engines (GStreamer default, mpv IPC, libmpv). User-Agent presets independent of the engine. No VOD, catchup, DVR, sports, Stremio, Add Sources as a lineup, ffmpeg/VLC engines, or ynoTV extras (favorites, 3-column, badges, transparent overlay).

## Constraints (locked)

- Live TV only. Clicking a past programme block still tunes **live**.
- Studio player sees **only the curated Playlist Editor lineup**, never Add Sources lists.
- TypeScript UI like the rest of studio. Not a React port of ynoTV.
- Studio visual tokens (`#0C0C10` chrome, `#16161E` tiles). No ynoTV theme pack.
- Stream URLs and access keys (`epgm_…`) never logged, toasted in full, or written to crash payloads.
- Playlist Editor **Play** (Settings mpv / VLC) is unchanged. G-houl is a separate surface.
- GStreamer remains optional at Windows install (tick / `/NOGHOUL`). That only removes the GStreamer engine, not the IPTV Player page.

## Architecture

One Live TV UI, two hosts, one Rust playback backend.

```
                    ┌─────────────────────────────────────┐
                    │  ghoul-ui (TypeScript)              │
                    │  categories | time grid | video pane│
                    └──────────────┬──────────────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              ▼                    ▼                    ▼
     studio IPTV Player    standalone ghoul.exe    ghoul-guide worker
     (embed in page)       (Tauri window)          (playlist + XMLTV)
              │                    │
              └─────────┬──────────┘
                        ▼
                 ghoul-play trait
            GStreamer | mpv IPC | libmpv
                        │
                        ▼
              video pane (HWND / --wid / embed)
```

### Units

| Unit | Does | Used how | Depends on |
|------|------|----------|------------|
| **ghoul-ui** | Core guide chrome, search, hide-grid, engine/UA dropdowns, keys | Mounted by either host | Snapshot from ghoul-guide; play/stop/zap on ghoul-play |
| **ghoul-guide** | Fetch/inflate/parse M3U + XMLTV on a worker; publish a snapshot | Both hosts | Playlist path/URL + XMLTV path/URL; no GST/mpv thread |
| **ghoul-play** | Start/stop/reload one live URL with UA + extra headers into the video pane | UI calls a single current engine | One of: GStreamer runtime, `mpv.exe`, libmpv |
| **studio host** | IPTV Player page, G-houl Player button, curated M3U + member XMLTV files | Nav **IPTV Player** | Playlist Editor rows; Settings access key for member feed |
| **standalone host** | `ghoul.exe` window, Open playlist / Open XMLTV | Double-click or CLI | Operator-supplied M3U + XMLTV only |

Video is never a `<video>` tag. The engine draws into the video pane. Leaving studio’s IPTV Player nav **stops playback and tears down the overlay** so it cannot punch through Playlist Editor.

## Shells

### Studio embed

Left nav **IPTV Player** stays. The page opens as a status landing (curated count, access key, member feed, which engines are present) plus a **G-houl Player** button (logo + label).

Clicking **G-houl Player** mounts ghoul-ui **inside that page**: top-right video preview, left categories, time grid in the remaining area.

- Empty curated playlist: do not mount. Message: `Load a curated playlist in Playlist Editor first.`
- No playable stream URLs: do not mount. Message: `Curated channels have no playable stream URLs.`
- No engine available: do not mount. Message names what is missing (GStreamer not installed, `mpv.exe` not found, libmpv missing).
- Studio does **not** spawn detached `ghoul.exe` for this flow.
- Engine id, UA preset, custom UA string, and last channel index persist in `{launch}/data/ghoul/player.json` so the next embed restores them. The lineup always comes from the current curated snapshot, not from that file.

Hide-grid (`G`) collapses categories + time grid; the video pane expands to fill the G-houl view. `G` restores the guide. Fullscreen (`F`) is the video pane only.

### Standalone `ghoul.exe`

A small Tauri window running the same ghoul-ui. No studio DB, no Add Sources, no members API.

On start the operator opens:

- Playlist: M3U/M3U8, local file or HTTP(S) URL
- XMLTV: local file or HTTP(S) URL

Playlist is required to play. XMLTV is required for programme blocks; playlist-only still plays with channel names and empty cells.

Last session is stored at `{exe}/data/ghoul-last.json`: playlist path/URL, XMLTV path/URL, engine id, UA preset id (and custom string if Custom), last channel index. Not provider stream URLs from inside the M3U. Next launch restores those fields.

CLI remains valid: `--playlist`, `--xml`, `--ua`, `--header`, `--gst-dir`. `--url` / `--file` are out of this Live TV cut (no single-file player chrome).

## Data

### Studio lineup

G-houl reads **visible curated channels only** (`list_managed`). Add Sources lists are never loaded into the player.

Studio writes `{launch}/data/ghoul/iptv.m3u` (visible-only snapshot). Stream URLs stay on disk, never logged. If a curated row has a User-Agent or extra headers, the snapshot includes them as `#EXTVLCOPT:http-user-agent=` and matching header tags so standalone-style parse and studio parse share one path.

XMLTV is the **member feed** my.epg.monster built from uploaded `channels.json`. Studio downloads it with `Authorization: Bearer` + `X-EPG-Member-Key`. The key is not placed in the URL. Bytes are written to `{launch}/data/ghoul/iptv.xml` and the player is given that path, not the key.

- Member feed available: use it. Do **not** synthesize a temp guide from local `epg.xml` + `channels.json`.
- No key / no feed / fetch fail: still mount with the playlist; programme cells empty; tell the operator to upload `channels.json` so now-playing can appear.

### Standalone lineup

Operator-supplied M3U + XMLTV. `#EXTVLCOPT:http-user-agent=` / `http-user-agent` on an entry is that channel’s UA. Extra VLC-opt headers are extra headers. No members API.

### ghoul-guide worker

Fetch, inflate (gzip), and parse run on a dedicated worker. The UI thread only reads a snapshot. The GStreamer / mpv thread does not wait on HTTP or XML parse.

Snapshot contents:

- Channels: name, `group-title`, `tvg-id`, `tvg-logo`, URL, per-entry UA/headers
- Categories: distinct `group-title` values, plus **All channels** first. Empty group-title → **Other**
- Programmes for playlist `tvg-id`s whose window intersects **now − 8h … now + 16h**
- `now` / `next` per `tvg-id`
- `loading` / `err` flags

`tvg-id` match is case-insensitive and uses the existing alias helper (`tvg_keys`: strip ` (src…)`, map `.us_localsN` → `.us`).

A large XMLTV must not freeze the window. Video may start on the first playlist URL before parse finishes. Footer may show `EPG loading`. When the worker publishes, now-playing and grid cells update without rebuilding the pipeline. Zap during parse starts the new URL immediately; titles fill in when that `tvg-id` is in the snapshot. Worker is cancelled on page teardown / process exit. Input is streamed (not `read_to_string` of unbounded bytes). Playlist fetch cap remains 32 MiB.

## Core guide

TiviMate-style. Same chrome in both hosts.

**When the guide is visible**

- Left: category list (`All channels`, then groups). Selecting a category filters the grid.
- Center/bottom: channel rows with a horizontal time grid. A now-line. Programme blocks for the loaded window. Viewport shows about **3 hours**, horizontally scrollable across the retained window. Times are **24-hour `HH:MM`**.
- Top-right: video preview + live chrome (play/pause, mute, volume, fullscreen, engine dropdown, UA dropdown).
- Search (`S`): case-insensitive substring on channel name. No hits → empty list, playback unchanged.
- Channel column shows `tvg-logo` if present, otherwise a letter tile. No logo-style settings in this cut.

Selecting a channel row or any programme block (past, now, or future) plays that channel **live**.

**Hide grid (`G`)** — categories + time grid hidden; video pane fills the G-houl view. Mouse wheel over the view **zaps**. When the guide is visible, wheel **scrolls** the grid.

**Now/next** — current programme highlighted on the playing row. Video overlay/footer: channel name + now title + `HH:MM–HH:MM` when known; name only if no match (no fake title). Channel up/down shows a ~4s zap overlay: index, name, group, now-playing.

**Keys (not remappable in this cut)**

| Action | Key |
|--------|-----|
| Channel up/down (wrap) | `↑` `↓` |
| Page | `PgUp` `PgDn` |
| Play/pause | `Space` |
| Mute | `M` |
| Fullscreen (video pane) | `F` |
| Toggle guide | `G` |
| Focus search | `S` |
| Back / exit fullscreen | `Esc` |

**Empty / loading**

- EPG still parsing: channels listed, cells `EPG loading`, video may already play.
- No XMLTV: names only.
- `EPG failed` (fetch/parse error): channels listed, cells empty, short footer status, not a modal.

**Out of this cut:** favorites, A–Z jump, transparent overlay, 3-column view, metadata badges, resolution filter, EPG shift, mini media bar, logo tile settings, remappable keys, pop-out, PiP, multiview.

## Engines

One engine at a time. Dropdown in the video chrome. Switch **stops** the current backend and reloads the current URL with the same UA/headers. Never two players drawing at once.

| Id | Default | Mechanism | Available when |
|----|---------|-----------|----------------|
| `gstreamer` | yes | Existing playbin + `VideoOverlay` into the pane | Bundled GStreamer runtime (install tick) |
| `mpv-ipc` | no | Bundled `{app}/tools/mpv/mpv.exe`, `--input-ipc-server` named pipe, `--wid` into the pane | `mpv.exe` exists |
| `libmpv` | no | In-process libmpv embed into the pane | `libmpv` next to bundled mpv (`{app}/tools/mpv/libmpv-2.dll` on Windows, `libmpv.so.2` / `libmpv.dylib` otherwise) |

Missing engines are disabled in the dropdown with a short reason. If `gstreamer` is the saved/default choice but its runtime is missing, the host starts the first available engine in order `gstreamer` → `mpv-ipc` → `libmpv`. Init failure: video pane status (`GStreamer not installed`, `mpv.exe not found`, `libmpv failed to start`); playback stopped.

Engine switch that fails: roll back to the previous engine if it can still start; else GStreamer if present; else stopped.

GStreamer `source-setup` sets `user-agent`, `is-live`, `extra-headers`. mpv IPC and libmpv set `user-agent` and `http-header-fields` (CRLF-joined `Key: Value`).

ffmpeg and VLC are **not** G-houl engines. ffmpeg/ffprobe stay Stream Audit / remux. VLC stays Settings **Play**.

## User-Agents

Independent of engine. Applied to the current stream request.

**Resolution order**

1. Curated row / M3U entry UA (`#EXTVLCOPT:http-user-agent=` or `http-user-agent`) if non-empty.
2. Else the G-houl preset dropdown.

Extra headers on the row (`Referer`, `Authorization`, …) always go out. Changing preset or engine reloads the current channel; it does not rebuild the playlist.

| Preset id | Label | String |
|-----------|-------|--------|
| `tivimate` | TiviMate 4.6.0 | `TiviMate/4.6.0 (Linux; Android 11)` |
| `vlc` | VLC | `VLC/3.0.21 LibVLC/3.0.21` |
| `gse` | GSE Smart IPTV | `GSE SMART IPTV/2.9.9` |
| `smarters` | IPTV Smarters | `IPTVSmartersPlayer` |
| `browser` | Browser | `Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36` |
| `custom` | Custom | operator-typed string |

Fallback default (no row UA): **`tivimate`**. An empty Custom string is treated as `tivimate`, not as a blank `User-Agent`.

Studio embed uses whatever UA/headers the curated snapshot stored for that row; it does not look up Add Sources.

## Errors

- Stream HTTP 401/403/404 or demux failure: pane `Stream failed` plus the code when known; frame cleared; zap still works. Toasts/logs redact the URL.
- XMLTV error: not a modal; `EPG failed` in the footer; channels remain.
- Standalone bad playlist path/URL: open-dialog error; playback does not start.
- Worker cancelled on teardown so a huge XMLTV cannot outlive the view.

## Privacy

- Never log or crash-report stream URLs, `epgm_…` keys, or `Authorization` values. Use existing redact helpers.
- Curated M3U snapshot, member XMLTV, and studio `player.json` live under `{launch}/data/ghoul/` (not git).
- Standalone `ghoul-last.json` stores playlist/XML **paths or source URLs the operator typed**, engine, preset, channel index — not provider stream URLs from inside the M3U.

## Testing

Automated (no live provider URLs):

- UA resolution: row UA wins; else preset; extra headers preserved.
- `tvg-id` match + `tvg_keys` aliases.
- Playlist groups, **All channels**, **Other**, wrap zap.
- Grid snapshot window: programmes outside now−8h / now+16h dropped.
- Engine trait with a mock backend: start/stop/reload; never two live.
- Studio gate: empty curated playlist does not mount; no-URL lineup does not mount.

Manual:

- Studio: G-houl Player mounts embed; hide-grid; leave nav stops video and overlay.
- Standalone: open M3U + XMLTV; restore last session.
- Each engine, when its binary/runtime is present, plays one local or LAN test stream.
- Preset change reloads the current channel with the new `User-Agent`.

## Relationship to parked G-houl

Keep: worker-thread XMLTV, `tvg_keys`, optional GStreamer install, visible-only M3U snapshot, member-feed XMLTV with key headers, no subtitle catalog, privacy redaction.

Replace: detached `ghoul.exe` as the studio IPTV Player; mpegts.js webview player; local EPG synthesis from `epg.xml` + `channels.json`; `--file` file-player chrome as part of this Live TV surface.

v2 studio does not show IPTV Player or G-houl until this v3 work is implemented.
