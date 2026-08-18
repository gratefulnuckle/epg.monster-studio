# Delta for members-api

## Purpose

Client for my.epg.monster members v1. C#: `MembersApiClient`, `CurationExporter`, `IssueReport`. Server may 404 until members v2; UI must handle that.

## ADDED Requirements

### Requirement: Auth headers
The system SHALL send `Authorization: Bearer {key}` and `X-EPG-Member-Key: {key}` and User-Agent `epg.monster-studio/{version}` on members requests. The raw key MUST never appear in logs or crash payloads.

#### Scenario: Empty key
- GIVEN no key
- WHEN ping is attempted
- THEN the result message is `Paste an access key from my.epg.monster → Keys (starts with epgm_).`
- AND no HTTP call is required

### Requirement: Ping
The system SHALL `GET {base}/api/member/v1/ping` (default base `https://epg.monster`) and store email/username, `feedUrl` / `feedUrlGz`, slug, `limits.maxChannels` (default 2500), `limits.maxBodyBytes` (default 3145728), last ping time.

#### Scenario: Successful ping
- GIVEN a valid `epgm_` key
- WHEN ping returns 200
- THEN email, username, feed URLs, and limits are stored
- AND last ping time is updated
- AND the raw key is not written to the daily log

### Requirement: Publish lineup
The system SHALL build channels.json from every managed channel with a non-empty tvg-id (including unknown ids), PUT to `/api/member/v1/feed/channels`, refuse empty bodies, and honor ping caps.

#### Scenario: No tvg-ids
- GIVEN all managed tvg-ids are empty
- WHEN upload runs
- THEN the message is `No channels with a tvg-id to upload.`
- AND no PUT is sent

#### Scenario: Never send stream URLs
- GIVEN managed channels with URLs
- WHEN the document is built
- THEN the JSON contains tvg-ids (and logo if C# includes it) and MUST NOT contain stream URLs

### Requirement: Job poll
The system SHALL poll `GET /api/member/v1/feed/jobs/latest` when a rebuild is queued until `buildStatus=ready`, and disable the upload button while in flight.

#### Scenario: In-flight disable
- GIVEN a PUT that queued a rebuild
- WHEN the job is not yet `ready`
- THEN Upload channels.json stays disabled
- AND polling continues until ready or a terminal error

### Requirement: Report UI
The system SHALL show server `report` counts (matched / unknown / received / unique / dups), `unknown[]` tvg-ids, missing-tvg-id count, and `feedUrl`.

#### Scenario: Upload report
- GIVEN a successful channels.json PUT with a report body
- WHEN Managed Output / Settings show the result
- THEN matched / unknown / received counts and `feedUrl` are visible
- AND unknown tvg-ids are listed without stream URLs

### Requirement: Crash issues
The system SHALL `POST /api/member/v1/issues` from the crash window with redacted logs (no keys, no stream URLs).

#### Scenario: Redacted crash POST
- GIVEN a pending crash report that mentions an `epgm_` key and an `http://` stream
- WHEN Send report to epg.monster runs
- THEN the POST body replaces keys with `epgm_***` and stream URLs with `http://[redacted]`
- AND a send failure is shown in the dialog without crashing the reporter
