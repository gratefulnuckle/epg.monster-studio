# Tasks

Implementation of the 1:1 remake. Each task is done only when it matches the C# oracle and the matching spec scenarios.

## 1. Scaffold

- [x] 1.1 Create Tauri v2 + TypeScript + Rust workspace (`src/`, `src-tauri/`). `productName`: `epg.monster studio`. Identifier: `monster.epg.studio`.
- [x] 1.2 Add crates `studio-core` and `studio-tuner`. GPL-3.0 headers. Copy `assets/logo.ico` into the Tauri bundle.
- [x] 1.3 Wire `openspec/project.md` conventions into `tauri.conf.json` (window title, dark background `#0C0C10`).
- [x] 1.4 Add `tools/README.md` describing ffmpeg/mpv layout (binaries gitignored).

## 2. Persistence (studio-core)

- [x] 2.1 Implement `SqliteStore` DDL + `EnsureColumn` migrations identical to C#.
- [x] 2.2 AppData path + legacy `iptv-studio` copy.
- [x] 2.3 Settings load/save compatible with C# `AppSettings` JSON.
- [x] 2.4 FTS5 search (min 2 chars, cap 400 for all-source).
- [x] 2.5 Tests: open a fixture DB; round-trip settings; FTS query.

## 3. Parser and sources

- [x] 3.1 Port `M3uParser` (name, group, tvg-*, url, attrs, Ungrouped).
- [x] 3.2 File + URL fetch with headers, cache, refresh.
- [x] 3.3 Tauri commands for add/remove/list/search.
- [x] 3.4 Tests from `M3uParserTests` fixtures.

## 4. App shell UI

- [x] 4.1 Splash (logo, version, tool list, XMLTV %).
- [x] 4.2 Main window: nav items + order + footer Settings + logo About.
- [x] 4.3 Title-bar search on Add Sources / Editor / Output only.
- [x] 4.4 Toasts 3s, tray minimize, daily log + 5s heartbeat.
- [x] 4.5 Crash report window + redaction.

## 5. Add Sources UI

- [x] 5.1 Closable source tabs + empty state + Add source dialog (file/URL/headers).
- [x] 5.2 Virtualized group + channel lists; Play; copy URL/tvg-id.
- [x] 5.3 Search all sources; add draft / hidden backup; grouped backup picker.

## 6. Playlist Editor

- [x] 6.1 Channel rows: 40×40 logo, red glyph, green check via `IsKnownTvgId`.
- [x] 6.2 Edit fields with verbatim headers; group typeahead; 72×72 logo preview.
- [x] 6.3 tvg-id AutoSuggest (filter/order/cap/format from C#).
- [x] 6.4 Green field + check on keystroke; now-playing card + zone combo.
- [x] 6.5 Stream rows: Play, Info, up/down, Remove, Add stream.
- [x] 6.6 In-place group rename (right-click / double-click).
- [x] 6.7 Add channels from sources (filtered). Visibility invariant tests.

## 7. EPG Audit

- [x] 7.1 Fetch/cache epg.monster XMLTV; strip epgshare; index programmes.
- [x] 7.2 Exact + fuzzy; skip Dummy ids; inline suggest; green match.
- [x] 7.3 Auto match: score combo + per-group checks.
- [x] 7.4 Full-page catalog browse with orange section headers.
- [x] 7.5 Search images; apply tvg-id/logo without clobbering logos unless asked.

## 8. Logo Audit

- [x] 8.1 Scan missing/invalid/broken/player-reject (VLC UA GET).
- [x] 8.2 Thumbnails, orange group counts, Clear logo, batch set.
- [x] 8.3 Save Logos window: serial PNG pack `{group}/{tvg-id}.png`.

## 9. Stream Audit

- [x] 9.1 Serial ffmpeg/ffprobe; delay 6000; timeout 15000; one process.
- [x] 9.2 Offline-slate average-hash; optional blackdetect.
- [x] 9.3 Grades A–F feed cards + group headers; persist `auditprocess.db`.
- [x] 9.4 Pause/resume/cancel; resume prompt; pause while playing.
- [x] 9.5 Auto-swap + undo log.
- [x] 9.6 Audit specific (groups + channels); Run today's groups (no auto-start).
- [x] 9.7 `# Results` window (graph, F breakdown, elapsed/ETA, export F-list).

## 10. Managed Output

- [x] 10.1 Columns Name / Group / tvg-id / Visible URL / Variants / Audit.
- [x] 10.2 Export visible / export all.
- [x] 10.3 Tuner lineup dialog: pick, auto-number, collision swap.
- [x] 10.4 Export channels.json; upload disabled without key.

## 11. TV Tuner

- [x] 11.1 Four cards; Start all / Stop all / Log / Graphs / Self-test / Info.
- [x] 11.2 TCP hosts 8080–8083; persist device ids; migrate 5004–5007.
- [x] 11.3 Routes: discover, lineup, guide, m3u, `/auto/v{n}`, logos, downspiral.
- [x] 11.4 Remux ffmpeg/VLC + buffer; live failover; SSDP + UDP 65001.
- [x] 11.5 IPTV remux toggle; member vs local EPG; `url-tvg`.
- [x] 11.6 Port `TunerClientProbe` tests (5/6/5/4).

## 12. Settings + members + playback

- [x] 12.1 Every Settings tile and label from `SettingsPage.xaml`.
- [x] 12.2 Detect bundled tools + Save.
- [x] 12.3 Members ping / PUT / job poll / crash POST; never log keys.
- [x] 12.4 PlayerService mpv/VLC args + header passing.

## 13. Parity gate

- [ ] 13.1 Side-by-side pass of linux-parity.md acceptance (Playlist Editor P0).
- [ ] 13.2 Open a real C# AppData DB (local only) and walk Add Sources → Editor → EPG → Logo → Stream → Output → Tuner.
- [ ] 13.3 Self-test all four tuners.
- [ ] 13.4 Confirm no provider URLs in `/lineup.json`.
- [x] 13.5 String audit: nav, placeholders, Settings headers match XAML.

## 14. Archive

- [x] 14.1 `openspec validate 1-1-tauri-remake`
- [x] 14.2 Archive the change so `openspec/specs/` becomes the living source of truth.
