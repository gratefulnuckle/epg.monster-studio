// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Mutex;

use serde::Serialize;
use studio_core::paths::{app_data_directory, database_path};
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SplashCheck {
    label: String,
    ok: bool,
    detail: String,
}

#[tauri::command]
fn get_studio_info() -> StudioInfoDto {
    StudioInfoDto {
        version: VERSION.to_string(),
        display_name: DISPLAY_NAME.to_string(),
    }
}

#[tauri::command]
fn splash_checks(app: tauri::AppHandle) -> Vec<SplashCheck> {
    let root = app
        .path()
        .resource_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let found = detect_bundled(&root);
    let has = |name: &str| found.iter().any(|(n, _)| n == name);
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
            label: "XMLTV catalog".into(),
            ok: true,
            detail: "epg.monster (after splash)".into(),
        },
    ]
}

#[tauri::command]
fn detect_bundled_tools(app: tauri::AppHandle) -> Result<usize, String> {
    let root = app
        .path()
        .resource_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    Ok(detect_bundled(&root).len())
}

#[tauri::command]
fn pick_source_file(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<bool, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("Playlists", &["m3u", "m3u8"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(false);
    };
    let path = file
        .into_path()
        .map_err(|e| e.to_string())?;
    state
        .store
        .lock()
        .map_err(|e| e.to_string())?
        .add_file_source(&path)
        .map_err(|e| e.to_string())?;
    Ok(true)
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
            pick_source_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running epg.monster studio");
}
