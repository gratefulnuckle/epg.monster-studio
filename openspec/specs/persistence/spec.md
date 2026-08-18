# persistence Specification

## Purpose

SQLite workspace, FTS, settings, caches, and legacy folder copy. C#: `SqliteStore`, `AuditProcessStore`, `Bootstrap`.

## Requirements

### Requirement: AppData location
The system SHALL pick a data root from the install folder, not from a v1 tree.

1. Let `app_dir` be the directory that contains the executable (on macOS, the directory that contains the `.app` bundle; for an AppImage, the AppImage's parent directory).
2. If `app_dir` is writable by the current user, data is `{app_dir}/data`.
3. Otherwise data is the OS user data folder: Windows `%LocalAppData%\epg.monster-studio`, Linux `$XDG_DATA_HOME/epg.monster-studio` or `~/.local/share/epg.monster-studio`, macOS `~/Library/Application Support/epg.monster-studio`.

That folder holds `epg.monster-studio.db`, `auditprocess.db`, `logs/`, `logo/`, `offline-slates/`, `cache/`, `tool-cache/`. Schema stays C#-compatible so a **manual** copy of an old DB still opens.

#### Scenario: Portable install
- GIVEN the directory that contains the executable is writable
- WHEN the app resolves its data folder
- THEN that folder is `{app_dir}/data`
- AND the app does not search `%LocalAppData%\epg.monster-studio` or `iptv-studio`

#### Scenario: System install
- GIVEN the directory that contains the executable is not writable
- WHEN the app resolves its data folder
- THEN that folder is the OS user data folder named `epg.monster-studio`

#### Scenario: No legacy copy
- GIVEN `%LocalAppData%\iptv-studio\` exists and the chosen data folder has no database
- WHEN the app first launches
- THEN the legacy folder is left untouched
- AND `iptv-studio.db` is not renamed or opened as the studio database

### Requirement: Main database
The system SHALL open `epg.monster-studio.db` with `PRAGMA journal_mode=WAL` and `foreign_keys=ON`, creating the C# tables if missing and applying the same `EnsureColumn` migrations.

#### Scenario: Open C# database
- GIVEN a database written by the WinUI app
- WHEN this remake starts
- THEN sources, managed channels, variants, catalog, tuner numbers, and settings load without schema error

### Requirement: Schema completeness
The system SHALL include tables `sources`, `channel_entries`, `managed_channels` (with `tvg_shift`, `in_tuner`, `tuner_number`), `stream_variants` (with `origin_name`, `origin_tvg_id`), `audit_results` (with grade/job/error_class/media columns), `swap_undo_log`, `epg_catalog` (with `section`), `epg_programmes`, `epg_now_playing`, `settings`, and FTS5 `channel_fts` on name, group_title, tvg_id, url.

#### Scenario: EnsureColumn migrations
- GIVEN an older C# database missing a later column such as `tvg_shift`
- WHEN the remake opens the file
- THEN the missing columns are added with the same defaults as C# `EnsureColumn`
- AND existing rows remain readable

### Requirement: Audit process database
The system SHALL persist the in-flight Stream Audit job and feed in `auditprocess.db` so pause/resume/crash recovery matches C#.

#### Scenario: Pause survives process exit
- GIVEN a Stream Audit job is paused mid-queue
- WHEN the app exits and is relaunched
- THEN `auditprocess.db` still lists remaining variants
- AND Stream Audit prompts Resume / Start new

### Requirement: Settings keys
The system SHALL persist `AppSettings` with the same property names and defaults as `src/EpgMonsterStudio.Core/Models/AppSettings.cs` (including unused `EpgShareUrl` so old JSON still loads).

#### Scenario: PascalCase JSON
- GIVEN a C# settings blob with `DefaultPlayer`, `IptvTuner`, and `EpgShareUrl`
- WHEN this remake loads settings
- THEN those keys deserialize without rename
- AND unused `EpgShareUrl` is preserved on the next save

### Requirement: Privacy of stored secrets
The system SHALL store playlists, headers, and keys only on this PC under the user profile, never in git, and MUST NOT upload playlists anywhere except the members channels.json (tvg-ids only).

#### Scenario: No keys in logs
- GIVEN a saved access key and a source URL with Authorization
- WHEN a daily log line or crash report is written
- THEN the raw `epgm_` key and provider stream URLs do not appear
