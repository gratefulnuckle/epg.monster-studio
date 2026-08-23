# epg.monster studio (Tauri) — freeze / crash audit

**Live board is GitHub, not this file.**

```powershell
gh issue list --label v2
gh issue list --label openspec
```

OpenSpec changes: `openspec/changes/<name>/` synced with `.\scripts\openspec-gh.ps1 -Change <name>`.
New leftover work goes on a GitHub issue. Do not append P1 items here.
Freeze P1/P2 items below are archive (GitHub #14 closed). v3 stays in `docs/V3.md`.

Historical freezes from 2026-08-18 are below. **Live leftovers** (after the 2026-08-19 pass):

| Item | Status |
|------|--------|
| GitHub `gratefulnuckle/epg.monster-studio` | **private** until walkthrough |
| NSIS / Authenticode / OS AppData | **v3** — see `docs/V3.md` |
| P1-7 close → tray | by design |
| P1-10 GNU dual RT_MANIFEST | **fixed** — MinGW `default-manifest.o` shadowed so Tauri's is the only RT_MANIFEST |

Need a **new `tauri dev` / NSIS binary** to pick up source fixes. GitHub push is later.

---

---

## This freeze (dev, ~21:57 local)

**Not the 2 GB EPG OOM this time.** PID `7208` (`debug\epg-monster-studio.exe`):

| | |
|--|--|
| RAM | **51 MB** (WebView2 child ~117 MB) |
| Windows | `Responding=False`, title `epg.monster studio` |
| Last good | `OnLaunched` at 02:57:47Z, heartbeats until 02:58:26Z |
| Then | unhandled JS rejection + UI dead |

### Crash that ran during this session

`debug/data/logs/crashes/crash-20260819-025826.txt`

```
TypeError: Cannot set properties of null (setting 'textContent')
    at updateMatch (src/editor.ts)
    at async paintForm
    at async select
```

`updateMatch` does `page.querySelector("#ed-now-title")!.textContent = ...` after `await invoke("is_known_tvg")` / `now_playing`. If the user (or tray `studio-navigate`) leaves Playlist Editor during that await, `shell.ts` `render()` replaces `#page` innerHTML and the nodes are gone. The `!` turns a missing node into an unhandled rejection. Crash hooks log it; WebView can sit **Not Responding** afterward.

A second report at 02:57:53 is a **false unclean-exit** from the previous kill of PID 21364 (the 2.1 GB hang). `showPendingCrash` still opens a modal on the next launch.

### What loaded after splash

Default nav is Add Sources (`shell.ts` `render("audit")`). Live DB:

- 3 sources, **70 349** channel entries (IPTOR 29 947, TDAY 33 621, ASTRA 6 781)
- First group by name: `(US) HBO MAX` (247). Fat groups: **United States 3812**, US Locals 2703, SamsungTV 2431
- **2035** managed channels
- **342 174** EPG programmes, **13 766** catalog ids
- Leftover `debug/data/cache/epg.monster-c3c57b0f.xml` **973 MB** from the earlier hang (still on disk)

`list_channels` has no LIMIT. Virtual lists do not window unless the scroller has a bounded height (see P0-6). Opening Editor calls `list_managed({ group: null })` with every variant URL.

---

## Earlier freeze (dev, ~21:43 local) — still a landmine

PID 21364: **~2.1 GB**, `Responding=False`. `fetch_epg_catalog` wrote a **973 MB** XMLTV file and parsed it on a **sync** Tauri command. Heartbeats stopped after two ticks. Killed.

Cause: `epg_refresh_schedule` 8s after shell. `usable` required `cache_has_xml()`. Copied live DB had programmes in SQLite but no `.xml` in cache → treated as empty → full `https://epg.monster/epg.xml` download.

**Partial patch in tree:** if `only_if_stale && catalog > 0 && programmes > 0`, skip. Does **not** fix splash `fetch_epg_catalog` when `catalog == 0`, or `rebuild_now_playing` reading the 973 MB file, or EPG Audit “Fetch”.

---

## P0 (historical — fixed in source)

### P0-1  Full XMLTV downloaded/parsed on the invoke thread — FIXED

`lib.rs` `fetch_epg_catalog`: `read_to_string` of the whole guide, parse all channels + programmes into `Vec`s, `replace_programmes` row-by-row, **while `lock_store` is held**. No timeout, no size cap.

`epg_refresh_schedule` / splash / EPG Audit “Fetch” all call it. 973 MB XML + copies in RAM = 2.1 GB and a frozen window.

**Fix:** Never fetch when SQLite already has catalog+programmes. Stream parse. `spawn_blocking`. Drop the store lock during HTTP. Delete the leftover `epg.monster-c3c57b0f.xml` or `rebuild_now_playing` will load it next.

### P0-2  Startup mutex deadlock — PATCHED IN SOURCE

`if let Ok(...) = store.lock()... { store.lock() }` kept the guard for the whole block. Nested lock = 1 thread, ~10 MB, no window. Installed NSIS build reproduced this. Guard is dropped before `apply`/`save_settings`. **Reinstall** to get it on Program Files.

### P0-3  NSIS omitted `WebView2Loader.dll` (windows-gnu) — PATCHED IN SOURCE

Vendored `src-tauri/windows/WebView2Loader.dll` + `bundle.resources`. Old setup.exe still installs a broken Program Files tree.

### P0-4  Large m3u add — PARTIAL

Regex is `OnceLock`; inserts are one transaction; add/refresh use `spawn_blocking`. **Still:** lock covers HTTP+parse+insert; after Load, `reload()` paints the first group (P0-6). Tests only 2 500 / 4 000 rows.

### P0-5  Stream Audit holds `store.lock()` across ffmpeg (15s × streams) — FIXED

Editor, EPG, tuner snapshots, add-source wait on the same mutex for the whole probe.

### P0-6  Virtual lists paint every row — FIXED

`virtual.ts` uses `scroller.clientHeight`. `.page { overflow: auto }` and list hosts only have `min-height`, so the page grows and `end === items.length`. Add Sources after splash loads the first group with **no SQL LIMIT**. Editor logos: one `<img src=tvgLogo>` per visible row; if unbounded, thousands of image decodes.

**Fix:** `height: 100%; min-height: 0` on the workspace column; list is the scroller. `list_channels(..., offset, limit)`. After import, show groups only.

### P0-7  Editor `updateMatch` null-deref after navigation — FIXED

`editor.ts` `updateMatch`: `querySelector(...)!` after await. `shell.ts` `render()` destroys the editor DOM. Unhandled rejection; crash modal; WebView can stick **Not Responding**.

**Fix:** Optional chain; abort if `#ed-fields` is gone after await. Don’t use `!` on nodes that can unmount.

---

## P1

### P1-1  Tuner `make_snapshot` loads programmes 1970–2099

Every `/guide.xml` / lineup hit: `list_managed(None)` + all programmes for those tvg-ids. Holds the UI store lock. Window to now−6h…now+36h; cache the snapshot.

### P1-2  ~100 sync `#[tauri::command] fn` handlers

Only add-source/refresh are `spawn_blocking`. Splash, EPG, logo probe, `list_channels`, `blocking_pick_file` all stall `invoke`.

### P1-3  Settings `Kind: 0` vs Rust `kind: String`

Settings deserialize fails → `unwrap_or_default()` → next `save_settings` **overwrites** live tuner profiles.

### P1-4  Four data directories

`tauri dev` → `debug/data`. Release toolchain exe → `release/data`. `dist/*.exe` → `dist/data`. Installed → `%LocalAppData%\epg.monster-studio` (v3).

### P1-5  Vite IPv6 vs WebView2 — PATCHED

`host: "127.0.0.1"` and `devUrl` `http://127.0.0.1:1420`.

### P1-6  Splash blocks heartbeat and promote

Min 5s + 2s + `tools_ensure` + `COUNT(*)` on 342k programmes. Heartbeat starts **after** splash (`main.ts`). Start heartbeat first.

### P1-7  Close/minimize hides to tray

Looks like a crash. Tray mascot easy to miss.

### P1-8  Unpaged IPC: `list_channels`, `list_managed(null)`, `list_catalog`, `epg_audit`, `output_summary`

70k source rows, 13k catalog ids, 2035 managed × variants, all as one JSON blob.

### P1-9  `rebuild_now_playing` `read_to_string`s every `cache/*.xml`

Will OOM on the 973 MB leftover.

### P1-10  GNU `.rsrc merge failure: multiple non-default manifests`

Icon/manifest embedding suspect.

### P1-11  Debug exe ~330 MB vs release ~37 MB

`tauri dev` feels “stuck compiling / not launching” compared to the release exe.

### P1-12  1:1 leftovers

InfoBar vs single toast; crash-before-splash; first-run tool-path heal; verbose tuner logs; `0x47` tune probe.

### P1-13  Add-source still holds the store mutex for HTTP + parse

`spawn_blocking` only leaves the Tokio pool. Other commands still wait.

### P1-14  Editor `list_managed(null)` includes every backup URL

`api.ts` types a slim DTO; the command returns full `ManagedChannel`.

### P1-15  EPG browse dumps every tvg-id as DOM

No virtual list; filter walks the full array per keystroke.

### P1-16  Logo Audit `logo_scan({ probe: true })` on mount

Sequential 15s probes inside a sync command.

### P1-17  Stream Audit polls full feed+queue every 400ms, rebuilds DOM

### P1-18  `tools_ensure` sync on splash (up to 15 min)

### P1-19  Tuner `setInterval(reload, 2000)` never cleared on nav

### P1-20  `showPendingCrash` on every launch after Task Manager / rebuild kill

False “Unexpected shutdown” modal (this session: crash-20260819-025753). Easy to think the new instance is broken.

### P1-21  `pick_source_file` still parses the playlist on the dialog command (sync)

Use `pick_playlist_path` + async `add_source_file` only.

---

## P2

- **P2-1** FTS5 not content-synced; refresh rebuilds all FTS.
- **P2-2** Stale `session.lock` from another product in the same data folder.
- **P2-3** Splash window 560×500 transparent; promote can fail → tiny frameless window.
- **P2-4** Portable exe without `WebView2Loader.dll` next to it fails.
- **P2-5** JS `isVisible()` true for off-screen/transparent windows.
- **P2-6** Release `windows_subsystem = "windows"`: panics have no console (hook logs only). `expect("open studio database")` before the window.

---

## What “release is broken” vs “dev is freezing” actually were

| Symptom | Cause |
|---------|--------|
| Installer runs, exe errors `WebView2Loader.dll` | P0-3 |
| Installer runs, nothing appears | P0-2 deadlock (old binary) |
| Release window tiny / splash stuck | P2-3, P1-6 |
| `tauri dev` 2 GB then Not Responding | P0-1 EPG XML |
| `tauri dev` 51 MB then Not Responding + crash txt | P0-7 editor null + P0-6 list paint; false unclean modal P1-20 |

Do not use `tauri dev` against this 275 MB copied DB until P0-1 and P0-6 are fixed. If you must: delete `debug/data/cache/epg.monster-c3c57b0f.xml`, and do not open Playlist Editor / fat groups until lists are paged.

---

## Status (after leftover pass)

| ID | Status |
|----|--------|
| P0-1 EPG XML OOM | **fixed** — skip fetch when SQLite has catalog+programmes; `spawn_blocking`; drop store lock during HTTP; refuse XML > 32 MB; `rebuild_now_playing` skips if programmes exist / skips huge files |
| P0-2 startup deadlock | **fixed** (reinstall for Program Files) |
| P0-3 WebView2Loader | **fixed** (reinstall) |
| P0-4 large m3u | **fixed** parse/tx/spawn_blocking; first-group paint capped (800) + virtual CSS |
| P0-5 audit mutex | **fixed** — ffmpeg probes run with the store lock dropped |
| P0-6 virtual list CSS | **fixed** — page column + `min-height: 0`; list is the scroller |
| P0-7 editor `updateMatch` null | **fixed** — no `!` after await; abort if Editor unmounted |
| P1-1 snapshot 1970–2099 | **fixed** — `list_programmes_nearby` (−6h / +36h) |
| P1-2 most commands sync | **fixed** — splash/GitHub/members/publish/import/export/logo save/EPG schedule/self-test all `spawn_blocking`; leftover handlers are tiny settings/dialog clicks |
| P1-3 Settings `Kind: 0` | **fixed** — int-or-string deserializer + test |
| P1-4 four data dirs | **v2: one folder** — `{launch}/data` only; OS AppData is v3 |
| P1-5 Vite localhost | **fixed** |
| P1-6 splash vs heartbeat | **fixed** — heartbeat starts before splash |
| P1-7 hide to tray | already toasted when audit runs; tray is by design |
| P1-8 unpaged IPC | **fixed** — `list_channels` limit 800; catalog browse paged 300; Editor / EPG / lineup slim; Managed Output uses visible URL + hidden counts only |
| P1-9 rebuild_now_playing | **fixed** — skip if DB has programmes; skip files > 32 MB |
| P1-10 rsrc merge | **fixed** — GNU uses `WindowsAttributes::new_without_app_manifest()` so rustc's default is the only RT_MANIFEST |
| P1-11 debug exe size | **fixed** — `profile.dev` debug=0 for crates.io deps; line-tables only on workspace crates |
| P1-12 1:1 leftovers | **fixed** — InfoBar-styled toast (title + severity + close); crash dialog before splash; tool-path heal; `0x47` probe |
| P1-13 add-source mutex during HTTP | **fixed** — HTTP + parse off the store lock; insert is 400-row batches |
| P1-14 editor full variants | **fixed** — `list_managed` IPC is slim; `get_managed` hydrates the selected row |
| P1-15 EPG browse DOM | **fixed** — server search + 300 cap |
| P1-16 logo probe stalls app | **fixed** — classify on mount; Scan runs `spawn_blocking` with 4s GET + progress events |
| P1-17 audit 400ms poll | **fixed** — 1.5s + last 80 feed rows |
| P1-18 tools_ensure | **fixed** — `spawn_blocking` |
| P1-19 tuner timer leak | **fixed** — clear on nav |
| P1-20 unclean crash modal | **fixed** — UI skips `unclean` |
| P1-21 pick_source_file parse | **fixed** — browse only |
| P2-1 FTS drift | **fixed** — rebuild `channel_fts` on open when counts mismatch |
| P2-2 stale `session.lock` | **fixed** — ignore locks without `product=epg.monster-studio` |
| P2-3 splash size / promote | **fixed** — promote retries size until ≥960×640 |
| P2-4 portable WebView2 | **fixed** — `bundle.resources` copies `WebView2Loader.dll` |
| P2-5 JS `isVisible()` | **fixed** — heartbeat uses the Rust window, not WebView2 |
| P2-6 poisoned mutex / no-console startup panic | **fixed** — `into_inner()`; DB/window failures use a crash report + MessageBox instead of `expect` |

Needs a **new `tauri dev` / release binary** to pick these up. Startup now **purges XMLTV cache files over 256 MB** (HTTP fetch, gzip inflate, and `rebuild_now_playing` use the same cap). Playlist Editor **Info** is a dialog; group rename is an in-place popup (right-click or double-click). Failed DB open shows a MessageBox instead of a silent `expect` panic.

mpv/VLC stay Settings paths only. Authenticode, NSIS, and G-houl are **v3**.

GitHub push is later.

**Norton / Windows Defender:** unsigned MinGW (`x86_64-pc-windows-gnu`) studio builds are often tagged **Win64.Evo-gen**. Prefer a signed v3 installer, or allow in Norton for local GNU unsigned builds.
