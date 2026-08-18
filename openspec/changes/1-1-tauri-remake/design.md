# Design: 1:1 Tauri remake

## Technical approach

Port `EpgMonsterStudio.Core` into a Rust crate (`studio-core`) and drive a TypeScript UI through Tauri v2 commands and events. The UI is a pixel-and-copy remake of the WinUI pages, not a new SPA information architecture.

```
TypeScript (pages, dialogs, tokens)
        │  invoke / listen
Tauri v2 commands + events
        │
studio-core (Rust)
  parse · sqlite · settings · epg · audit · logos
  player spawn · members http · crash/log
        │
studio-tuner (Rust)
  TCP :8080–8083 · HDHomeRun docs · remux child · SSDP/65001
        │
Child processes: mpv, vlc, ffmpeg, ffprobe
```

The C# app is the oracle. When implementing a page, open the matching `Pages/*.xaml` + `.cs` and the Core service, then reproduce.

## Architecture decisions

### Decision: Rust owns all domain logic

Parser, SQLite, audit queue, EPG catalog, members API, tuner HTTP, and process spawn live in Rust.

**Why:** matches C# Core vs WinUI split; UI stays thin; tuner must keep running while the window is in the tray.

**Alternative rejected:** TypeScript domain + thin Rust FS/process wrappers — would fork behavior and make DB races likely.

### Decision: Same AppData folder and schema

Path: `%LocalAppData%\epg.monster-studio\`

Files (C# names, keep them):

| File | Role |
|------|------|
| `epg.monster-studio.db` | Main SQLite (WAL) |
| `auditprocess.db` | Stream Audit job + feed persistence |
| `logs/epg.monster-studio-YYYY-MM-DD.log` | Daily log + 5s heartbeat |
| `logs/crashes/` | Crash reports |
| `pending-crash.txt` | Shown on next launch |
| `logo/` | Default Save Logos root |
| `offline-slates/` | Extra slate stills |
| `cache/` | URL source downloads |

First launch: if the new folder is empty and `%LocalAppData%\iptv-studio\` exists, copy it (C# `Bootstrap` behavior).

**Why:** 1:1 includes the operator’s existing workspace. A parallel `epg.monster-studio-tauri` folder would split state.

**Alternative rejected:** new folder + importer — extra UI the C# app does not have.

### Decision: TypeScript UI, Fluent-dark CSS tokens, not a component-library restyle

Use TypeScript (React or vanilla — pick one in implementation and keep it). Style with the exact tokens in `visual-parity`. Virtualize long lists (sources can be 30k+ rows).

Do not use a generic dashboard kit that changes spacing, type, or control chrome.

WinUI Mica / caption-button color hacks are a **platform exception**. Tauri window: dark `#0C0C10`, title `epg.monster studio`, icon `assets/logo.ico`. Search box lives in the title bar on Add Sources, Playlist Editor, and Managed Output only.

### Decision: User-mode TCP tuner (already C# behavior)

C# already left HTTP.sys. Rust uses Tokio TCP (hyper or equivalent) on 8080–8083. Same routes, same JSON/XML/M3U bodies, no provider URLs in HDHomeRun lineup.

### Decision: External play only

`PlayerService` launches mpv (`--force-window=yes --keep-open=yes` + `--http-header-fields`) or VLC (`:http-user-agent`). No libmpv embed.

### Decision: GPL-3.0 + bundled tools

Same license as source. Bundle portable `tools/ffmpeg` and `tools/mpv` next to the app; Settings **Detect bundled tools** fills paths. Keep `THIRD_PARTY_NOTICES.md`.

## Data model

Reproduce `SqliteStore.Initialize` exactly, including `EnsureColumn` migrations so older operator DBs open:

**sources** — id, name, kind (`file`|`url`), location, headers_json, etag, last_modified, cached_path, loaded_at, channel_count

**channel_entries** — id, source_id, group_title, name, tvg_id, tvg_name, tvg_logo, url, attrs_json, line_no

**managed_channels** — id, name, group_title, tvg_id, tvg_logo, notes, sort_order, tvg_shift, in_tuner, tuner_number

**stream_variants** — id, managed_channel_id, url, label, source_entry_id, visibility (`visible`|`hidden_backup`), priority, last_audit_ok, last_audit_at, origin_name, origin_tvg_id

**Invariant:** at most one `visible` variant per managed channel. Lowest `priority` among visibles is the exported URL (top row).

**audit_results** — base columns plus grade, width, height, fps, aspect_ratio, video_codec, audio_codec, job_id, channel_id, channel_name, group_title, tvg_id, error_class

**swap_undo_log**, **epg_catalog** (+ section), **epg_programmes**, **epg_now_playing**, **settings** (key/value), **channel_fts** (fts5: name, group_title, tvg_id, url)

Settings JSON keys match `AppSettings` property names so a C# settings blob deserializes.

## Tauri command map (normative names)

Frontend invokes these (snake_case). Implementation may group modules but MUST expose this behavior.

**Shell:** `get_studio_info`, `show_toast`, `minimize_to_tray`, `open_path`, `report_crash`, `submit_crash`

**Sources:** `add_source_file`, `add_source_url`, `refresh_source`, `remove_source`, `list_sources`, `list_groups`, `list_channels`, `search_sources`

