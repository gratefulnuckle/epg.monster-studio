# tv-tuner Specification

## Purpose

Local HDHomeRun-style hosts for Plex, Jellyfin, Emby, plus an IPTV M3U+XMLTV host for TiviMate.

## Requirements

### Requirement: Page chrome
The system SHALL title the page `TV Tuner` and show the shipped subtitle about Start after Settings enable, ports 8080–8083, and lineup numbers living in Managed Output.

#### Scenario: Toolbar
- GIVEN TV Tuner is open
- WHEN the toolbar is visible
- THEN it contains `Start all enabled` (accent), `Stop all`, `Log`, `Graphs`, `Self-test`

### Requirement: Four cards always listed
The system SHALL always list four cards — Plex, Jellyfin, Emby, IPTV — even if disabled.

#### Scenario: Empty hint
- GIVEN no card is enabled
- WHEN the list would be empty of startable hosts
- THEN hint text is `Enable a tuner in Settings (Plex, Jellyfin, Emby, or IPTV), Save, then press Start on that card.`

### Requirement: Per-card controls
Each card SHALL provide Start, Stop, Log, Graphs, Info, live connection count, **Allowed connections** NumberBox (1–16), **Open TV tuner links**, and a selectable detail/URL block.

#### Scenario: Enable does not listen
- GIVEN Plex is enabled in Settings but not started
- WHEN the operator has not pressed Start
- THEN port 8080 is not listening

#### Scenario: IPTV default
- GIVEN a new install with default settings
- WHEN TV Tuner is shown
- THEN the IPTV card is enabled (still requires Start)
- AND Plex, Jellyfin, Emby are disabled

### Requirement: Default ports and names
The system SHALL default ports to 8080 Plex, 8081 Jellyfin, 8082 Emby, 8083 IPTV. Friendly names: `epg.monster studio (plex|jellyfin|emby)` and `epg.monster studio (iptv)`. Device IDs are 8 uppercase hex, persisted; Start MUST NOT mint a new id.

#### Scenario: Legacy port migrate
- GIVEN a stored port of 5004, 5005, 5006, or 5007
- WHEN settings load
- THEN the port is rewritten to that kind’s default 8080–8083

### Requirement: HDHomeRun documents
While a Plex/Jellyfin/Emby host is running, the system SHALL serve `/discover.json`, `/lineup_status.json`, `/lineup.json` (URLs only `/auto/v{n}`), `/guide.xml`, `/xmltv.xml`. Jellyfin also serves `/tuner.m3u`.

#### Scenario: No provider URLs in lineup
- GIVEN a running Plex host
- WHEN `GET /lineup.json`
- THEN no provider stream URL appears
- AND each channel URL contains `/auto/v` plus its tuner number

### Requirement: IPTV playlist and EPG
The IPTV host SHALL serve `/playlist.m3u8` (also `/tuner.m3u`, `/playlist.m3u`, `/lineup.m3u`) with `url-tvg` pointing at local `/guide.xml` or the members feed when that option is on. Logos: playlist `tvg-logo` and EPG `<icon>` from the managed logo (or hosted `/logos/{tvg-id}.png` when Use local logos is on).

#### Scenario: Remux off
- GIVEN IPTV remux is unchecked
- WHEN `/playlist.m3u8` is fetched
- THEN entries use the visible provider URLs (same as Export)
- AND HDHomeRun `/lineup.json` on other cards still remuxes

#### Scenario: Remux on
- GIVEN IPTV remux is checked (default)
- WHEN `/playlist.m3u8` is fetched
- THEN entries point at `/auto/v{n}`
- AND a tune returns MPEG-TS with sync byte `0x47` near the start

### Requirement: Remux and failover
The system SHALL remux via bundled ffmpeg (profiles `mpeg2_ac3` default or `copy_aac`) or VLC copy-to-TS, buffer `RemuxBufferKb` (default from `RemuxOptions`), and fail over a live tune from a dead visible stream to a hidden backup.

#### Scenario: Live failover
- GIVEN a tune on the visible stream that dies and a hidden backup exists
- WHEN remux detects the source is gone
- THEN ffmpeg/VLC is restarted against the backup without leaking the provider URL to `/lineup.json`

### Requirement: Discovery
When Settings **Advertise tuners** is on, the system SHALL advertise running tuners on HDHomeRun UDP **65001** and SSDP. Allow LAN binds `0.0.0.0`; advertised URLs still show `127.0.0.1` when bind is `0.0.0.0` unless the operator is on another machine.

#### Scenario: Advertise running only
- GIVEN Advertise tuners is on and only IPTV is started
- WHEN a client queries SSDP / UDP 65001
- THEN only the running IPTV device is advertised
- AND display BaseUrl uses `127.0.0.1` when bound to `0.0.0.0`

### Requirement: Downspiral
When Jellyfin Downspiral is on, the system SHALL publish `/downspiral/index.json` plus `/downspiral/{group}.m3u8` and `/downspiral/{group}.xml` per Managed Output group, without changing HDHomeRun or `/tuner.m3u`.

#### Scenario: Per-group lists
- GIVEN Jellyfin Downspiral is enabled and two managed groups exist
- WHEN `/downspiral/index.json` is fetched
- THEN each group has an `.m3u8` and `.xml` entry
- AND `/tuner.m3u` is unchanged

### Requirement: Self-test
The system SHALL run `TunerClientProbe` mimicking Plex/Jellyfin/Emby/TiviMate HTTP and report the same per-kind check counts (Plex 5, Jellyfin 6, Emby 5, IPTV 4).

#### Scenario: Probe counts
- GIVEN all four hosts can be started
- WHEN Self-test runs
- THEN the report includes 5 Plex, 6 Jellyfin, 5 Emby, and 4 IPTV checks

### Requirement: Log and Graphs
The system SHALL open a verbose tuner log window and a graphs window (connections / request / stream breakdowns).

#### Scenario: Log window title
- GIVEN TV Tuner is open
- WHEN Log is clicked
- THEN a window titled `TV Tuner log` shows this session’s verbose requests

### Requirement: Info help
The system SHALL open setup help for adding the device: Plex/Jellyfin/Emby HDHomeRun at `http://127.0.0.1:{port}` and guide `{that}/guide.xml`; TiviMate playlist `:8083/playlist.m3u8` + EPG.

#### Scenario: Info lists local URLs
- GIVEN a card’s port is 8080
- WHEN Info is opened
- THEN the help includes `http://127.0.0.1:8080` and `/guide.xml`
- AND it does not list provider stream URLs
