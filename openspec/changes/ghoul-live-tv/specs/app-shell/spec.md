# app-shell Specification

## MODIFIED Requirements

### Requirement: Navigation items and order
The system SHALL provide a left pane 220px wide with a clickable logo (About) and
these items in this order: Add Sources, Playlist Editor, EPG Audit, Logo Audit,
Stream Audit, Managed Output, TV Tuner, **IPTV Player**; footer:
**Check For Updates**, then Settings.

#### Scenario: Default page
- GIVEN a fresh session
- WHEN the main window appears
- THEN **Add Sources** is selected

#### Scenario: IPTV Player is listed
- GIVEN the main window
- WHEN the operator reads the nav
- THEN **IPTV Player** appears after **TV Tuner** and before the footer items

#### Scenario: Leaving IPTV Player tears down video
- GIVEN the G-houl guide is mounted on the IPTV Player page
- WHEN the operator clicks any other nav item
- THEN playback stops
- AND the video pane is destroyed before the next page renders
- AND no video surface remains over Playlist Editor

#### Scenario: Logo opens About
- GIVEN the main window
- WHEN the operator clicks the nav logo
- THEN the About dialog opens (license GNU GPL v3.0, 2026 edition, version, links)
- AND the tooltip is `About epg.monster studio`
