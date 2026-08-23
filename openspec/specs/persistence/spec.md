# persistence Specification

## Purpose

SQLite workspace, FTS, settings, caches, and no legacy folder copy.

## Requirements

### Requirement: AppData location
The system SHALL pick a data root from the install folder, not from a v1 tree.

1. Let `app_dir` be `EPG_MONSTER_HOME` if set; else a source checkout (`package.json` + `src-tauri` in the cwd); else the directory that contains the executable (on macOS, the folder that contains the `.app` bundle).
2. Data is always `{app_dir}/data`. v2 does not use OS AppData (`%LocalAppData%`, XDG, Application Support). Those locations are v3.

That folder holds `epg.monster-studio.db`, `auditprocess.db`, `logs/`, `logo/`, `offline-slates/`, `cache/`, `tool-cache/`. A **manual** copy of an old DB still opens.

#### Scenario: Portable install
- GIVEN the directory that contains the executable is writable
- WHEN the app resolves its data folder
- THEN that folder is `{app_dir}/data`
- AND the app does not search `%LocalAppData%\epg.monster-studio` or `iptv-studio`

#### Scenario: Launch-folder data
- GIVEN `EPG_MONSTER_HOME` or the executable directory
- WHEN the app resolves its data folder
- THEN that folder is `{app_dir}/data`
- AND OS AppData is not used

#### Scenario: No legacy copy
- GIVEN `%LocalAppData%\iptv-studio\` exists and the chosen data folder has no database
- WHEN the app first launches
- THEN the legacy folder is left untouched
- AND `iptv-studio.db` is not renamed or opened as the studio database

### Requirement: Main database
The system SHALL open `epg.monster-studio.db` with `PRAGMA journal_mode=WAL` and `foreign_keys=ON`, creating the tables if missing and applying `EnsureColumn` migrations.

#### Scenario: Open existing database
- GIVEN a database written by an earlier studio build
- WHEN the app starts
- THEN sources, managed channels, variants, catalog, tuner numbers, and settings load without schema error

### Requirement: Schema completeness
The system SHALL include tables `sources`, `channel_entries`, `managed_channels` (with `tvg_shift`, `in_tuner`, `tuner_number`), `stream_variants` (with `origin_name`, `origin_tvg_id`), `audit_results` (with grade/job/error_class/media columns), `swap_undo_log`, `epg_catalog` (with `section`), `epg_programmes`, `epg_now_playing`, `settings`, and FTS5 `channel_fts` on name, group_title, tvg_id, url.

#### Scenario: EnsureColumn migrations
- GIVEN an older database missing a later column such as `tvg_shift`
- WHEN the app opens the file
- THEN the missing columns are added with the same defaults as `EnsureColumn`
- AND existing rows remain readable

### Requirement: Audit process database
The system SHALL persist the in-flight Stream Audit job and feed in `auditprocess.db` so pause/resume/crash recovery still works after relaunch.

#### Scenario: Pause survives process exit
- GIVEN a Stream Audit job is paused mid-queue
- WHEN the app exits and is relaunched
- THEN `auditprocess.db` still lists remaining variants
- AND Stream Audit prompts Resume / Start new

### Requirement: Settings keys
The system SHALL persist `AppSettings` with PascalCase property names (including unused `EpgShareUrl` so old JSON still loads).

#### Scenario: PascalCase JSON
- GIVEN a settings blob with `DefaultPlayer`, `IptvTuner`, and `EpgShareUrl`
- WHEN the app loads settings
- THEN those keys deserialize without rename
- AND unused `EpgShareUrl` is preserved on the next save

### Requirement: Privacy of stored secrets
The system SHALL store playlists, headers, and keys only on this PC under the user profile, never in git, and MUST NOT upload playlists anywhere except the members channels.json (tvg-ids only).

#### Scenario: No keys in logs
- GIVEN a saved access key and a source URL with Authorization
- WHEN a daily log line or crash report is written
- THEN the raw `epgm_` key and provider stream URLs do not appear
