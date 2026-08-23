# stream-audit Specification

## Purpose

Serial ffmpeg/ffprobe probes of managed variants, slate/black detection, grades, pause/resume, auto-swap with undo, weekly group slices.

## Requirements

### Requirement: Page chrome
The system SHALL title the page `Stream Audit` and include the shipped intro copy about serial probes, offline slates, persisted results, and `auditprocess.db`.

#### Scenario: Toolbar buttons
- GIVEN Stream Audit is open
- WHEN the toolbar is visible
- THEN it contains these actions: `Start` (all / visible / specific / today's groups), `Pause`, `Resume`, `Cancel`, `Undo last swap`, checkbox `Auto-swap on fail` (default checked), `# Results` on the far right, and a trash icon immediately to its right that clears the current audit

#### Scenario: Clear current audit
- GIVEN Stream Audit has a job or stored `audit_results`
- WHEN the operator clicks the trash icon and confirms
- THEN the job, feed, and `audit_results` rows are discarded
- AND `# Results` shows no count
- AND the page returns to Idle

### Requirement: Strictly serial probes
The system MUST run at most one ffmpeg/ffprobe process globally. Default delay is **6000 ms**, default timeout **15000 ms**.

#### Scenario: No overlap
- GIVEN two queued variants
- WHEN the first probe is still running
- THEN the second process is not started

#### Scenario: Delay
- GIVEN a probe finishes
- WHEN the next starts
- THEN at least `AuditDelayMs` have elapsed (unless cancelled)

### Requirement: Offline slate fail
The system SHALL, after a decode that ffmpeg considers OK, grab a center-crop frame and average-hash it against bundled `offline-slate.png` plus files in `{app}/data/offline-slates/`. A match is a FAIL (`offline slate`) so auto-swap can fire.

#### Scenario: Known offline card
- GIVEN a stream that only shows the known offline still
- WHEN probed
- THEN the result is fail with an offline-slate error class
- AND it is not treated as a healthy visible stream

### Requirement: Optional blackdetect
The system SHALL run ffmpeg blackdetect only when Settings `BlackDetectEnabled` is true, after a passing probe (~5s extra), and fail fully black samples.

#### Scenario: Blackdetect off
- GIVEN `BlackDetectEnabled` is false
- WHEN a probe would otherwise pass
- THEN ffmpeg blackdetect is not started

#### Scenario: Fully black fail
- GIVEN `BlackDetectEnabled` is true and the decoded sample is fully black
- WHEN blackdetect finishes
- THEN the result is fail with a black-screen error class

### Requirement: Grades A–F
The system SHALL assign letter grades and show a 40×40 grade pill on each feed card, plus status chip and latency. Group header rows use `#1E1E2A`.

#### Scenario: Feed persistence
- GIVEN a running or finished job
- WHEN the operator leaves Stream Audit and returns
- THEN the full result list is still shown (memory + `auditprocess.db`)

### Requirement: Pause resume cancel
The system SHALL pause (leave job on disk), resume, and cancel. Unclean exit leaves the job paused; opening Stream Audit prompts **Resume** / **Start new**.

#### Scenario: Resume after crash
- GIVEN `auditprocess.db` has remaining work
- WHEN Stream Audit opens
- THEN the operator is prompted to resume or start new
- AND Start new discards the incomplete job

### Requirement: Auto-swap and undo
The system SHALL, when `Auto-swap on fail` is on, promote a backup after the visible variant fails, write `swap_undo_log`, and support **Undo last swap**.

#### Scenario: Visible fails backup later ok
- GIVEN visible fails and a hidden backup later probes OK
- WHEN auto-swap is enabled
- THEN the backup becomes visible
- AND undo restores the previous visible variant

### Requirement: Pause while playing
The system SHALL pause the queue while an external player is active if Settings `PauseAuditWhilePlaying` is true.

#### Scenario: Player holds the queue
- GIVEN `PauseAuditWhilePlaying` is true and mpv/VLC is running
- WHEN the next probe would start
- THEN the queue stays paused until the player exits

### Requirement: Audit specific channels
The system SHALL open a picker that can select entire groups **and** individual channels, then probe only those (visible only or include hidden backups).

#### Scenario: Picker start
- GIVEN groups and channels checked in Audit specific
- WHEN Start is clicked
- THEN only those selected variants are queued
- AND hidden backups are included only if that checkbox is on

### Requirement: Run today's groups
The system SHALL probe groups assigned to today’s weekday in Settings (includes hidden backups). It MUST NOT auto-start a probe on launch. If `WeeklyAuditAutoRun` is true, it MAY toast a reminder only.

#### Scenario: Skip already run
- GIVEN `WeeklyAuditLastRun` is today’s date
- WHEN the operator does not click again
- THEN today’s groups are not probed automatically

### Requirement: Results window
The system SHALL open `# Results` with a grade graph (A–F), F error-type breakdown, elapsed vs first ETA, and export of the F-list.

#### Scenario: Clocks
- GIVEN a job
- WHEN Results is open
- THEN elapsed time and the first ETA estimate are visible
