# Delta for settings

## Purpose

Persist every `AppSettings` field with the same tiles, labels, and defaults as WinUI `SettingsPage.xaml`.

## ADDED Requirements

### Requirement: Players tile
The system SHALL show tile **Players**, hint `External player used from Playlist Editor and Stream.`, **Default player** combo `mpv` | `VLC` (default mpv), **mpv.exe path**, **vlc.exe path**.

### Requirement: Stream Audit tile
The system SHALL show tile **Stream Audit**, hint about ffmpeg/ffprobe, paths for ffmpeg.exe and ffprobe.exe, **Delay between probes (ms)** default 6000 (0–120000), **Probe timeout (ms)** default 15000 (1000–120000), checkbox `Auto-swap visible stream to working backup on fail` (default on), checkbox `Pause auto-audit while external player is active` (default on).

### Requirement: Guide tile
The system SHALL show tile **Guide**, hint `XMLTV catalog. Built from tvg-ids in this file.`, **Default User-Agent for URL sources** default `epg.monster-studio/v1.0-beta`, multiline **XMLTV guide URL (epg.monster)** default `https://epg.monster/epg.xml`.

### Requirement: my.epg.monster tile
The system SHALL show email, password-style **Access key (epgm_…)**, **API base** placeholder `https://epg.monster`, **Test key**, **Upload channels.json**, plus status / feed URL / last publish lines. The key MUST never be logged.

#### Scenario: Test key
- GIVEN a key starting with `epgm_`
- WHEN Test key is clicked
- THEN `GET /api/member/v1/ping` runs
- AND feed URL / limits are stored on success
- AND 401/404 text is shown on failure without printing the key

### Requirement: TV Tuner tiles
The system SHALL show a **TV Tuner** section with hint that IPTV is on for new installs, others off, ports 8080–8083, Start on the TV Tuner panel; four nested cards Plex, Jellyfin, Emby, `IPTV (TiviMate / Smarters)` each with enable, friendly name, port, tuner count 1–16, Allow LAN, and URL help. Jellyfin has `Downspiral — one playlist + guide per group (switch lists without changing Jellyfin profiles)`. IPTV has `Remux IPTV playlist through Studio (MPEG-TS)` and combo **Tuner EPG for IPTV players**: `Local Studio guide (/guide.xml)` | `my.epg.monster curated feed`. Global checkbox `Advertise tuners on the network (HDHomeRun UDP 65001 + SSDP). Turn on Allow LAN if Plex is another PC.` (default on).

#### Scenario: Member EPG disabled without feed
- GIVEN no `MemberFeedUrl` / gz
- WHEN the IPTV EPG combo is shown
- THEN `my.epg.monster curated feed` is disabled with text telling the operator to upload from Managed Output first

### Requirement: Remux tile
The system SHALL show **Engine** `ffmpeg` | `VLC` (default ffmpeg), **ffmpeg profile** `Plex MPEG2 + AC3 (recommended)` (`mpeg2_ac3`) | `Threadfin copy (H264 + AAC stereo)` (`copy_aac`), **Buffer before send (KB)** 512–16384.

### Requirement: Logos tile
The system SHALL show **Logo save directory** placeholder `%LocalAppData%\epg.monster-studio\logo`, `Host the logos folder on the tuner`, `Use local logos in tuner playlists and EPG`.

### Requirement: Weekly Stream Audit tile
The system SHALL show Monday–Sunday group text boxes (Sunday spanning two columns), checkbox `Remind me when today's groups have not run (does not start a probe)`, and `Fail fully black screens (ffmpeg blackdetect)`.

### Requirement: Screen matches tile
The system SHALL list slate stills and buttons `Add screen…`, `Remove selected`, `Open folder` for `%LocalAppData%\epg.monster-studio\offline-slates\`.

### Requirement: Diagnostics tile
The system SHALL show `Open logs folder`, `Open crash reports`, the log path, and **Optional Python path** placeholder `python.exe`.

### Requirement: Detect and Save
The system SHALL **Detect bundled tools** (fill mpv/ffmpeg/ffprobe from `tools/` next to the exe) and **Save** persist all tiles to the same settings keys the C# app uses.

#### Scenario: Round-trip with C# DB
- GIVEN a settings blob written by the C# app
- WHEN this remake loads Settings
- THEN every field shows the stored value (including tuner device ids)
