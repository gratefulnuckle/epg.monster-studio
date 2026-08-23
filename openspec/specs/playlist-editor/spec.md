# playlist-editor Specification

## Purpose

Curate managed channels: metadata, logos, tvg-id matching, now-playing, visible stream plus hidden backups.

## Requirements

### Requirement: Toolbar count and bulk actions
The Playlist Editor action row SHALL keep Load / Add channels / Export on the left, the channel-count text next, then refresh and remove-all (trash) icons on the far right — the same glyphs as Add Sources.

#### Scenario: Count sits left of icons
- GIVEN a managed playlist is loaded
- WHEN Playlist Editor is shown
- THEN the `N channels` text is immediately left of the refresh and trash icons
- AND those icons are on the far right of the action row

### Requirement: Video Player selector
The Playlist Editor SHALL show the same **Video Player** combo as Add Sources (`mpv` / `VLC`), bound to `DefaultPlayer` (default mpv). Changing it updates Settings so Play on a variant uses that engine.

#### Scenario: Change player on Playlist Editor
- GIVEN Playlist Editor is shown
- WHEN the operator picks mpv in Video Player
- THEN `DefaultPlayer` is saved
- AND Add Sources and Settings show mpv

### Requirement: Resizable columns
The Playlist Editor SHALL lay out Groups, Channels, and Edit channel as three columns whose widths do not follow row content (long group or channel names wrap/ellipsis). The operator SHALL be able to drag the dividers to resize Groups and Channels the same way as Add Sources, with widths remembered for the session (local storage). The Edit channel pane SHALL keep a stable width when the selected group (category) changes; extra form content (now playing, suggestions, backups) scrolls inside that pane.

#### Scenario: Category change does not resize the editor
- GIVEN a channel is open in Edit channel
- WHEN the operator selects a different group
- THEN the three column widths stay as set
- AND the Edit channel pane does not grow or shrink with the new group's names

#### Scenario: Drag Groups divider
- GIVEN Playlist Editor is shown
- WHEN the operator drags the divider between Groups and Channels
- THEN Groups width follows the pointer (clamped)
- AND the width is restored on the next visit to Playlist Editor

### Requirement: Channel list row
The system SHALL render each managed channel as a two-line row: 40×40 logo (or red broken glyph), name `#EEEEF0`, tvg-id `#AAAAAB`, and a green check (`#32CD32`) when `IsKnownTvgId(tvg-id)` is true. Tooltip on the check: `tvg-id matches EPG catalog`.

#### Scenario: Known id shows check
- GIVEN catalog contains the channel’s tvg-id
- WHEN the list binds
- THEN the green check is visible

#### Scenario: Unknown id hides check
- GIVEN the tvg-id is empty or not in the catalog
- WHEN the list binds
- THEN the check is not shown

### Requirement: Edit fields
The system SHALL provide fields with these headers/placeholders: `Name`; `Group` placeholder `Type a group name…`; `tvg-id (type for EPG suggestions)` placeholder `Start typing a channel id or name…`; `EPG timeshift / tvg-shift (hours vs Eastern-style guide)`; `Logo URL (tvg-logo)`; `Primary stream URL (exported)`; `Notes`.

#### Scenario: Group typeahead
- GIVEN existing managed groups
- WHEN the operator types in Group
- THEN existing group titles are suggested

### Requirement: tvg-id suggestions on keystroke
The system SHALL, on user input in tvg-id, filter the EPG catalog where `TvgId` or `Name` contains the trimmed query (case-insensitive), order starts-with-id first then name A–Z, take 40, and format `{tvg-id}  —  {name}` (two spaces, em dash).

#### Scenario: Pick suggestion
- GIVEN suggestions are open
- WHEN the operator chooses a line
- THEN tvg-id becomes the text before `  —  `
- AND if Name is empty it MAY be filled from the catalog name
- AND match UI updates immediately

#### Scenario: Empty query
- GIVEN the tvg-id box is cleared
- WHEN suggestions refresh
- THEN the suggestion list is empty

### Requirement: Green tvg-id field
The system SHALL set the tvg-id field foreground to `#32CD32` and show the match check when `IsKnownTvgId(typed)` is true; otherwise text `#EEEEF0` and check hidden.

#### Scenario: Keystroke updates match
- GIVEN a known id is typed
- WHEN the last character that completes a catalog id is entered
- THEN the field turns `#32CD32` and the check appears on that keystroke

### Requirement: Now playing and tvg-shift
The system SHALL provide a combo of named zones (Eastern 0 through Tonga +18, including +10.5 India) and a now-playing card using `GetNowPlaying(tvgId, shiftHours)`.

#### Scenario: Programme on air
- GIVEN a known tvg-id with a programme in the shifted window
- WHEN shift or tvg-id changes
- THEN a card styled `#14281A` / `#4CAF50` shows **NOW PLAYING**, title, and local times

#### Scenario: Known but off-air
- GIVEN a known tvg-id with no programme now
- WHEN now-playing refreshes
- THEN the card shows `No programme at this time`

#### Scenario: Unknown id
- GIVEN an unknown tvg-id
- WHEN now-playing would refresh
- THEN the now-playing card is not shown

### Requirement: Logo preview
The system SHALL show a 72×72 logo preview that updates as Logo URL changes; failed load shows the red glyph only.

#### Scenario: Broken preview
- GIVEN a Logo URL that fails to load
- WHEN the preview image errors
- THEN only the red broken-logo glyph is shown
- AND the URL field is left as typed

### Requirement: Stream plus backups list
The system SHALL list variants with per-row **Play**, title + URL, **Info** (origin name / tvg-id), **up/down** reorder (top = exported visible), and **Remove**, plus fields `Add stream URL`, `Label` placeholder `e.g. IPTOR`, and button `Add stream`.

#### Scenario: Reorder promotes export
- GIVEN two variants
- WHEN the operator moves a backup to the top
- THEN that URL becomes the visible exported variant
- AND a swap undo entry is written for that action

#### Scenario: Info dialog
- GIVEN a variant with origin metadata
- WHEN Info is clicked
- THEN origin name and origin tvg-id are shown

### Requirement: Visibility invariant
The system SHALL keep exactly one `visible` variant per managed channel; others are `hidden_backup`.

#### Scenario: Add stream stays hidden
- GIVEN a channel with a visible URL
- WHEN the operator adds a stream URL
- THEN the new variant is `hidden_backup`

### Requirement: Group rename
The system SHALL rename a group via right-click or double-click in-place popup (Enter save, Esc / click away cancel).

#### Scenario: Right-click rename
- GIVEN a group in the list
- WHEN the operator right-clicks and types a new name + Enter
- THEN all channels in that group get the new `group_title`

### Requirement: Empty state
When no curated channels are loaded, Playlist Editor SHALL match Add Sources layout: page title, page text, **Video Player**, then a blank empty panel with **Load curated playlist** and **Create curated playlist**. Create uses loaded sources (picker) when any exist, or an empty list from scratch when none do. After a playlist exists, the same header stays and the action buttons (Load / Add from sources / Export / Refresh / Clear) follow Video Player.

#### Scenario: No curated list
- GIVEN Playlist Editor with zero managed channels
- WHEN the page opens
- THEN the three-column workspace is hidden
- AND Load curated playlist and Create curated playlist are shown

### Requirement: Add channels from sources
The system SHALL offer **Add channels from sources…** filtered by the search box (name / tvg-id).

#### Scenario: Filter in picker
- GIVEN many source channels
- WHEN the operator types in the add-from-sources dialog
- THEN the list filters by name and tvg-id
