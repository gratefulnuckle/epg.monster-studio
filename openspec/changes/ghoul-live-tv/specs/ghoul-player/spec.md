# ghoul-player Specification

## Purpose

G-houl is the Live TV sidecar: a TiviMate-style EPG guide plus a video pane, live
channels only, mounted inside the studio IPTV Player page or run as `ghoul.exe`.

## ADDED Requirements

### Requirement: Live TV only
The system SHALL tune **live** for every guide interaction. Clicking a past
programme block MUST NOT seek, request catchup, or open VOD.

#### Scenario: Past block tunes live
- GIVEN the grid shows a programme that ended an hour ago
- WHEN the operator clicks that block
- THEN that channel starts playing live

### Requirement: Studio sees only the curated lineup
The studio host SHALL read visible curated channels only (`list_managed`). Add
Sources lists MUST NOT be loaded into the player.

#### Scenario: Snapshot is visible-only
- GIVEN a curated channel with one visible variant and two hidden backups
- WHEN studio writes `{launch}/data/ghoul/iptv.m3u`
- THEN only the visible variant URL appears
- AND per-row User-Agent and extra headers are written as `#EXTVLCOPT:http-user-agent=` and matching header tags

### Requirement: Mount gates
The studio page SHALL refuse to mount the guide, with a named reason, when it
cannot play.

#### Scenario: Empty curated playlist
- GIVEN no curated channels
- WHEN the operator clicks **G-houl Player**
- THEN the guide does not mount
- AND the message is `Load a curated playlist in Playlist Editor first.`

#### Scenario: No playable stream URLs
- GIVEN curated channels whose visible variants have empty URLs
- WHEN the operator clicks **G-houl Player**
- THEN the guide does not mount
- AND the message is `Curated channels have no playable stream URLs.`

#### Scenario: No engine available
- GIVEN no GStreamer runtime, no `mpv.exe`, and no libmpv
- WHEN the operator clicks **G-houl Player**
- THEN the guide does not mount
- AND the message names what is missing

### Requirement: Guide snapshot on a worker
Fetch, gzip inflate, and XMLTV parse SHALL run on a dedicated worker. The UI
thread SHALL only read a snapshot, and the playback thread MUST NOT wait on HTTP
or XML parse.

#### Scenario: Retained window
- GIVEN an XMLTV with programmes spanning a week
- WHEN the worker publishes
- THEN only programmes whose window intersects now-8h to now+16h are retained

#### Scenario: Video starts before parse finishes
- GIVEN a large XMLTV still parsing
- WHEN the operator zaps to another channel
- THEN the new URL starts immediately
- AND the footer may read `EPG loading`
- AND titles fill in when that `tvg-id` reaches the snapshot

#### Scenario: Worker cancelled on teardown
- GIVEN a parse in flight
- WHEN the page is torn down or the process exits
- THEN the worker is cancelled

#### Scenario: tvg-id aliases
- GIVEN a playlist `tvg-id` of `KAUT-DT.us_locals1.us (src05)`
- WHEN the worker matches XMLTV `KAUT-DT.us`
- THEN the programmes are attached to that channel
- AND matching is case-insensitive

### Requirement: Categories
The category list SHALL be **All channels** first, then distinct `group-title`
values. An empty `group-title` SHALL be listed as **Other**.

#### Scenario: Empty group
- GIVEN an M3U entry with no `group-title`
- WHEN categories are built
- THEN that channel appears under **Other**

### Requirement: Core guide chrome
The guide SHALL show a left category list, channel rows with a horizontal time
grid, a now-line, a viewport of about **3 hours** scrollable across the retained
window, times as 24-hour `HH:MM`, and a top-right video preview with play/pause,
mute, volume, fullscreen, an engine dropdown and a UA dropdown.

#### Scenario: Channel column logo
- GIVEN a channel with `tvg-logo`
- WHEN the row renders
- THEN the logo is shown
- AND a channel without `tvg-logo` shows a letter tile

#### Scenario: Search
- GIVEN the operator presses `S` and types a substring
- WHEN no channel name matches, case-insensitively
- THEN the list is empty
- AND playback is unchanged

#### Scenario: Hide grid
- GIVEN the guide is visible
- WHEN the operator presses `G`
- THEN categories and the time grid hide and the video pane fills the G-houl view
- AND the mouse wheel zaps instead of scrolling the grid
- AND `G` restores the guide

#### Scenario: Keys
- GIVEN the guide has focus
- WHEN the operator presses the arrow keys, `PgUp`/`PgDn`, `Space`, `M`, `F`, `G`, `S`, or `Esc`
- THEN channel up/down (wrapping), page, play/pause, mute, video-pane fullscreen, toggle guide, focus search, and back/exit-fullscreen run
- AND the keys are not remappable in this cut

#### Scenario: Now/next line
- GIVEN the playing channel has a matched programme
- WHEN the overlay or footer renders
- THEN it shows channel name, now title, and `HH:MM-HH:MM`
- AND a channel with no match shows the name only, never a fabricated title

#### Scenario: Zap overlay
- GIVEN the operator presses channel up or down
- WHEN the new channel starts
- THEN a zap overlay shows index, name, group, and now-playing for about 4 seconds

### Requirement: Engines
Exactly one engine SHALL be live at a time. Switching engines SHALL stop the
current backend and reload the current URL with the same UA and headers.

