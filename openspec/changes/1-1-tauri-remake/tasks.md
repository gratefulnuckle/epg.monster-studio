# Tasks

Implementation of the 1:1 remake. Each task is done only when it matches the C# oracle and the matching spec scenarios.

## 1. Scaffold

- [ ] 1.1 Create Tauri v2 + TypeScript + Rust workspace (`src/`, `src-tauri/`). `productName`: `epg.monster studio`. Identifier: `monster.epg.studio`.
- [ ] 1.2 Add crates `studio-core` and `studio-tuner`. GPL-3.0 headers. Copy `assets/logo.ico` into the Tauri bundle.
- [ ] 1.3 Wire `openspec/project.md` conventions into `tauri.conf.json` (window title, dark background `#0C0C10`).
- [ ] 1.4 Add `tools/README.md` describing ffmpeg/mpv layout (binaries gitignored).

## 2. Persistence (studio-core)

- [ ] 2.1 Implement `SqliteStore` DDL + `EnsureColumn` migrations identical to C#.
- [ ] 2.2 AppData path + legacy `iptv-studio` copy.
- [ ] 2.3 Settings load/save compatible with C# `AppSettings` JSON.
- [ ] 2.4 FTS5 search (min 2 chars, cap 400 for all-source).
- [ ] 2.5 Tests: open a fixture DB; round-trip settings; FTS query.

## 3. Parser and sources

- [ ] 3.1 Port `M3uParser` (name, group, tvg-*, url, attrs, Ungrouped).
- [ ] 3.2 File + URL fetch with headers, cache, refresh.
- [ ] 3.3 Tauri commands for add/remove/list/search.
- [ ] 3.4 Tests from `M3uParserTests` fixtures.

## 4. App shell UI

- [ ] 4.1 Splash (logo, version, tool list, XMLTV %).
- [ ] 4.2 Main window: nav items + order + footer Settings + logo About.
- [ ] 4.3 Title-bar search on Add Sources / Editor / Output only.
- [ ] 4.4 Toasts 3s, tray minimize, daily log + 5s heartbeat.
- [ ] 4.5 Crash report window + redaction.

## 5. Add Sources UI

- [ ] 5.1 Closable source tabs + empty state + Add source dialog (file/URL/headers).
- [ ] 5.2 Virtualized group + channel lists; Play; copy URL/tvg-id.
- [ ] 5.3 Search all sources; add draft / hidden backup; grouped backup picker.

## 6. Playlist Editor

- [ ] 6.1 Channel rows: 40×40 logo, red glyph, green check via `IsKnownTvgId`.
- [ ] 6.2 Edit fields with verbatim headers; group typeahead; 72×72 logo preview.
- [ ] 6.3 tvg-id AutoSuggest (filter/order/cap/format from C#).
- [ ] 6.4 Green field + check on keystroke; now-playing card + zone combo.
- [ ] 6.5 Stream rows: Play, Info, up/down, Remove, Add stream.
- [ ] 6.6 In-place group rename (right-click / double-click).
- [ ] 6.7 Add channels from sources (filtered). Visibility invariant tests.

## 7. EPG Audit

- [ ] 7.1 Fetch/cache epg.monster XMLTV; strip epgshare; index programmes.
- [ ] 7.2 Exact + fuzzy; skip Dummy ids; inline suggest; green match.
- [ ] 7.3 Auto match: score combo + per-group checks.
- [ ] 7.4 Full-page catalog browse with orange section headers.
- [ ] 7.5 Search images; apply tvg-id/logo without clobbering logos unless asked.

## 8. Logo Audit

- [ ] 8.1 Scan missing/invalid/broken/player-reject (VLC UA GET).
- [ ] 8.2 Thumbnails, orange group counts, Clear logo, batch set.
- [ ] 8.3 Save Logos window: serial PNG pack `{group}/{tvg-id}.png`.

## 9. Stream Audit

- [ ] 9.1 Serial ffmpeg/ffprobe; delay 6000; timeout 15000; one process.
- [ ] 9.2 Offline-slate average-hash; optional blackdetect.
- [ ] 9.3 Grades A–F feed cards + group headers; persist `auditprocess.db`.
- [ ] 9.4 Pause/resume/cancel; resume prompt; pause while playing.
- [ ] 9.5 Auto-swap + undo log.
- [ ] 9.6 Audit specific (groups + channels); Run today's groups (no auto-start).
- [ ] 9.7 `# Results` window (graph, F breakdown, elapsed/ETA, export F-list).

## 10. Managed Output

- [ ] 10.1 Columns Name / Group / tvg-id / Visible URL / Variants / Audit.
- [ ] 10.2 Export visible / export all.
- [ ] 10.3 Tuner lineup dialog: pick, auto-number, collision swap.
- [ ] 10.4 Export channels.json; upload disabled without key.

## 11. TV Tuner

- [ ] 11.1 Four cards; Start all / Stop all / Log / Graphs / Self-test / Info.
- [ ] 11.2 TCP hosts 8080–8083; persist device ids; migrate 5004–5007.
- [ ] 11.3 Routes: discover, lineup, guide, m3u, `/auto/v{n}`, logos, downspiral.
- [ ] 11.4 Remux ffmpeg/VLC + buffer; live failover; SSDP + UDP 65001.
- [ ] 11.5 IPTV remux toggle; member vs local EPG; `url-tvg`.
- [ ] 11.6 Port `TunerClientProbe` tests (5/6/5/4).

## 12. Settings + members + playback

- [ ] 12.1 Every Settings tile and label from `SettingsPage.xaml`.
- [ ] 12.2 Detect bundled tools + Save.
- [ ] 12.3 Members ping / PUT / job poll / crash POST; never log keys.
- [ ] 12.4 PlayerService mpv/VLC args + header passing.

## 13. Parity gate

- [ ] 13.1 Side-by-side pass of linux-parity.md acceptance (Playlist Editor P0).
- [ ] 13.2 Open a real C# AppData DB (local only) and walk Add Sources → Editor → EPG → Logo → Stream → Output → Tuner.
- [ ] 13.3 Self-test all four tuners.
- [ ] 13.4 Confirm no provider URLs in `/lineup.json`.
- [ ] 13.5 String audit: nav, placeholders, Settings headers match XAML.

## 14. Archive

- [ ] 14.1 `openspec validate 1-1-tauri-remake`
- [ ] 14.2 Archive the change so `openspec/specs/` becomes the living source of truth.
