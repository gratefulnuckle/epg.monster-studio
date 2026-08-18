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

### Requirement: Report UI
The system SHALL show server `report` counts (matched / unknown / received / unique / dups), `unknown[]` tvg-ids, missing-tvg-id count, and `feedUrl`.

### Requirement: Crash issues
The system SHALL `POST /api/member/v1/issues` from the crash window with redacted logs (no keys, no stream URLs).
