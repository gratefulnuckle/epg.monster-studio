# Design: G-houl Live TV player

Source of truth for behavior: `docs/superpowers/specs/2026-09-04-ghoul-live-tv-design.md`.
This document records **how** it lands in this tree, and the three places where a
locked constraint forced a decision the design did not spell out.

## Crate layout

```
src-tauri/crates/
  ghoul-guide/   M3U parse · XMLTV worker · tvg_keys · Snapshot        (workspace member)
  ghoul-play/    Engine trait · availability · UA presets · mpv-ipc · libmpv
  ghoul-pane/    native video-pane host (child window over the webview)
  ghoul-gst/     GStreamer sidecar bin                       (workspace EXCLUDED)
  ghoul-app/     standalone ghoul.exe (Tauri)                (phase 6)
```

Frontend:

```
src/ghoul-types.ts   DTOs shared by both hosts
src/ghoul-ui.ts      the Live TV UI (categories · time grid · video chrome · keys)
src/ghoul.ts         studio IPTV Player page: status landing + G-houl Player button
src/styles.css       .gh-* rules, studio tokens only
```

## Decision 1 — GStreamer runs out of process

Locked constraint: *"GStreamer remains optional at Windows install (tick /
`/NOGHOUL`). That only removes the GStreamer engine, not the IPTV Player page."*

If `epg-monster-studio.exe` linked `gstreamer-rs`, `gstreamer-1.0-0.dll` would sit
in the exe's import table. An operator who declines the install tick would not lose
one engine — Windows would refuse to load the process and **studio would not start
at all**. Same for a linked `libmpv`.

So no engine may be a load-time import of the studio binary:

| Engine | Mechanism | Studio import table |
|--------|-----------|---------------------|
| `gstreamer` | child process `ghoul-gst.exe --hwnd <n>`, line protocol on stdin/stdout | untouched |
| `mpv-ipc` | child process `mpv.exe --wid=<hwnd> --input-ipc-server=<pipe>`, JSON IPC | untouched |
| `libmpv` | in-process, but `libloading` at runtime — never linked | untouched |

The design's `ghoul-play` trait is preserved; only the `gstreamer` arm's
implementation is a process boundary instead of a function call. `ghoul-gst` is a
`[workspace] exclude` member so `cargo check --workspace` and CI stay green on
machines without GStreamer dev libraries; it is built by
`scripts/build-ghoul-gst.ps1` / `.sh` and staged to `{app}/tools/ghoul/ghoul-gst.exe`.

The sidecar keeps the parked `gstplay.rs` playbin + `VideoOverlay` code and the
parked `runtime.rs` GStreamer-prefix search verbatim — including the two hard-won
Windows comments (no `set_render_rectangle` after d3d11 subclasses the HWND; no
redundant/zero-size `MoveWindow` on the overlay HWND).

## Decision 2 — the video pane is a native child window

Video is never a `<video>` tag. `ghoul-pane` creates a `WS_CHILD | WS_CLIPSIBLINGS`
window on the Tauri window, held above the WebView2 child HWND, and moves it to
track the client rect of `#gh-video`:

```
ResizeObserver(#gh-video) ─┐
window resize ─────────────┼→ ghoul_set_rect(x,y,w,h) → MoveWindow(pane, …)
scroll / hide-grid ────────┘
```

Rect updates are coalesced on an animation frame and dropped when unchanged
(a redundant `MoveWindow` on a GST-subclassed HWND deadlocks the UI thread —
see the parked `chrome_win.rs` note). The pane paints black, so a stopped engine
shows a black rect and never a hole.

Teardown is mandatory, not best-effort: nav away, `disposePage`, window close and
process exit all call `ghoul_unmount()`, which stops the engine **then** destroys
the pane. This is what keeps the overlay from punching through Playlist Editor.

## Decision 3 — Windows first, other platforms tracked

`ghoul-pane` compiles on every target. Non-Windows returns `Unsupported`, the
IPTV Player page still renders its status landing, and the **G-houl Player** button
is disabled. Phase 7 adds the X11 (`XReparentWindow` + `gst_video_overlay`) and
macOS (`NSView` + `--wid`) hosts. Recorded as a gap in `proposal.md` rather than
silently narrowed.

## Decision 4 — two tvg-id helpers, on purpose

`studio-core::epg::tvg_lookup_ids` returns case-preserving candidates with space/dot
variants, for catalog matching. `ghoul-guide::tvg_keys` returns **lowercased** keys
and is the map key for the snapshot. They are not interchangeable; `tvg_keys` is
ported verbatim from the parked `ghoul/src-tauri/crates/ghoul/src/guide.rs` (whose
tests come with it) and stays the only key used inside the guide worker.

## Decision 5 — the snapshot carries programme rows, not just now/next

The parked worker kept only now/next, because the parked UI only had a footer. The
grid needs every programme in the retained window, so `Snapshot` becomes:

```rust
pub struct Snapshot {
    pub loading: bool,
    pub err: Option<String>,
    pub progs: HashMap<String, Vec<Prog>>,   // tvg_keys() key → window rows
    pub now:   HashMap<String, NowOn>,
    pub next:  HashMap<String, NowOn>,
}
```

Window stays **now − 8h … now + 16h**; rows outside are dropped at parse time, so
the map cannot grow without bound. Input is streamed (`Read` + `take`), never
`read_to_string` of unbounded bytes; playlist fetch cap stays 32 MiB, XML 256 MiB.
The worker publishes partials every 200 kept programmes so the grid fills in while
a large XMLTV is still parsing.

## Data flow — studio host

```
list_managed(hydrate)            store.headers_for_channels()
        │                                  │
        └──────────────┬───────────────────┘
                       ▼
   export_ghoul_snapshot()  →  {launch}/data/ghoul/iptv.m3u
   (visible variant only, #EXTVLCOPT:http-user-agent= + header tags)
                       │
 members feed  ────────┼───→  {launch}/data/ghoul/iptv.xml
 (Bearer + X-EPG-Member-Key; key never in the URL, path handed to the player)
                       ▼
                  ghoul-guide worker  →  Snapshot  →  ghoul-ui
```

`export_ghoul_snapshot` is a new `studio-core::export` function beside
`export_visible_only`; it does not change `export_visible_only`, which Managed
Output still calls.

## Prefs

| Host | File | Fields |
|------|------|--------|
| studio | `{launch}/data/ghoul/player.json` | engine id, UA preset id, custom UA, last channel index |
| standalone | `{exe}/data/ghoul-last.json` | the above + playlist path/URL + XMLTV path/URL |

Neither file ever stores a provider stream URL from inside the M3U. The lineup
always comes from the live curated snapshot, never from `player.json`.

## Privacy

Reuses the existing redaction discipline. `ghoul::log::redact_url` (ported) is the
only way a URL reaches a log line. Stream URLs, `epgm_…` keys, and `Authorization`
values never enter logs, toasts, crash payloads, or issue bodies. `data/ghoul/` is
covered by the existing `/data/` gitignore rule.

## Testing

Automated, no live provider URLs (design "Testing" section, verbatim):
UA resolution · `tvg_keys` aliases · playlist groups / **All channels** / **Other** /
wrap zap · snapshot window drops out-of-range programmes · engine trait against a
mock backend (start/stop/reload, never two live) · studio gate (empty curated
lineup and URL-less lineup do not mount).

Manual (needs a GUI, a GStreamer runtime, `mpv.exe`, and a test stream — none of
which exist in a headless checkout): the design's four manual items.
