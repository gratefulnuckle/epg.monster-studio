# Proposal: G-houl Live TV player

## Why

`docs/superpowers/specs/2026-09-04-ghoul-live-tv-design.md` is approved for
implementation planning. It supersedes the parked `ghoul/openspec/specs/ghoul-player`
model, in which the studio **IPTV Player** page only launched a detached
`ghoul.exe`. Studio now **embeds** the Live TV UI; standalone `ghoul.exe` stays so
the player runs without studio.

`docs/V3.md` parks IPTV Player, G-houl, and GStreamer packing as v3. This change
*is* that v3 work, so it also flips the "hidden until v3" notes in
`openspec/project.md` and `docs/V3.md`.

## What

A TiviMate-style live-only guide (categories, time grid, video pane) mounted in
two hosts over one Rust playback backend:

- **ghoul-guide** — M3U + XMLTV fetch/inflate/parse on a worker; UI reads a snapshot.
- **ghoul-play** — one live URL with UA + extra headers into the video pane, over
  three engines: `gstreamer` (default), `mpv-ipc`, `libmpv`.
- **ghoul-pane** — native child window the engine draws into, tracking the DOM rect.
- **studio host** — IPTV Player page: status landing → **G-houl Player** button →
  guide mounted in-page, fed by the curated Playlist Editor lineup + member XMLTV.
- **standalone host** — `ghoul.exe`, same UI, operator-supplied M3U + XMLTV.

## Out of scope

Verbatim from the design's "Out of this cut": VOD, catchup, DVR, sports, Stremio,
Add Sources as a lineup, ffmpeg/VLC engines, favorites, A–Z jump, transparent
overlay, 3-column view, metadata badges, resolution filter, EPG shift, mini media
bar, logo tile settings, remappable keys, pop-out, PiP, multiview,
`--url` / `--file` single-file player chrome.

Unchanged by this change: Playlist Editor **Play** (Settings mpv / VLC), Stream
Audit ffmpeg/ffprobe, Settings' two-player list.

## Known gaps (tracked, not dropped)

- The design names `libmpv.so.2` / `libmpv.dylib`, but bundled GStreamer, `mpv.exe`,
  and `libmpv-2.dll` are all Windows paths. Phase 1–6 ship the Windows pane host.
  Phase 7 adds the X11 and macOS pane hosts; until it lands, non-Windows shows the
  status landing with the player button disabled.
