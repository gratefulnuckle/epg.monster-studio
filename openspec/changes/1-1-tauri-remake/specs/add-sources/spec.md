# Delta for add-sources

## Purpose

Load M3U/M3U8 playlists from files or URLs, browse groups/channels, search, play, copy, and add rows into the managed playlist. WinUI: `ManualAuditPage` + `AddSourceDialog`.

## ADDED Requirements

### Requirement: Source tabs
The system SHALL show each loaded playlist as a closable tab, with a `+` control to add a source.

#### Scenario: Empty state
- GIVEN no sources
- WHEN Add Sources is shown
- THEN an empty state with a large icon and an **Add source…** action is shown (not status text only)

#### Scenario: Close tab
- GIVEN a source tab
- WHEN the operator closes it
- THEN that source is removed from the session store (matching C# remove)

### Requirement: Add file source
The system SHALL add a source from a local `.m3u` or `.m3u8` file and parse it off the UI thread with a progress indicator.

#### Scenario: Load file
- GIVEN a valid local m3u
- WHEN the operator picks it in Add source
- THEN a tab appears with the file name
- AND groups and channels are listed
- AND `channel_count` matches parsed rows

### Requirement: Add URL source with headers
The system SHALL add a source from an HTTP(S) URL with optional User-Agent, Authorization, Cookie, and arbitrary header pairs.

#### Scenario: Custom headers
- GIVEN a URL that requires a User-Agent
- WHEN the operator enters the URL and UA and confirms
- THEN the download uses those headers
- AND headers persist on the source as `headers_json`

#### Scenario: Refresh URL
- GIVEN a URL source
- WHEN the operator refreshes
- THEN the playlist is re-fetched (using etag/last-modified cache when present)

### Requirement: Parser fields
The system SHALL parse name, `group-title` (missing → `Ungrouped`), tvg-id, tvg-name, tvg-logo, URL, remaining EXTINF attrs (`attrs_json`), and original line order.

#### Scenario: Missing group
- GIVEN an EXTINF with no group-title
- WHEN parsed
- THEN `group_title` is `Ungrouped`

### Requirement: Browse columns
The system SHALL show a group list and a virtualized channel list with Play, name, tvg-id, and URL.

#### Scenario: Copy tvg-id
- GIVEN a channel with a tvg-id
- WHEN the operator clicks the tvg-id
- THEN the full tvg-id is copied and a toast confirms

#### Scenario: Copy URL
- GIVEN a channel URL
- WHEN the operator clicks the truncated URL
- THEN the full URL is copied and a toast confirms

### Requirement: Search all loaded sources
The system SHALL filter by name, group, tvg-id, and URL across **all loaded sources** when the query has at least 2 characters, capping results at 400.

#### Scenario: Short query ignored
- GIVEN sources loaded
- WHEN the operator types one character
- THEN no all-source search runs

#### Scenario: CNN search
- GIVEN large sources
- WHEN the operator types `CNN`
- THEN matching rows appear without freezing the UI

### Requirement: Add to managed playlist
The system SHALL let the operator add a source row as a new managed channel draft (group **Unassigned**) or as a hidden backup on an existing channel.

#### Scenario: New draft
- GIVEN a managed playlist exists
- WHEN the operator adds a source row as a new channel
- THEN a managed channel is created in group `Unassigned`
- AND Playlist Editor focuses that draft / group field

#### Scenario: Hidden backup
- GIVEN a managed channel
- WHEN the operator attaches a different URL from a source
- THEN a `hidden_backup` variant is added
- AND the visible URL is unchanged
- AND a URL identical to the visible URL is refused

#### Scenario: Backup picker is grouped
- GIVEN the backup target picker
- WHEN it opens
- THEN channels are shown in a **group tree**, not a flat list

#### Scenario: Add column hidden until managed exists
- GIVEN no managed playlist yet
- WHEN Add Sources lists channels
- THEN the add-to-playlist control is hidden
