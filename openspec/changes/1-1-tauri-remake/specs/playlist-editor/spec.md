# Delta for playlist-editor

## Purpose

Curate managed channels: metadata, logos, tvg-id matching, now-playing, visible stream plus hidden backups. WinUI: `PlaylistEditorPage` — this is the P0 1:1 surface.

## ADDED Requirements

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

### Requirement: Stream plus backups list
The system SHALL list variants with per-row **Play**, title + URL, **Info** (origin name / tvg-id), **up/down** reorder (top = exported visible), and **Remove**, plus fields `Add stream URL`, `Label` placeholder `e.g. IPTOR`, and button `Add stream`.

#### Scenario: Reorder promotes export
- GIVEN two variants
- WHEN the operator moves a backup to the top
- THEN that URL becomes the visible exported variant
- AND a swap undo entry is written if C# does so for that action

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

### Requirement: Add channels from sources
The system SHALL offer **Add channels from sources…** filtered by the search box (name / tvg-id).

#### Scenario: Filter in picker
- GIVEN many source channels
- WHEN the operator types in the add-from-sources dialog
- THEN the list filters by name and tvg-id
