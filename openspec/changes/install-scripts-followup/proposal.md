# Proposal: Install-script follow-up

## Why

`studio.ps1` / `studio.sh` broke in tester runs (PATH overflow, UI smash, Node
uninstall miss, rustup stderr). Living spec is `openspec/specs/install-scripts/`.
This change tracks remaining hardening so it is not only chat memory.

## What

Finish `.studio-install.json` (paths + package ids), PATH rebuild without
duplication, tight two-pane UI, winget-only Node/Rust, Scoop only for media.

## Out of scope

NSIS, Authenticode, OS AppData (v3).