#### Scenario: Never two players
- GIVEN `gstreamer` is playing
- WHEN the operator selects `mpv-ipc`
- THEN the GStreamer backend is stopped before mpv starts

#### Scenario: Missing engines are disabled
- GIVEN `mpv.exe` is absent
- WHEN the engine dropdown renders
- THEN `mpv-ipc` is disabled with a short reason

#### Scenario: Saved engine is missing
- GIVEN the saved engine is `gstreamer` and its runtime is absent
- WHEN the host starts
- THEN the first available engine is used, in the order `gstreamer`, `mpv-ipc`, `libmpv`

#### Scenario: Switch fails
- GIVEN a switch to another engine fails to start
- WHEN the failure is reported
- THEN the previous engine is restored if it can still start, else GStreamer if present, else playback is stopped

#### Scenario: Headers reach the backend
- GIVEN a channel with a User-Agent and a `Referer`
- WHEN GStreamer plays it
- THEN `source-setup` sets `user-agent`, `is-live`, and `extra-headers`
- AND mpv IPC and libmpv set `user-agent` and CRLF-joined `http-header-fields`

#### Scenario: ffmpeg and VLC are not engines
- GIVEN the engine dropdown
- WHEN it renders
- THEN it offers only `gstreamer`, `mpv-ipc`, and `libmpv`

### Requirement: User-Agent presets
Presets SHALL be independent of the engine and applied to the current stream
request. Resolution order is the curated row or M3U entry UA when non-empty, else
the preset dropdown.

#### Scenario: Row UA wins
- GIVEN an entry with `#EXTVLCOPT:http-user-agent=Foo/1`
- WHEN the preset is `vlc`
- THEN the request uses `Foo/1`

#### Scenario: Preset table
- GIVEN the UA dropdown
- WHEN it renders
- THEN it offers `tivimate`, `vlc`, `gse`, `smarters`, `browser`, and `custom`
- AND the fallback default is `tivimate`
- AND an empty Custom string is treated as `tivimate`, never a blank `User-Agent`

#### Scenario: Preset change reloads
- GIVEN a channel is playing
- WHEN the operator changes the preset
- THEN the current channel reloads with the new `User-Agent`
- AND the playlist is not rebuilt

### Requirement: Errors are not modals
Stream and EPG failures SHALL be reported in place, never as a modal dialog, and
MUST NOT reveal the URL.

#### Scenario: Stream refused
- GIVEN a stream returns 401, 403, or 404, or fails to demux
- WHEN the failure surfaces
- THEN the pane reads `Stream failed` plus the code when known
- AND the frame is cleared
- AND zap still works
- AND any toast or log line redacts the URL

#### Scenario: XMLTV failure
- GIVEN the XMLTV fetch or parse fails
- WHEN the failure surfaces
- THEN the footer reads `EPG failed`
- AND the channel list remains

### Requirement: Member feed XMLTV
The studio host SHALL use the my.epg.monster member feed built from the uploaded
`channels.json`, downloaded with `Authorization: Bearer` and `X-EPG-Member-Key`.
The key MUST NOT be placed in the URL, and the player SHALL be given the file
path, not the key.

#### Scenario: Feed available
- GIVEN a member key and a built feed
- WHEN studio prepares the guide
- THEN bytes are written to `{launch}/data/ghoul/iptv.xml`
- AND a temp guide is NOT synthesized from local `epg.xml` plus `channels.json`

#### Scenario: No key or fetch failed
- GIVEN no member key, no feed, or a failed fetch
- WHEN studio prepares the guide
- THEN the guide still mounts with the playlist
- AND programme cells are empty
- AND the operator is told to upload `channels.json` so now-playing can appear

### Requirement: Privacy
Stream URLs, `epgm_` access keys, and `Authorization` values MUST NOT be logged,
toasted in full, or written to crash payloads.

#### Scenario: Saved prefs
- GIVEN the operator quits with a channel playing
- WHEN prefs are written
- THEN `{launch}/data/ghoul/player.json` holds engine id, UA preset, custom UA string, and last channel index
- AND it holds no provider stream URL
- AND the lineup on the next mount comes from the current curated snapshot, not from that file

### Requirement: Standalone ghoul.exe
`ghoul.exe` SHALL run the same Live TV UI with no studio database, no Add Sources,
and no members API.

#### Scenario: Open sources
- GIVEN a fresh launch
- WHEN the operator opens a playlist and an XMLTV, each a local file or an HTTP(S) URL
- THEN the playlist is required to play
- AND a playlist without XMLTV still plays, with channel names and empty cells

#### Scenario: Restore last session
- GIVEN a previous session
- WHEN `ghoul.exe` starts
- THEN `{exe}/data/ghoul-last.json` restores playlist path or URL, XMLTV path or URL, engine id, UA preset (and custom string), and last channel index
- AND it does not store provider stream URLs from inside the M3U

#### Scenario: CLI
- GIVEN a command line
- WHEN `--playlist`, `--xml`, `--ua`, `--header`, or `--gst-dir` is passed
- THEN it is honoured
- AND `--url` and `--file` are not part of this Live TV cut

### Requirement: Optional GStreamer install
Declining the Windows GStreamer install tick (or `/NOGHOUL`) SHALL remove only the
GStreamer engine.

#### Scenario: Studio still starts
- GIVEN no GStreamer runtime is installed
- WHEN studio launches
- THEN the process starts normally
- AND the IPTV Player page is still listed and still opens
- AND the `gstreamer` engine is disabled with a reason
