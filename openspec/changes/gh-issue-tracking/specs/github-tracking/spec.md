# github-tracking Specification

## Purpose

GitHub issues (via `gh`) are the live tracker for OpenSpec changes and leftover
work. Local markdown is spec or archive, not the board.

## ADDED Requirements

### Requirement: Live board is GitHub

Agents and operators SHALL treat `gh issue list` as the live work board. New
leftover defects SHALL be GitHub issues, not new sections in `ISSUES.md`.

#### Scenario: List v2 work
- GIVEN `gh` is authenticated for `gratefulnuckle/epg.monster-studio`
- WHEN the operator runs `gh issue list --label v2`
- THEN open v2 issues are listed

### Requirement: OpenSpec change maps to issues

Each folder `openspec/changes/<name>/` SHALL have a GitHub epic and one issue
per open checkbox in `tasks.md`, created by `scripts/openspec-gh.ps1` or
`scripts/openspec-gh.sh`. Mapping is stored in `openspec/changes/<name>/github.md`.
A second run MUST NOT create duplicate issues. Checkboxes marked `[x]` SHALL
close their mapped issues on sync (unless `--no-close`).

#### Scenario: Sync a change
- GIVEN `openspec/changes/gh-issue-tracking/tasks.md` has open checkboxes
- WHEN `.\scripts\openspec-gh.ps1 -Change gh-issue-tracking` runs
- THEN an epic issue exists
- AND each open task has an issue number in `github.md`
- AND a second run does not create duplicates

#### Scenario: Close completed tasks
- GIVEN a task line is `- [x]` and `github.md` maps it to an open issue
- WHEN the sync script runs without `--no-close`
- THEN that GitHub issue is closed

### Requirement: No secrets in issues

Issue bodies MUST NOT contain access keys, provider stream URLs, or G-houl
source paths.

#### Scenario: v3 player work
- GIVEN a v3 player task
- WHEN the issue is created
- THEN the body points at `docs/V3.md`
- AND it does not list `./ghoul` file contents
