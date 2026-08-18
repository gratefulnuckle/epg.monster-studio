# Delta for playback

## Purpose

External playback via mpv or VLC, and location of bundled ffmpeg/ffprobe/mpv. C#: `PlayerService`, `ToolPaths`, `ToolBootstrap`.

## ADDED Requirements

### Requirement: Default player mpv
The system SHALL launch the configured default player when Play is pressed on a source row, editor variant, or audit row.

#### Scenario: mpv args
- GIVEN default player is mpv and `mpv.exe` exists
- WHEN Play is invoked with a URL
- THEN the process is started with `--force-window=yes --keep-open=yes` and the URL
- AND if the source has headers, they are passed as `--http-header-fields` (CRLF joined `Key: Value`)

#### Scenario: VLC args
- GIVEN default player is VLC
- WHEN Play is invoked
- THEN VLC starts with the quoted URL
- AND if a User-Agent header exists it is passed as `:http-user-agent=`

#### Scenario: Missing binary
- GIVEN the configured path does not exist
- WHEN Play is invoked
- THEN an error tells the operator to configure the path in Settings (C# wording)

### Requirement: Bundled tools
The system SHALL look for portable tools under `{app}/tools/mpv/mpv.exe` and `{app}/tools/ffmpeg/ffmpeg.exe` + `ffprobe.exe`, and **Detect bundled tools** fills Settings paths.

#### Scenario: First-run heal
- GIVEN empty path settings and bundled binaries exist
- WHEN settings load
- THEN paths are filled to the bundled locations (C# `LoadSettingsWithDefaults`)

### Requirement: Python engine optional
The system SHALL accept an optional Python path for `engines/` workers (JSON protocol in source `engines/README.md`) without requiring Python for any v1.0-beta critical path.