**Managed:** `list_managed`, `save_channel`, `add_from_source`, `add_stream`, `move_stream`, `remove_stream`, `rename_group`, `export_m3u`, `undo_swap`

**EPG/Logo:** `fetch_xmltv`, `epg_suggest`, `epg_apply`, `epg_auto_match`, `now_playing`, `logo_scan`, `logo_save_pack`

**Audit:** `audit_start`, `audit_pause`, `audit_resume`, `audit_cancel`, `audit_snapshot`, `audit_pick`

**Tuner:** `tuner_start`, `tuner_stop`, `tuner_start_all`, `tuner_stop_all`, `tuner_status`, `tuner_self_test`, `tuner_set_max`

**Members:** `members_ping`, `members_publish`

**Settings:** `settings_get`, `settings_save`, `tools_detect`

Events: `audit://progress`, `audit://state`, `tuner://status`, `epg://fetch-progress`, `log://line`, `toast`.

## Tuner routes (must match C#)

On each enabled/started host:

| Path | Body |
|------|------|
| `/` , `/discover.json` | HDHomeRun discover (FriendlyName, DeviceID, LineupURL, TunerCount) |
| `/lineup_status.json` | Scan status |
| `/lineup.json` | Channels → `/auto/v{n}` only (never provider URLs) |
| `/lineup.post` | Accept, no-op or documented C# behavior |
| `/guide.xml` , `/xmltv.xml` | Studio XMLTV from managed + catalog |
| `/tuner.m3u` , `/lineup.m3u` , `/playlist.m3u8` , `/playlist.m3u` | M3U; IPTV remux-off may use visible URLs |
| `/auto/v{n}` | ffmpeg/VLC remux MPEG-TS; live failover to backup |
| `/logos/{tvg-id}.png` | Local logo pack when hosting enabled |
| `/downspiral/index.json` , `/downspiral/{group}.m3u8` , `/downspiral/{group}.xml` | Jellyfin Downspiral only |

Discovery: UDP 65001 + SSDP when Settings **Advertise tuners** is on.

Defaults: Plex 8080 off, Jellyfin 8081 off, Emby 8082 off, IPTV 8083 **on**. Device id = 8 hex from GUID, persisted. Legacy ports 5004–5007 migrate to 8080–8083.

## Audit pacing (normative)

```
while queue not empty and not cancelled:
    if pause_while_playing and player_active: wait
    result ← ffmpeg/ffprobe(timeout)
    if decode ok: compare frame hash to offline-slates (bundled + AppData)
    if still ok and blackdetect enabled: ffmpeg blackdetect
    persist audit_results + grade
    if auto_swap and visible failed and a backup later passes: swap + swap_undo_log
    sleep(delay_ms)   # default 6000
```

Never start a second probe until the previous process has exited. Unclean exit leaves job paused in `auditprocess.db`.

## Members API

Base default `https://epg.monster`.

- `GET /api/member/v1/ping` — Bearer + `X-EPG-Member-Key`, UA `epg.monster-studio/{version}`
- `PUT /api/member/v1/feed/channels` — curated tvg-ids only, never stream URLs; cap 2500 / 3 MiB unless ping `limits` says otherwise
- `GET /api/member/v1/feed/jobs/latest` — poll until `buildStatus=ready`
- `POST /api/member/v1/issues` — crash reporter, redacted

## Search

Add Sources title-bar search: all loaded sources, **minimum 2 characters**, cap **400** hits, FTS/LIKE on name, group, tvg-id, URL. Must not freeze UI on 70k rows (C# bench: CNN* ~3 ms).

## Performance targets (from original spec + audit)

| Metric | Target |
|--------|--------|
| Load 50k-line playlist | Parse + index &lt; 10s; UI responsive |
| Scroll | Virtualized; no full visual tree |
| Search (indexed) | &lt; 100ms after load |
| Tuner lineup | No provider URLs in `/lineup.json` |

## Risks

| Risk | Mitigation |
|------|------------|
| Schema drift | Copy C# DDL + tests; open a real operator DB in CI if available (no URLs in git) |
| Tuner clients picky | Port `TunerClientProbe` assertions (Plex 5, Jellyfin 6, Emby 5, IPTV 4) |
| List virtualization | Virtual lists from day one on sources + editor |
| Slate/hash mismatch | Bundle `offline-slate.png`; same average-hash as C# |
| Accidental redesign | visual-parity tokens + screenshot checklist vs WinUI |

## Platform exceptions (do not fake)

- WinUI `TitleBar` / `MicaBackdrop` / caption-button theme hooks
- Inno Setup installer (later change)
- HTTP.sys / URL ACL copy
- Windows `.exe` tool names on non-Windows (out of this change)

## File layout (planned implementation)

```
epg.monster-studio-tauri/
  openspec/                          ← this change
  assets/logo.ico, logo.png
  src/                               ← TypeScript UI
  src-tauri/
    crates/studio-core/              ← Rust port of Core
    crates/studio-tuner/
    src/lib.rs                       ← Tauri commands
    tauri.conf.json                  ← productName "epg.monster studio"
  tools/ffmpeg, tools/mpv            ← gitignored binaries
```
