# app-shell Specification

## Purpose

Application chrome: splash, main window, navigation, title-bar search, toasts, tray, About, and crash reporting.

## Requirements

### Requirement: Product identity
The system SHALL present the window title `epg.monster studio` (all lowercase) and use `assets/logo.ico` as the window/taskbar icon.

#### Scenario: Launch title
- GIVEN the app process starts
- WHEN the main window is shown
- THEN the native title is exactly `epg.monster studio`
- AND the icon is the shipped `logo.ico` (not a generated PNG conversion)

### Requirement: Splash then main window
The system SHALL show a splash window on `#0C0C10` with the brand logo, version text, a resource checklist, and XMLTV download progress, then open the main window.

#### Scenario: Splash checklist
- GIVEN a first or subsequent launch
- WHEN splash runs
- THEN it lists tool/EPG checks (ffmpeg, ffprobe, mpv, XMLTV)
- AND XMLTV progress is shown as a percentage
- AND the log line style is `XMLTV begin epg.monster (1/1)` then programme/channel counts
- AND the splash remains at least 5 seconds

#### Scenario: Catalog is epg.monster only
- GIVEN splash fetch
- WHEN guides are downloaded
- THEN only configured XMLTV URLs are fetched
- AND epgshare `.txt` catalogs are not fetched

### Requirement: Navigation items and order
The system SHALL provide a left pane 220px wide with a clickable logo (About) and these items in this order: Add Sources, Playlist Editor, EPG Audit, Logo Audit, Stream Audit, Managed Output, TV Tuner; footer: **Check For Updates**, then Settings. IPTV Player (G-houl) is v3.

#### Scenario: Default page
- GIVEN a fresh session
- WHEN the main window appears
- THEN **Add Sources** is selected

#### Scenario: Logo opens About
- GIVEN the main window
- WHEN the operator clicks the nav logo
- THEN the About dialog opens (license GNU GPL v3.0, 2026 edition, version, links)
- AND the tooltip is `About epg.monster studio`

### Requirement: Check For Updates
The system SHALL show **Check For Updates** in the nav footer above Settings. Opening it queries GitHub Releases for this **public** repo and reports current vs latest without crashing if GitHub is unreachable or has no release yet.

#### Scenario: Check on open
- GIVEN the operator opens Check For Updates
- WHEN the page mounts
- THEN it shows this build (2026 edition + semver) and the latest GitHub tag, or a status line if GitHub is unreachable / has no release
- AND **Open GitHub release** opens the latest release URL in the browser

### Requirement: Title-bar search scope
The system SHALL show a 320px search box with placeholder `Search name, group, tvg-id, URL…` on Add Sources, Playlist Editor, and Managed Output only.

#### Scenario: Hidden on other pages
- GIVEN the operator is on EPG Audit, Logo Audit, Stream Audit, TV Tuner, Check For Updates, or Settings
- WHEN the page is shown
- THEN the title-bar search box does not filter that page (EPG may keep its own in-page filter)

### Requirement: Toasts
The system SHALL show a bottom InfoBar-style toast that auto-closes after 3 seconds and is closable.

#### Scenario: Copy confirmation
- GIVEN the operator copies a URL or tvg-id
- WHEN the copy succeeds
- THEN a success toast appears and dismisses after 3 seconds

### Requirement: Minimize to tray
The system SHALL hide the window to a tray icon on minimize so Stream Audit and tuners keep running.

#### Scenario: Audit continues in tray
- GIVEN Stream Audit is running
- WHEN the operator minimizes the window
- THEN the window hides to the tray
- AND probes continue
- AND the daily log records a tray hide (not a crash)

### Requirement: Crash report on next launch
The system SHALL write a crash report under `{launch}/data/logs/crashes/` and, on the next launch, show a crash report window before the main UI.

#### Scenario: Pending crash
- GIVEN `pending-crash.txt` exists
- WHEN the app launches
- THEN a crash report window is shown
- AND stream URLs and access keys are redacted
- AND the operator can submit via members issues API or dismiss

### Requirement: Daily log and heartbeat
The system SHALL write `{launch}/data/logs/YYYY-MM-DD.log` and emit a `[Watch] heartbeat` line every 5 seconds while running.

#### Scenario: Heartbeat while visible
- GIVEN the main window is open
- WHEN 5 seconds elapse
- THEN the log contains `[Watch] heartbeat` including visibility state
