# Delta for persistence

## Purpose

SQLite workspace, FTS, settings, caches, and legacy folder copy. C#: `SqliteStore`, `AuditProcessStore`, `Bootstrap`.

## ADDED Requirements

### Requirement: AppData location
The system SHALL use `%LocalAppData%\epg.monster-studio\` as the data root (product id `epg.monster-studio`).

#### Scenario: Legacy copy
- GIVEN `%LocalAppData%\iptv-studio\` exists and the new folder has no database
- WHEN the remake first launches
- THEN the legacy folder is copied into `epg.monster-studio` before open

### Requirement: Main database
The system SHALL open `epg.monster-studio.db` with `PRAGMA journal_mode=WAL` and `foreign_keys=ON`, creating the C# tables if missing and applying the same `EnsureColumn` migrations.

#### Scenario: Open C# database
- GIVEN a database written by the WinUI app
- WHEN this remake starts
- THEN sources, managed channels, variants, catalog, tuner numbers, and settings load without schema error

### Requirement: Schema completeness
The system SHALL include tables `sources`, `channel_entries`, `managed_channels` (with `tvg_shift`, `in_tuner`, `tuner_number`), `stream_variants` (with `origin_name`, `origin_tvg_id`), `audit_results` (with grade/job/error_class/media columns), `swap_undo_log`, `epg_catalog` (with `section`), `epg_programmes`, `epg_now_playing`, `settings`, and FTS5 `channel_fts` on name, group_title, tvg_id, url.

### Requirement: Audit process database
The system SHALL persist the in-flight Stream Audit job and feed in `auditprocess.db` so pause/resume/crash recovery matches C#.

### Requirement: Settings keys
The system SHALL persist `AppSettings` with the same property names and defaults as `src/EpgMonsterStudio.Core/Models/AppSettings.cs` (including unused `EpgShareUrl` so old JSON still loads).

### Requirement: Privacy of stored secrets
The system SHALL store playlists, headers, and keys only on this PC under the user profile, never in git, and MUST NOT upload playlists anywhere except the members channels.json (tvg-ids only).
