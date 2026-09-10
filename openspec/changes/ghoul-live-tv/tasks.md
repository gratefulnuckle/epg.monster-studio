# Tasks

## Phase 1 — ghoul-guide crate

- [x] Create `src-tauri/crates/ghoul-guide` and add it to the workspace members
- [x] Port `tvg_keys` verbatim from the parked tree, with its alias tests
- [x] Port the M3U parser and extend it with `tvg-logo`, `#EXTVLCOPT:http-user-agent=`, and extra `#EXTVLCOPT`/`#EXTHTTP` headers per entry
- [x] Build categories: distinct `group-title`, **All channels** first, empty group-title becomes **Other**
- [x] Port the XMLTV worker; widen `Snapshot` to carry programme rows plus now/next
- [x] Keep the now−8h/now+16h window, streamed input, 32 MiB playlist cap, gzip inflate, partial publishes
- [x] Make `Snapshot`, `Channel`, `Category` serde-serializable for the UI
- [x] Unit tests: groups/All/Other, wrap zap index, window drop, gzip, alias match

## Phase 2 — ghoul-play crate

- [x] Create `src-tauri/crates/ghoul-play` with the `Engine` trait (start/stop/reload/pause/mute/volume/status)
- [x] UA preset table and the resolution order (row UA wins, else preset, empty Custom means `tivimate`)
- [x] `mpv-ipc` engine: `mpv.exe --wid` + named-pipe JSON IPC
- [x] `libmpv` engine: runtime `libloading`, never a link-time import
- [x] `gstreamer` engine: spawn and drive the `ghoul-gst` sidecar over stdin/stdout
- [x] `availability()` reporting each engine plus a short reason when missing
- [x] Fallback order `gstreamer` → `mpv-ipc` → `libmpv`; failed switch rolls back
- [x] Unit tests: UA resolution, extra headers preserved, mock backend start/stop/reload, never two live

## Phase 3 — ghoul-pane crate

- [x] Create `src-tauri/crates/ghoul-pane`; Windows child window over the WebView2 HWND
- [x] `set_rect` coalesced and no-op when unchanged; black background brush
- [x] `destroy` on unmount; non-Windows returns `Unsupported`

## Phase 4 — ghoul-gst sidecar

- [x] Create `src-tauri/crates/ghoul-gst` as a `[workspace] exclude` binary
- [x] Port `gstplay.rs` and `runtime.rs` from the parked tree, keeping the Windows overlay comments
- [x] Line protocol: `open/ua/header/play/pause/mute/volume/rect/quit` in, `playing/paused/buffering/error/eos` out
- [x] `source-setup` sets `user-agent`, `is-live`, `extra-headers`
- [x] `scripts/build-ghoul-gst.ps1` / `.sh` staging to `{app}/tools/ghoul/`

## Phase 5 — studio host

- [x] `studio-core::export::export_ghoul_snapshot` (visible-only, per-row UA and header tags)
- [x] `src-tauri/src/ghoul.rs`: status, prepare, snapshot, mount/unmount, set_rect, play/stop/pause/mute/volume, engine, UA, prefs commands
- [x] Member XMLTV download with `Authorization: Bearer` + `X-EPG-Member-Key`; key never in the URL; bytes to `{launch}/data/ghoul/iptv.xml`
- [x] `{launch}/data/ghoul/player.json` for engine, UA preset, custom UA, last channel index
- [x] Register the commands in `invoke_handler`
- [x] Unit tests: gate on empty curated lineup and on a lineup with no playable URLs

## Phase 6 — UI

- [x] `src/ghoul-types.ts` DTOs
- [x] `src/ghoul-ui.ts`: categories, channel rows, 3-hour time grid, now-line, 24-hour `HH:MM`
- [x] Any programme block (past, now, future) tunes **live**
- [x] Video chrome: play/pause, mute, volume, fullscreen, engine dropdown, UA dropdown
- [x] Keys `↑ ↓ PgUp PgDn Space M F G S Esc`, wrap on channel up/down
- [x] Hide-grid `G` expands the video pane; wheel zaps when hidden, scrolls the grid when visible
- [x] Search `S`, case-insensitive substring on channel name, playback unchanged on no hits
- [x] `tvg-logo` tile with a letter-tile fallback
- [x] Now/next highlight, overlay/footer name + title + `HH:MM–HH:MM`, ~4s zap overlay
- [x] Empty/loading/failed states: `EPG loading`, names only, `EPG failed` in the footer, never a modal
- [x] `src/ghoul.ts` status landing plus the **G-houl Player** button, with the three do-not-mount messages
- [x] `src/styles.css` `.gh-*` rules using `#0C0C10` / `#16161E`
- [x] `src/shell.ts`: `IPTV Player` nav item after TV Tuner; dispose unmounts the pane
- [x] `public/ghoul.png` for the button

## Phase 7 — standalone ghoul.exe

- [ ] `src-tauri/crates/ghoul-app` Tauri binary plus `ghoul.html` vite entry, reusing `ghoul-ui.ts`
- [ ] Open playlist / Open XMLTV (local file or HTTP(S)); playlist required to play
- [ ] `{exe}/data/ghoul-last.json` restore, without provider stream URLs
- [ ] CLI `--playlist`, `--xml`, `--ua`, `--header`, `--gst-dir`

## Phase 8 — non-Windows pane hosts

- [ ] X11 pane host
- [ ] macOS pane host

## Phase 9 — docs and specs

- [ ] Promote `openspec/changes/ghoul-live-tv/specs/ghoul-player/spec.md` into `openspec/specs/`
- [ ] Update `openspec/project.md` and `docs/V3.md` so IPTV Player is no longer hidden
- [ ] `.\scripts\openspec-gh.ps1 -Change ghoul-live-tv`
