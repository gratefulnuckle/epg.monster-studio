// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use studio_core::models::{ChannelEntry, PlaylistSource};
use studio_core::paths::{app_data_directory, database_path};
use studio_core::player;
use studio_core::store::SqliteStore;
use studio_core::tools::detect_bundled;
use studio_core::{DISPLAY_NAME, VERSION};
use tauri::Manager;

struct AppState {
    store: Mutex<SqliteStore>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioInfoDto {
    version: String,
    display_name: String,
    database_path: String,
    managed_count: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SplashCheck {
    label: String,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceDto {
    id: String,
    name: String,
    kind: String,
    location: String,
    channel_count: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupDto {
    title: String,
    count: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelDto {
    id: String,
    source_id: String,
    group_title: String,
    name: String,
    tvg_id: Option<String>,
    tvg_logo: Option<String>,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UrlSourceArgs {
    url: String,
    name: Option<String>,
    headers: Option<BTreeMap<String, String>>,
}

impl From<PlaylistSource> for SourceDto {
    fn from(s: PlaylistSource) -> Self {
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            location: s.location,
            channel_count: s.channel_count,
        }
    }
}

impl From<ChannelEntry> for ChannelDto {
    fn from(c: ChannelEntry) -> Self {
        Self {
            id: c.id,
            source_id: c.source_id,
            group_title: c.group_title,
            name: c.name,
            tvg_id: c.tvg_id,
            tvg_logo: c.tvg_logo,
            url: c.url,
        }
    }
}

fn lock_store<'a>(state: &'a tauri::State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, SqliteStore>, String> {
    state.store.lock().map_err(|e| e.to_string())
}

fn app_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    app.path()
        .resource_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

#[tauri::command]
fn get_studio_info(state: tauri::State<AppState>) -> Result<StudioInfoDto, String> {
    let store = lock_store(&state)?;
    Ok(StudioInfoDto {
        version: VERSION.to_string(),
        display_name: DISPLAY_NAME.to_string(),
        database_path: database_path().to_string_lossy().into_owned(),
        managed_count: store.managed_count().unwrap_or(0),
    })
}

#[tauri::command]
fn splash_checks(app: tauri::AppHandle, state: tauri::State<AppState>) -> Vec<SplashCheck> {
    let root = app_root(&app);
    let found = detect_bundled(&root);
    let has = |name: &str| found.iter().any(|(n, _)| n == name);
    let store = lock_store(&state).ok();
    let sources = store
        .as_ref()
        .and_then(|s| s.list_sources().ok())
        .map(|s| s.len())
        .unwrap_or(0);
    let managed = store
        .as_ref()
        .and_then(|s| s.managed_count().ok())
        .unwrap_or(0);
    let db = database_path();
    vec![
        SplashCheck {
            label: "ffmpeg".into(),
            ok: has("ffmpeg"),
            detail: if has("ffmpeg") {
                "OK".into()
            } else {
                "not found — Detect bundled tools".into()
            },
        },
        SplashCheck {
            label: "ffprobe".into(),
            ok: has("ffprobe"),
            detail: if has("ffprobe") {
                "OK".into()
            } else {
                "not found".into()
            },
        },
        SplashCheck {
            label: "mpv".into(),
            ok: has("mpv"),
            detail: if has("mpv") {
                "OK".into()
            } else {
                "not found".into()
            },
        },
        SplashCheck {
            label: "Workspace".into(),
            ok: db.exists(),
            detail: format!("{sources} sources · {managed} curated · {}", db.display()),
        },
    ]
}

#[tauri::command]
fn detect_bundled_tools(app: tauri::AppHandle) -> Result<usize, String> {
    Ok(detect_bundled(&app_root(&app)).len())
}

#[tauri::command]
fn list_sources(state: tauri::State<AppState>) -> Result<Vec<SourceDto>, String> {
    Ok(lock_store(&state)?
        .list_sources()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(SourceDto::from)
        .collect())
}

#[tauri::command]
fn list_groups(state: tauri::State<AppState>, source_id: String) -> Result<Vec<GroupDto>, String> {
    Ok(lock_store(&state)?
        .groups_with_counts(&source_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(title, count)| GroupDto { title, count })
        .collect())
}

#[tauri::command]
fn list_channels(
    state: tauri::State<AppState>,
    source_id: String,
    group_title: String,
) -> Result<Vec<ChannelDto>, String> {
    Ok(lock_store(&state)?
        .channels_by_group(&source_id, &group_title)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(ChannelDto::from)
        .collect())
}

#[tauri::command]
fn search_sources(state: tauri::State<AppState>, query: String) -> Result<Vec<ChannelDto>, String> {
    Ok(lock_store(&state)?
        .search_sources(&query)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(ChannelDto::from)
        .collect())
}

#[tauri::command]
fn remove_source(state: tauri::State<AppState>, source_id: String) -> Result<(), String> {
    lock_store(&state)?
        .remove_source(&source_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn pick_source_file(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<Option<SourceDto>, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("Playlists", &["m3u", "m3u8"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let src = lock_store(&state)?
        .add_file_source(&path)
        .map_err(|e| e.to_string())?;
    Ok(Some(src.into()))
}

#[tauri::command]
fn add_source_url(
    state: tauri::State<AppState>,
    args: UrlSourceArgs,
) -> Result<SourceDto, String> {
    let headers = args.headers.unwrap_or_default();
    let cache = app_data_directory().join("cache");
    lock_store(&state)?
        .add_url_source(&args.url, args.name.as_deref(), &headers, &cache)
        .map(SourceDto::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn play_url(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    url: String,
    source_id: Option<String>,
) -> Result<(), String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let headers = source_id.and_then(|id| {
        store
            .list_sources()
            .ok()?
            .into_iter()
            .find(|s| s.id == id)
            .and_then(|s| serde_json::from_str(&s.headers_json).ok())
    });
    player::play(&url, &settings, headers.as_ref(), &app_root(&app))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = database_path();
    let store = SqliteStore::open(&db).expect("open studio database");
    let _ = app_data_directory();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            store: Mutex::new(store),
        })
        .invoke_handler(tauri::generate_handler![
            get_studio_info,
            splash_checks,
            detect_bundled_tools,
            list_sources,
            list_groups,
            list_channels,
            search_sources,
            remove_source,
            pick_source_file,
            add_source_url,
            play_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running epg.monster studio");
}
