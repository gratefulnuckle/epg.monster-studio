# managed-output Specification

## Purpose

Overview of the curated list, exports, undo, members upload, and tuner lineup numbering.

## Requirements

### Requirement: Columns
The system SHALL list managed channels with columns **Name**, **Group**, **tvg-id**, **Visible URL**, **Variants**, **Audit**.

#### Scenario: Visible URL present
- GIVEN a channel with a visible variant
- WHEN the row renders
- THEN the visible stream URL is shown (truncated is ok; full value remains copyable)

### Requirement: Search
The system SHALL filter this list from the title-bar search box (name, group, tvg-id, URL).

#### Scenario: Title-bar filter
- GIVEN managed rows with distinct names and groups
- WHEN the title-bar search contains a name fragment of at least 2 characters
- THEN only matching Name / Group / tvg-id / URL rows remain

### Requirement: Export visible and all
The system SHALL export a visible-only `.m3u8` and an export-all file that includes backups.

#### Scenario: Visible-only has no hidden URLs
- GIVEN a channel with one visible and one hidden URL
- WHEN export visible runs
- THEN the file contains only the visible URL for that channel

### Requirement: Undo last swap
The system SHALL offer **Undo last swap** using `swap_undo_log`.

#### Scenario: Undo restores previous visible
- GIVEN a swap was written to `swap_undo_log`
- WHEN Undo last swap is clicked
- THEN the previous visible variant is restored
- AND the log entry is consumed

### Requirement: Tuner lineup dialog
The system SHALL open **Tuner lineup…** to pick `in_tuner` channels, auto-number 1…N, allow editing the number box, and swap on collision.

#### Scenario: Auto populate
- GIVEN selected InTuner channels
- WHEN Auto Populate runs
- THEN `tuner_number` becomes 1…N in list order

#### Scenario: Collision swap
- GIVEN channel A is 5 and channel B is 8
- WHEN the operator sets B to 5
- THEN A and B swap numbers

### Requirement: Export and upload channels.json
The system SHALL **Export channels.json** locally and **Upload channels.json** via the members API (every non-empty tvg-id; never stream URLs).

#### Scenario: Upload disabled without key
- GIVEN no access key in Settings
- WHEN Managed Output is shown
- THEN Upload is disabled with a hint to save a key
