# Delta for epg-audit

## Purpose

Match managed tvg-ids and logos against the epg.monster XMLTV catalog. WinUI: `EpgMatchPage`. Catalog is `<channel>` tvg-ids in that file — not epgshare `.txt`.

## ADDED Requirements

### Requirement: Catalog source
The system SHALL fetch and cache XMLTV from the Settings guide URL(s), default `https://epg.monster/epg.xml` (gz accepted). Empty list means epg.monster only. epgshare hosts are stripped on load.

#### Scenario: Fetch fills catalog
- GIVEN a reachable epg.monster XMLTV
- WHEN fetch completes
- THEN `epg_catalog` rows equal the file’s `<channel>` ids
- AND programmes are indexed for now-playing

### Requirement: Exact and fuzzy match
The system SHALL classify managed channels as exact-catalog, unknown (id present but not in XML), or empty, and offer fuzzy name suggestions (C# threshold 0.55 for listing; auto-match uses the operator-chosen score).

#### Scenario: Exact keep
- GIVEN a managed tvg-id that exists in the catalog
- WHEN EPG Audit scans
- THEN the row is exact and not overwritten by fuzzy

#### Scenario: Dummy ids
- GIVEN catalog ids named Dummy (or C#’s dummy skip list)
- WHEN auto-apply runs
- THEN those ids are not applied

### Requirement: Inline tvg-id suggest
The system SHALL use the same catalog typeahead as Playlist Editor (contains, prefix-first, cap 40, `{id}  —  {name}`) inside EPG Audit, with green text + check when `IsKnownTvgId`.

### Requirement: Auto match dialog
The system SHALL provide Auto match with a **score level** combo and **per-group checkboxes** (not a single 0.85-all-groups dialog).

#### Scenario: High-confidence apply
- GIVEN groups selected and a score level
- WHEN the operator confirms Auto match
- THEN only unique suggestions at or above that score in selected groups are applied
- AND logos are not overwritten unless the operator asked to apply logos
- AND ambiguous regional collisions are left untouched

### Requirement: Catalog browse overlay
The system SHALL open a full-page catalog browser with orange section headers (UK, US, …) and a back action.

#### Scenario: Section headers
- GIVEN the catalog has `section` values
- WHEN browse is open
- THEN section header rows use issue-orange styling `#FF6D00`

### Requirement: Search images
The system SHALL offer **Search images** (Google transparent image search) for the current channel, matching the WinUI control.

### Requirement: Apply tvg-id and logo
The system SHALL apply a chosen catalog id and/or logo to one channel from the detail pane, and support bulk apply from auto-match.

#### Scenario: Single apply
- GIVEN a suggestion on a selected channel
- WHEN Apply is clicked
- THEN `managed_channels.tvg_id` (and logo if chosen) updates
- AND the editor green-check state is consistent after save
