# Delta for visual-parity

## Purpose

Exact visual tokens, layout chrome, and copy rules so the remake is indistinguishable from the WinUI 3 studio at a glance. Source: `MainWindow.xaml`, page XAML, `docs/openspec/linux-parity.md` §13.

## ADDED Requirements

### Requirement: Color tokens
The system MUST use these colors (not approximations):

| Token | Hex | Use |
|-------|-----|-----|
| Page background | `#0C0C10` | Window / pages |
| Tile | `#16161E` + 1px stroke | Settings cards, tuner cards |
| Name text | `#EEEEF0` | Channel names |
| tvg-id unmatched | `#AAAAAB` or `#EEEEF0` | List / box |
| tvg-id matched | `#32CD32` | Known catalog id + check |
| Broken logo | `#E57373` | Failed image glyph |
| Now playing card | bg `#14281A`, border `#4CAF50`, label `#81C784` | Editor + EPG |
| Issue orange | `#FF6D00` | Group “N issues” |
| Audit feed | `#12121A` pane, `#1E1E2A` group header, `#1A1A24` result card | Stream Audit |
| Splash / chrome stroke | `#2A2A35` | Splash card border |

#### Scenario: Matched tvg-id is lime
- GIVEN a managed channel whose tvg-id is in the EPG catalog
- WHEN the editor shows that id
- THEN the tvg-id text is `#32CD32` and the match check is visible

#### Scenario: Broken logo is red
- GIVEN a logo URL that fails to load
- WHEN the 40×40 or 72×72 slot renders
- THEN only the `#E57373` broken-image glyph is shown (no stacked failed bitmap)

### Requirement: Settings tile chrome
The system SHALL render Settings as a two-column tile grid, page padding 28/20, tile spacing 16, max content width 1180, title `Settings` and subtitle `epg.monster studio · dark theme · bundled tools`.

#### Scenario: Header buttons
- GIVEN Settings is open
- WHEN the header is visible
- THEN buttons **Detect bundled tools** and **Save** (accent) are top-right

### Requirement: No redesign
The system MUST NOT introduce a new layout language (light theme, different nav order, card-less density, or renamed pages).

#### Scenario: Nav labels unchanged
- GIVEN the left pane
- WHEN the operator reads items
- THEN labels are exactly `Add Sources`, `Playlist Editor`, `EPG Audit`, `Logo Audit`, `Stream Audit`, `Managed Output`, `TV Tuner`, `Settings`

### Requirement: Verbatim copy
The system SHALL use the same visible strings as the matching WinUI XAML, including the ellipsis character `…` (not `...`) where the source uses it.

#### Scenario: Search placeholder
- GIVEN the title-bar search box
- WHEN it is empty
- THEN placeholder text is `Search name, group, tvg-id, URL…`
