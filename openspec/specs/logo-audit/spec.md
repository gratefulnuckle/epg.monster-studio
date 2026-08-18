# logo-audit Specification

## Purpose

Find missing, invalid, broken, or player-rejected logos; fix them; optionally save a local PNG pack. WinUI: `LogoAuditPage`, `SaveLogosWindow`.

## Requirements

### Requirement: Scan classifications
The system SHALL scan managed logos and classify each issue as missing, invalid, broken (won't load), or player-reject (Wikimedia/SVG/HTML/player-hostile).

#### Scenario: Player-style GET
- GIVEN a logo URL
- WHEN the scanner probes it
- THEN it uses a player-like GET (VLC UA as in C# `LogoPlayerCompat`)
- AND Wikimedia/SVG/WebP-as-unsupported cases are labeled player-reject, not generic broken

### Requirement: Issue list chrome
The system SHALL show each issue row with a 32×32 thumbnail (or red issue icon) and group headers with orange `#FF6D00` issue counts.

#### Scenario: Group count
- GIVEN 3 issues in NEWS
- WHEN the list renders
- THEN the NEWS header shows the issue count in `#FF6D00`

### Requirement: Clear logo
The system SHALL provide **Clear logo**, which clears `tvg-logo` without rebuilding the finder list incorrectly (C# mid-audit fix).

#### Scenario: Clear
- GIVEN a selected channel with a logo
- WHEN Clear logo runs
- THEN `tvg_logo` is empty
- AND the scan can mark it missing on the next scan

### Requirement: Batch set logos
The system SHALL allow multi-select channels plus a group combo to set logos in batch.

#### Scenario: Batch apply URL
- GIVEN several issue channels selected and a logo URL entered
- WHEN Set on selected runs
- THEN each selected channel’s `tvg_logo` becomes that URL
- AND unselected rows are unchanged

### Requirement: Save Logos window
The system SHALL open **Save Logos** as a window that does not start downloads until confirmed, bound to Settings `Logo save directory`.

#### Scenario: Confirm then serial download
- GIVEN N managed channels with logo URLs and non-empty tvg-ids
- WHEN the operator confirms Start
- THEN files download **one at a time**
- AND path is `{root}/{lowercase-sanitized-group}/{tvg-id}.png`
- AND the image is converted to PNG even if the source was JPEG/GIF
- AND empty tvg-id rows are skipped
- AND player-rejected URLs are marked failed
- AND Cancel stops after the current file
- AND Playlist Editor `tvg-logo` is **not** rewritten

#### Scenario: Progress rows
- GIVEN a save run
- WHEN a file finishes
- THEN that numbered row shows pending / saved / failed + reason
