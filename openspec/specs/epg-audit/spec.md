# epg-audit Specification

## Purpose

Match managed tvg-ids and logos against the epg.monster XMLTV catalog. Catalog is `<channel>` tvg-ids in that file — not epgshare `.txt`.

## Requirements

### Requirement: Catalog source
The system SHALL fetch and cache XMLTV from the Settings guide URL(s), default `https://epg.monster/epg.xml` (gz accepted). Empty list means epg.monster only. epgshare hosts are stripped on load.

#### Scenario: Fetch fills catalog
- GIVEN a reachable epg.monster XMLTV
- WHEN fetch completes
- THEN `epg_catalog` rows equal the file’s `<channel>` ids
- AND programmes are indexed for now-playing

### Requirement: Exact and fuzzy match
The system SHALL classify managed channels as exact-catalog, unknown (id present but not in XML), or empty, and offer fuzzy name suggestions (threshold 0.55 for listing; auto-match uses the operator-chosen score).

#### Scenario: Exact keep
- GIVEN a managed tvg-id that exists in the catalog
- WHEN EPG Audit scans
- THEN the row is exact and not overwritten by fuzzy

#### Scenario: Dummy ids
- GIVEN catalog ids named Dummy (or the dummy skip list)
- WHEN auto-apply runs
- THEN those ids are not applied

### Requirement: Inline tvg-id suggest
The system SHALL use the same catalog typeahead as Playlist Editor (contains, prefix-first, cap 40, `{id}  —  {name}`) inside EPG Audit, with green text + check when `IsKnownTvgId`.

#### Scenario: Typeahead format
- GIVEN a catalog id whose name contains the typed query
- WHEN the operator types in the EPG Audit tvg-id box
- THEN suggestions are formatted `{id}  —  {name}`
- AND at most 40 rows are shown
- AND a known id turns the field green with a check

### Requirement: Auto match dialog
The system SHALL provide Auto match with a **score level** combo and **per-group checkboxes** (not a single 0.85-all-groups dialog).

#### Scenario: High-confidence apply
- GIVEN groups selected and a score level
- WHEN the operator confirms Auto match
- THEN only unique suggestions at or above that score in selected groups are applied
- AND logos are not overwritten unless the operator asked to apply logos
- AND ambiguous regional collisions are left untouched

### Requirement: Catalog browse overlay
The system SHALL open a dedicated catalog window (`catalog.html`) that reads `epg_catalog` from SQLite (the same table Playlist Editor tvg-id typeahead uses). It SHALL NOT re-parse XMLTV. Opening Browse catalog SHALL show a loading bar, then virtualized rows (empty filter = first page). Typing in Filter SHALL search `tvg-id` and name.

#### Scenario: Section headers
- GIVEN the catalog has `section` values
- WHEN browse is open
- THEN section header rows use issue-orange styling `#FF6D00`

#### Scenario: Browse loads a list
- GIVEN `epg_catalog` has rows
- WHEN the operator clicks Browse catalog
- THEN the catalog window shows channel rows without requiring a search first

### Requirement: Search images
The system SHALL offer **Search images** (Google transparent image search) for the current channel.

#### Scenario: Google transparent search
- GIVEN a selected channel with a name
- WHEN Search images is clicked
- THEN the default browser opens a Google Images query for that name with transparent-image intent
- AND the managed logo is not changed until the operator pastes and applies a URL

### Requirement: Apply tvg-id and logo
The system SHALL apply a chosen catalog id and/or logo to one channel from the detail pane, and support bulk apply from auto-match.

#### Scenario: Single apply
- GIVEN a suggestion on a selected channel
- WHEN Apply is clicked
- THEN `managed_channels.tvg_id` (and logo if chosen) updates
- AND the editor green-check state is consistent after save
