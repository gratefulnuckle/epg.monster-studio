// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use studio_core::models::{
    ChannelEntry, EpgSuggestion, ManagedChannel, NowPlaying, PlaylistSource, StreamVariant,
};
use studio_core::paths::{
    app_data_directory, crashes_directory, current_log_path, database_path, logs_directory,
    offline_slates_directory,
};
use studio_core::audit;
use studio_core::curation;
use studio_core::epg;
use studio_core::logo;
use studio_core::lineup;
use studio_core::members;
use studio_core::export::{export_all, export_visible_only};
use studio_core::models::{CatalogEntry, EpgAuditRow};
use studio_core::player;
use studio_core::settings::AppSettings;
use studio_core::store::SqliteStore;
use studio_core::tools::{
    default_ffmpeg_path, default_ffprobe_path, default_mpv_path, default_vlc_path, detect_bundled,
};
use studio_core::{
    display_version, github_open_studio_issues, latest_github_tag, DISPLAY_NAME, VERSION,
};
use studio_tuner::manager::{self, TunerManager};
use studio_tuner::host::TunerSnapshot;
use tauri::{Emitter, Manager};
use std::sync::Arc;

struct AppState {
    store: Arc<Mutex<SqliteStore>>,
    audit: Mutex<audit::ProcessStore>,
    tuner: Mutex<TunerManager>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioInfoDto {
    version: String,
    display_version: String,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSourceArgs {
    path: String,
    name: Option<String>,
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

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        match w.is_visible() {
            Ok(true) => {
                let _ = w.hide();
                studio_core::crash::mark_tray_state();
            }
            _ => show_main_window(app),
        }
    }
}

fn lock_store<'a>(state: &'a tauri::State<'a, AppState>) -> Result<std::sync::MutexGuard<'a, SqliteStore>, String> {
    state.store.lock().map_err(|e| e.to_string())
}

fn lock_audit<'a>(
    state: &'a tauri::State<'a, AppState>,
) -> Result<std::sync::MutexGuard<'a, audit::ProcessStore>, String> {
    state.audit.lock().map_err(|e| e.to_string())
}

fn app_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    let hint = app
        .path()
        .resource_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    studio_core::bootstrap::find_app_root(&hint)
}

#[tauri::command]
fn get_studio_info(state: tauri::State<AppState>) -> Result<StudioInfoDto, String> {
    let store = lock_store(&state)?;
    Ok(StudioInfoDto {
        version: VERSION.to_string(),
        display_version: display_version(),
        display_name: DISPLAY_NAME.to_string(),
        database_path: database_path().to_string_lossy().into_owned(),
        managed_count: store.managed_count().unwrap_or(0),
    })
}

fn splash_shorten(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    if s.len() > 52 && (s.contains('\\') || s.contains('/')) {
        return std::path::Path::new(s)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(s)
            .to_string();
    }
    if s.len() <= 56 {
        s.to_string()
    } else {
        format!("…{}", &s[s.len() - 52..])
    }
}

fn splash_check_path(label: &str, path: &std::path::Path, required: bool) -> SplashCheck {
    if path.is_file() {
        SplashCheck {
            label: label.into(),
            ok: true,
            detail: splash_shorten(&path.display().to_string()),
        }
    } else if required {
        SplashCheck {
            label: label.into(),
            ok: false,
            detail: "Not found — set in Settings".into(),
        }
    } else {
        SplashCheck {
            label: label.into(),
            ok: true,
            detail: "Not found (optional)".into(),
        }
    }
}

#[tauri::command]
fn splash_checks(app: tauri::AppHandle, state: tauri::State<AppState>) -> Vec<SplashCheck> {
    let root = app_root(&app);
    let settings = lock_store(&state)
        .ok()
        .and_then(|s| s.load_settings().ok())
        .unwrap_or_default();
    let pick = |stored: &str, fallback: std::path::PathBuf| {
        let t = stored.trim();
        if t.is_empty() {
            fallback
        } else {
            std::path::PathBuf::from(t)
        }
    };
    let data = app_data_directory();
    let db = database_path();
    let cache = data.join("cache");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(&cache);
    vec![
        SplashCheck {
            label: "Application data folder".into(),
            ok: data.is_dir(),
            detail: splash_shorten(&data.display().to_string()),
        },
        SplashCheck {
            label: "SQLite database".into(),
            ok: db.is_file(),
            detail: splash_shorten(&db.display().to_string()),
        },
        splash_check_path(
            "mpv player",
            &pick(&settings.mpv_path, default_mpv_path(&root)),
            true,
        ),
        splash_check_path(
            "ffmpeg (auto-audit)",
            &pick(&settings.ffmpeg_path, default_ffmpeg_path(&root)),
            true,
        ),
        splash_check_path(
            "ffprobe",
            &pick(&settings.ffprobe_path, default_ffprobe_path(&root)),
            false,
        ),
        splash_check_path("VLC (optional)", &pick(&settings.vlc_path, default_vlc_path()), false),
        SplashCheck {
            label: "Playlist cache folder".into(),
            ok: cache.is_dir(),
            detail: splash_shorten(&cache.display().to_string()),
        },
    ]
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SplashEpgStatus {
    catalog: i32,
    programmes: i32,
    cached: bool,
}

#[tauri::command]
fn check_app_update() -> Result<SplashCheck, String> {
    match latest_github_tag() {
        Ok(tag) => {
            let local = VERSION.trim_start_matches('v');
            let remote = tag.trim_start_matches('v');
            if remote == local || tag == VERSION {
                Ok(SplashCheck {
                    label: "Checking github for updates".into(),
                    ok: true,
                    detail: format!("up to date ({tag})"),
                })
            } else {
                Ok(SplashCheck {
                    label: "Checking github for updates".into(),
                    ok: true,
                    detail: format!("update {tag}"),
                })
            }
        }
        Err(e) => Ok(SplashCheck {
            label: "Checking github for updates".into(),
            ok: false,
            detail: splash_shorten(&e),
        }),
    }
}

#[tauri::command]
fn check_github_issues() -> Result<SplashCheck, String> {
    match github_open_studio_issues() {
        Ok((n, title)) => {
            let detail = match (n, title) {
                (0, _) => "0 open".into(),
                (1, Some(t)) => format!("1 open · {t}"),
                (n, Some(t)) => format!("{n} open · {t}"),
                (n, None) => format!("{n} open"),
            };
            Ok(SplashCheck {
                label: "GitHub open issues".into(),
                ok: true,
                detail,
            })
        }
        Err(e) => Ok(SplashCheck {
            label: "GitHub open issues".into(),
            ok: false,
            detail: splash_shorten(&e),
        }),
    }
}

#[tauri::command]
fn splash_epg_status(state: tauri::State<AppState>) -> SplashEpgStatus {
    let store = lock_store(&state).ok();
    let catalog = store
        .as_ref()
        .and_then(|s| s.catalog_count().ok())
        .unwrap_or(0);
    let programmes = store
        .as_ref()
        .and_then(|s| s.programme_count().ok())
        .unwrap_or(0);
    SplashEpgStatus {
        catalog,
        programmes,
        cached: catalog > 0,
    }
}

#[tauri::command]
fn promote_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let w = app
        .get_webview_window("main")
        .ok_or_else(|| "main window missing".to_string())?;
    let _ = w.set_decorations(false);
    let _ = w.set_resizable(true);
    let _ = w.set_minimizable(true);
    let _ = w.set_maximizable(true);
    let _ = w.set_min_size(Some(tauri::LogicalSize::new(960.0, 640.0)));
    let _ = w.set_size(tauri::LogicalSize::new(1400.0, 900.0));
    let _ = w.center();
    apply_window_chrome(&w, false);
    let _ = w.set_focus();
    Ok(())
}

/// Splash: tight DWM rounding, no Win11 light border, transparent WebView2.
/// Main: square window, studio fill, shadow back on.
fn apply_window_chrome(window: &tauri::WebviewWindow, splash: bool) {
    let _ = window.set_shadow(!splash);
    let color = if splash {
        tauri::window::Color(0, 0, 0, 0)
    } else {
        tauri::window::Color(0x0c, 0x0c, 0x10, 0xff)
    };
    let _ = window.set_background_color(Some(color));
    #[cfg(windows)]
    {
        if let Ok(hwnd) = window.hwnd() {
            set_dwm_chrome(hwnd.0 as isize, splash);
        }
    }
}

#[cfg(windows)]
fn set_dwm_chrome(hwnd: isize, splash: bool) {
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWA_BORDER_COLOR: u32 = 34;
    const DWMWCP_DONOTROUND: u32 = 1;
    const DWMWCP_ROUNDSMALL: u32 = 3;
    const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;
    let pref: u32 = if splash {
        DWMWCP_ROUNDSMALL
    } else {
        DWMWCP_DONOTROUND
    };
    let border = DWMWA_COLOR_NONE;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &pref as *const u32 as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border as *const u32 as *const std::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(windows)]
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: isize,
        attr: u32,
        pv: *const std::ffi::c_void,
        cb: u32,
    ) -> i32;
}

#[tauri::command]
fn detect_bundled_tools(app: tauri::AppHandle) -> Result<usize, String> {
    Ok(detect_bundled(&app_root(&app)).len())
}

#[tauri::command]
fn tools_missing(app: tauri::AppHandle) -> Result<Vec<studio_core::bootstrap::ToolSpec>, String> {
    studio_core::bootstrap::missing_tools(&app_root(&app))
}

#[tauri::command]
fn tools_ensure(app: tauri::AppHandle) -> Result<(), String> {
    let root = app_root(&app);
    studio_core::bootstrap::ensure(&root, |p| {
        let _ = app.emit("tools-progress", &p);
    })
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
fn pick_playlist_path(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("Playlists", &["m3u", "m3u8", "txt"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    Ok(Some(
        file.into_path()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned(),
    ))
}

#[tauri::command]
fn add_source_file(
    state: tauri::State<AppState>,
    args: FileSourceArgs,
) -> Result<SourceDto, String> {
    let path = std::path::PathBuf::from(args.path.trim().trim_matches('"'));
    if !path.is_file() {
        return Err("File path (required — browse or paste a valid path)".into());
    }
    lock_store(&state)?
        .add_file_source_named(&path, args.name.as_deref())
        .map(SourceDto::from)
        .map_err(|e| e.to_string())
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

#[tauri::command]
fn refresh_source(state: tauri::State<AppState>, source_id: String) -> Result<SourceDto, String> {
    let cache = app_data_directory().join("cache");
    lock_store(&state)?
        .refresh_source(&source_id, &cache)
        .map(SourceDto::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_backup_from_source(
    state: tauri::State<AppState>,
    managed_id: String,
    entry_id: String,
) -> Result<String, String> {
    lock_store(&state)?
        .add_backup_from_entry(&managed_id, &entry_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn managed_count(state: tauri::State<AppState>) -> Result<i32, String> {
    lock_store(&state)?.managed_count().map_err(|e| e.to_string())
}

#[tauri::command]
fn list_managed_groups(state: tauri::State<AppState>) -> Result<Vec<GroupDto>, String> {
    Ok(lock_store(&state)?
        .managed_groups()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(title, count)| GroupDto { title, count })
        .collect())
}

#[tauri::command]
fn list_managed(
    state: tauri::State<AppState>,
    group: Option<String>,
) -> Result<Vec<ManagedChannel>, String> {
    lock_store(&state)?
        .list_managed(group.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_managed(state: tauri::State<AppState>, id: String) -> Result<Option<ManagedChannel>, String> {
    lock_store(&state)?.get_managed(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_managed(state: tauri::State<AppState>, channel: ManagedChannel) -> Result<(), String> {
    lock_store(&state)?
        .upsert_managed(&channel)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_managed(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    lock_store(&state)?.delete_managed(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_managed_group(
    state: tauri::State<AppState>,
    old_name: String,
    new_name: String,
) -> Result<i32, String> {
    lock_store(&state)?
        .rename_managed_group(&old_name, &new_name)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_stream(
    state: tauri::State<AppState>,
    managed_id: String,
    url: String,
    label: Option<String>,
) -> Result<StreamVariant, String> {
    lock_store(&state)?
        .add_stream(&managed_id, &url, label.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_variant(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    lock_store(&state)?.delete_variant(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn move_variant(
    state: tauri::State<AppState>,
    managed_id: String,
    variant_id: String,
    delta: i32,
) -> Result<(), String> {
    let store = lock_store(&state)?;
    let vars = store.get_variants(&managed_id).map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = vars.into_iter().map(|v| v.id).collect();
    let i = ids.iter().position(|id| id == &variant_id).ok_or("stream not found")?;
    let j = i as i32 + delta;
    if j < 0 || j as usize >= ids.len() {
        return Ok(());
    }
    ids.swap(i, j as usize);
    store
        .apply_variant_order(&managed_id, &ids)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn suggest_tvg(state: tauri::State<AppState>, query: String) -> Result<Vec<EpgSuggestion>, String> {
    lock_store(&state)?.suggest_tvg(&query).map_err(|e| e.to_string())
}

#[tauri::command]
fn now_playing(
    state: tauri::State<AppState>,
    tvg_id: String,
    shift_hours: f64,
) -> Result<Option<NowPlaying>, String> {
    lock_store(&state)?
        .now_playing(&tvg_id, shift_hours)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn is_known_tvg(state: tauri::State<AppState>, tvg_id: String) -> Result<bool, String> {
    Ok(lock_store(&state)?.is_known_tvg_id(Some(&tvg_id)))
}

#[tauri::command]
fn add_from_source(state: tauri::State<AppState>, entry_id: String) -> Result<ManagedChannel, String> {
    lock_store(&state)?
        .add_from_source_entry(&entry_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn import_curated(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    replace: bool,
) -> Result<String, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("Playlists", &["m3u", "m3u8"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok("cancelled".into());
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let label = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("curated");
    let (added, skipped) = lock_store(&state)?
        .import_curated(&content, replace, label)
        .map_err(|e| e.to_string())?;
    Ok(format!("Imported {added}, skipped {skipped} already present."))
}

#[tauri::command]
fn export_managed(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    include_backups: bool,
) -> Result<String, String> {
    let name = if include_backups {
        "epg.monster-studio-output-all"
    } else {
        "epg.monster-studio-output"
    };
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("M3U8 playlist", &["m3u8", "m3u"])
        .set_file_name(name)
        .blocking_save_file();
    let Some(file) = picked else {
        return Ok("cancelled".into());
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    let channels = lock_store(&state)?
        .list_managed(None)
        .map_err(|e| e.to_string())?;
    let body = if include_backups {
        export_all(&channels)
    } else {
        export_visible_only(&channels)
    };
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(if include_backups {
        "Exported visible streams and backups".into()
    } else {
        "Exported visible streams".into()
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputRow {
    id: String,
    name: String,
    group: String,
    tvg_id: String,
    visible_url: String,
    variants_summary: String,
    audit_status: String,
    in_tuner: bool,
    tuner_number: Option<i32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputSummary {
    rows: Vec<OutputRow>,
    recent_swaps: i32,
    tuner_count: i32,
    enabled_tuners: i32,
    has_key: bool,
}

#[tauri::command]
fn output_summary(state: tauri::State<AppState>, filter: Option<String>) -> Result<OutputSummary, String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let mut channels = store.list_managed(None).map_err(|e| e.to_string())?;
    if let Some(q) = filter.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let ql = q.to_ascii_lowercase();
        channels.retain(|c| {
            c.name.to_ascii_lowercase().contains(&ql)
                || c.group_title.to_ascii_lowercase().contains(&ql)
                || c.tvg_id
                    .as_deref()
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .contains(&ql)
                || c.variants
                    .iter()
                    .find(|v| v.visibility == "visible")
                    .or_else(|| c.variants.first())
                    .map(|v| v.url.to_ascii_lowercase().contains(&ql))
                    .unwrap_or(false)
        });
    }
    let tuner_count = channels.iter().filter(|c| c.in_tuner).count() as i32;
    let rows = channels
        .into_iter()
        .map(|c| {
            let vis = c
                .variants
                .iter()
                .find(|v| v.visibility == "visible")
                .or_else(|| c.variants.first());
            let hidden = c
                .variants
                .iter()
                .filter(|v| v.visibility == "hidden_backup")
                .count();
            let audit = match vis.and_then(|v| v.last_audit_ok) {
                None => "Unknown",
                Some(true) => "OK",
                Some(false) => "Fail",
            };
            OutputRow {
                id: c.id,
                name: c.name,
                group: c.group_title,
                tvg_id: c.tvg_id.unwrap_or_default(),
                visible_url: vis
                    .map(|v| v.url.clone())
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| "(none)".into()),
                variants_summary: format!("1 vis + {hidden} hid"),
                audit_status: audit.into(),
                in_tuner: c.in_tuner,
                tuner_number: c.tuner_number,
            }
        })
        .collect();
    Ok(OutputSummary {
        rows,
        recent_swaps: store.pending_swap_count(5).unwrap_or(0),
        tuner_count,
        enabled_tuners: settings.enabled_tuner_count(),
        has_key: !settings.member_access_key.trim().is_empty(),
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TunerPickRow {
    id: String,
    name: String,
    group: String,
    included: bool,
    number: Option<i32>,
}

#[tauri::command]
fn lineup_candidates(state: tauri::State<AppState>) -> Result<Vec<TunerPickRow>, String> {
    let channels = lock_store(&state)?
        .list_managed(None)
        .map_err(|e| e.to_string())?;
    if channels.is_empty() {
        return Err("Load a playlist in Playlist Editor first".into());
    }
    Ok(lineup::playlist_order(&channels)
        .into_iter()
        .map(|c| TunerPickRow {
            id: c.id,
            name: c.name,
            group: c.group_title,
            included: c.in_tuner,
            number: c.tuner_number,
        })
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TunerPick {
    id: String,
    included: bool,
    number: Option<i32>,
}

#[tauri::command]
fn save_tuner_lineup(state: tauri::State<AppState>, picks: Vec<TunerPick>) -> Result<String, String> {
    let store = lock_store(&state)?;
    let mut channels = store.list_managed(None).map_err(|e| e.to_string())?;
    if channels.is_empty() {
        return Err("Load a playlist in Playlist Editor first".into());
    }
    let ids: Vec<String> = picks
        .iter()
        .filter(|p| p.included)
        .map(|p| p.id.clone())
        .collect();
    lineup::include(&mut channels, &ids);
    for p in picks.iter().filter(|p| p.included) {
        if let Some(n) = p.number.filter(|n| *n > 0) {
            lineup::assign_number(&mut channels, &p.id, n).map_err(|e| e.to_string())?;
        }
    }
    for ch in &channels {
        store.upsert_managed(ch).map_err(|e| e.to_string())?;
    }
    let on_tuner = channels.iter().filter(|c| c.in_tuner).count();
    let mut msg = format!("{on_tuner} channel(s) on tuner, ordered by playlist group");
    if on_tuner > 400 {
        msg.push_str(" — Plex cannot save a mapping this large (it puts every channel in the URL). Keep the Plex lineup under ~400, or use the IPTV card.");
    }
    Ok(msg)
}

#[tauri::command]
fn export_channels_json(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<String, String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let channels = store.list_managed(None).map_err(|e| e.to_string())?;
    let cap = if settings.member_max_channels > 0 {
        settings.member_max_channels
    } else {
        members::DEFAULT_MAX_CHANNELS
    };
    let built = curation::build(&channels, VERSION, None, Some(cap));
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("channels.json", &["json"])
        .set_file_name("channels")
        .blocking_save_file();
    let Some(file) = picked else {
        return Ok("cancelled".into());
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, curation::to_json(&built.document)).map_err(|e| e.to_string())?;
    Ok(format!(
        "channels.json · {} unique tvg-id · {} empty skipped · {} dups",
        built.included, built.skipped_no_tvg_id, built.skipped_duplicate
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishReport {
    ok: bool,
    text: String,
}

#[tauri::command]
fn publish_channels(state: tauri::State<AppState>) -> Result<PublishReport, String> {
    let store = lock_store(&state)?;
    let mut settings = store.load_settings().map_err(|e| e.to_string())?;
    let key = settings.member_access_key.trim().to_string();
    if key.is_empty() {
        return Err("Add your my.epg.monster access key in Settings first.".into());
    }
    let channels = store.list_managed(None).map_err(|e| e.to_string())?;
    drop(store);
    let (built, result) = members::publish_lineup(
        &settings.member_api_base,
        &key,
        &channels,
        VERSION,
    );
    if result.ok {
        if let Some(u) = result.feed_url.as_deref().filter(|s| !s.is_empty()) {
            settings.member_feed_url = u.to_string();
        }
        if let Some(u) = result.feed_url_gz.as_deref().filter(|s| !s.is_empty()) {
            settings.member_feed_url_gz = u.to_string();
        }
        if let Some(n) = result.max_channels.filter(|n| *n > 0) {
            settings.member_max_channels = n;
        }
        if let Some(n) = result.max_body_bytes.filter(|n| *n > 0) {
            settings.member_max_body_bytes = n;
        }
        settings.member_last_published_at = members_now();
        lock_store(&state)?
            .save_settings(&settings)
            .map_err(|e| e.to_string())?;
    }
    Ok(PublishReport {
        ok: result.ok,
        text: members::format_publish_report(&built, &result),
    })
}

fn members_now() -> String {
    time_utc_compact()
}

fn time_utc_compact() -> String {
    let n = studio_core::audit::now_iso();
    if let Some((d, rest)) = n.split_once('T') {
        let t = rest.trim_end_matches('Z');
        let t = t.get(..8).unwrap_or(t);
        format!("{d}T{t}Z")
    } else {
        n
    }
}

#[tauri::command]
fn clear_managed(state: tauri::State<AppState>) -> Result<(), String> {
    lock_store(&state)?.clear_managed().map_err(|e| e.to_string())
}

#[tauri::command]
fn load_settings(state: tauri::State<AppState>) -> Result<AppSettings, String> {
    lock_store(&state)?.load_settings().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(state: tauri::State<AppState>, settings: AppSettings) -> Result<(), String> {
    lock_store(&state)?
        .save_settings(&settings)
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolPathsDto {
    mpv: String,
    vlc: String,
    ffmpeg: String,
    ffprobe: String,
}

#[tauri::command]
fn detect_tool_paths(app: tauri::AppHandle) -> ToolPathsDto {
    let root = app_root(&app);
    ToolPathsDto {
        mpv: default_mpv_path(&root).to_string_lossy().into_owned(),
        vlc: default_vlc_path().to_string_lossy().into_owned(),
        ffmpeg: default_ffmpeg_path(&root).to_string_lossy().into_owned(),
        ffprobe: default_ffprobe_path(&root).to_string_lossy().into_owned(),
    }
}

#[tauri::command]
fn members_ping(api_base: String, access_key: String) -> members::MemberPingResult {
    members::ping(&api_base, &access_key, Some(VERSION))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsFolders {
    logs: String,
    crashes: String,
    slates: String,
    current_log: String,
    logo_dir: String,
}

#[tauri::command]
fn settings_folders() -> SettingsFolders {
    SettingsFolders {
        logs: logs_directory().to_string_lossy().into_owned(),
        crashes: crashes_directory().to_string_lossy().into_owned(),
        slates: offline_slates_directory().to_string_lossy().into_owned(),
        current_log: current_log_path().to_string_lossy().into_owned(),
        logo_dir: logo::default_logo_dir().to_string_lossy().into_owned(),
    }
}

#[tauri::command]
fn list_slates() -> Vec<String> {
    let dir = offline_slates_directory();
    let mut names = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
                if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                    names.push(n.to_string());
                }
            }
        }
    }
    names.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    names
}

#[tauri::command]
fn add_slate(app: tauri::AppHandle) -> Result<String, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("Images", &["png", "jpg", "jpeg"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok("cancelled".into());
    };
    let src = file.into_path().map_err(|e| e.to_string())?;
    let name = src
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid file".to_string())?;
    let dest = offline_slates_directory().join(name);
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    Ok(name.to_string())
}

#[tauri::command]
fn remove_slate(name: String) -> Result<(), String> {
    let dest = offline_slates_directory().join(name);
    if dest.is_file() {
        std::fs::remove_file(dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_folder(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(path);
    if let Some(dir) = if p.is_dir() {
        Some(p.clone())
    } else {
        p.parent().map(|d| d.to_path_buf())
    } {
        let _ = std::fs::create_dir_all(&dir);
        #[cfg(windows)]
        {
            std::process::Command::new("explorer")
                .arg(dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(not(windows))]
        {
            std::process::Command::new("xdg-open")
                .arg(dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn consume_pending_crash() -> Option<studio_core::crash::CrashReport> {
    studio_core::crash::consume_pending_crash()
}

#[tauri::command]
fn write_crash_report(
    kind: String,
    title: String,
    summary: String,
    details: String,
) -> studio_core::crash::CrashReport {
    studio_core::crash::append_log("Fatal", "CrashGuard", &title);
    studio_core::crash::write_crash_report(&kind, &title, &summary, &details, "Exception")
}

#[tauri::command]
fn log_heartbeat(visible: bool, tray: bool) {
    studio_core::crash::append_log(
        "Trace",
        "Watch",
        &format!("heartbeat visible={visible} tray={tray}"),
    );
}

#[tauri::command]
fn mark_tray_state() {
    studio_core::crash::mark_tray_state();
}

#[tauri::command]
fn mark_clean_exit() {
    studio_core::crash::mark_clean_exit();
}

#[tauri::command]
fn post_issue(
    state: tauri::State<AppState>,
    kind: String,
    title: String,
    summary: String,
    details: String,
    notes: Option<String>,
) -> Result<members::MemberIssueResult, String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let key = settings.member_access_key.trim().to_string();
    if key.is_empty() {
        return Ok(members::MemberIssueResult {
            message: "Add your access key in Settings → my.epg.monster first.".into(),
            ..members::MemberIssueResult::default()
        });
    }
    let count = store.managed_count().ok();
    drop(store);
    let slug = settings
        .member_feed_url
        .rsplit('/')
        .next()
        .map(|s| s.trim_end_matches(".gz").to_string())
        .filter(|s| !s.is_empty());
    let user = if !settings.member_username.trim().is_empty() {
        settings.member_username.clone()
    } else {
        settings.member_email.clone()
    };
    let payload = studio_core::issue::build(
        &kind,
        &title,
        Some(&summary),
        Some(&details),
        VERSION,
        slug.as_deref(),
        count,
        notes.as_deref(),
        Some(&user),
    );
    let base = settings.member_api_base.clone();
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        members::post_issue(&base, &key, &payload, Some(VERSION))
    })) {
        Ok(r) => Ok(r),
        Err(_) => Err("Send report failed (internal error).".into()),
    }
}

#[tauri::command]
fn epg_catalog_count(state: tauri::State<AppState>) -> Result<i32, String> {
    lock_store(&state)?.catalog_count().map_err(|e| e.to_string())
}

#[tauri::command]
fn epg_guide_url(state: tauri::State<AppState>) -> Result<String, String> {
    let s = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    Ok(epg::resolve_xml_urls(&s)
        .into_iter()
        .next()
        .unwrap_or_else(|| epg::DEFAULT_XML_URL.into()))
}

#[tauri::command]
fn fetch_epg_catalog(state: tauri::State<AppState>, url: Option<String>) -> Result<String, String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let urls = if let Some(u) = url.filter(|s| !s.trim().is_empty() && !epg::is_epgshare_url(s)) {
        vec![u]
    } else {
        epg::resolve_xml_urls(&settings)
    };
    let cache = app_data_directory().join("cache");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let mut all_ch = Vec::new();
    let mut all_prog = Vec::new();
    let mut etag = None;
    let mut last_mod = None;
    for u in &urls {
        let got = epg::fetch_xmltv_conditional(u, None, None)?;
        let epg::FetchXmltv::Body {
            bytes,
            etag: e,
            last_modified: lm,
        } = got
        else {
            continue;
        };
        if etag.is_none() {
            etag = e;
            last_mod = lm;
        }
        let host = u
            .split("://")
            .nth(1)
            .unwrap_or(u)
            .split('/')
            .next()
            .unwrap_or("epg.monster")
            .to_string();
        let stamp = format!("{:x}", u.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32)));
        let path = cache.join(format!("{host}-{stamp}.xml"));
        epg::materialize_xmltv(&bytes, &path).map_err(|e| e.to_string())?;
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        all_ch.extend(epg::parse_xmltv_channels(&text, &host));
        all_prog.extend(epg::index_programmes_from_xml(&text));
    }
    let mut seen = std::collections::HashSet::new();
    all_ch.retain(|c| seen.insert(c.tvg_id.to_ascii_lowercase()));
    store.replace_epg_catalog(&all_ch).map_err(|e| e.to_string())?;
    store.replace_programmes(&all_prog).map_err(|e| e.to_string())?;
    store
        .touch_epg_cache_meta(true, true, etag.as_deref(), last_mod.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "{} catalog ids · {} programmes indexed",
        all_ch.len(),
        all_prog.len()
    ))
}

#[tauri::command]
fn rebuild_now_playing(state: tauri::State<AppState>) -> Result<String, String> {
    let cache = app_data_directory().join("cache");
    let mut all = Vec::new();
    if cache.is_dir() {
        for f in std::fs::read_dir(&cache).map_err(|e| e.to_string())? {
            let f = f.map_err(|e| e.to_string())?.path();
            if f.extension().and_then(|e| e.to_str()) == Some("xml") {
                let text = std::fs::read_to_string(&f).map_err(|e| e.to_string())?;
                all.extend(epg::index_programmes_from_xml(&text));
            }
        }
    }
    let store = lock_store(&state)?;
    store.replace_programmes(&all).map_err(|e| e.to_string())?;
    store
        .touch_epg_cache_meta(false, true, None, None)
        .map_err(|e| e.to_string())?;
    Ok(format!("Reindexed {} programmes from cache", all.len()))
}

fn cache_has_xml() -> bool {
    let cache = app_data_directory().join("cache");
    let Ok(rd) = std::fs::read_dir(cache) else {
        return false;
    };
    rd.flatten()
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("xml"))
}

#[tauri::command]
fn epg_refresh_schedule(
    state: tauri::State<AppState>,
    only_if_stale: bool,
) -> Result<String, String> {
    let store = lock_store(&state)?;
    let catalog = store.catalog_count().unwrap_or(0);
    let programmes = store.programme_count().unwrap_or(0);
    let usable = catalog > 0 && programmes > 0 && cache_has_xml();
    let meta = store.load_epg_cache_meta();
    if only_if_stale && usable && meta.index_is_fresh(epg::REFRESH_INTERVAL_SECS) {
        return Ok("skipped".into());
    }
    if !usable {
        drop(store);
        fetch_epg_catalog(state, None)?;
        return Ok("downloaded".into());
    }
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let url = epg::resolve_xml_urls(&settings)
        .into_iter()
        .next()
        .unwrap_or_else(|| epg::DEFAULT_XML_URL.into());
    let etag = meta.etag.clone();
    let last_mod = meta.last_modified.clone();
    drop(store);
    match epg::fetch_xmltv_conditional(&url, etag.as_deref(), last_mod.as_deref()) {
        Ok(epg::FetchXmltv::NotModified) => {
            rebuild_now_playing(state)?;
            Ok("reindexed".into())
        }
        Ok(epg::FetchXmltv::Body { .. }) => {
            fetch_epg_catalog(state, None)?;
            Ok("downloaded".into())
        }
        Err(_) => {
            rebuild_now_playing(state)?;
            Ok("reindexed".into())
        }
    }
}

#[tauri::command]
fn epg_audit(state: tauri::State<AppState>) -> Result<Vec<EpgAuditRow>, String> {
    let store = lock_store(&state)?;
    let channels = store.list_managed(None).map_err(|e| e.to_string())?;
    let catalog = store.list_catalog().map_err(|e| e.to_string())?;
    Ok(epg::build_epg_audit(&channels, &catalog))
}

#[tauri::command]
fn epg_apply(
    state: tauri::State<AppState>,
    managed_id: String,
    tvg_id: String,
    logo: Option<String>,
    apply_logo: bool,
) -> Result<(), String> {
    let store = lock_store(&state)?;
    let Some(mut ch) = store.get_managed(&managed_id).map_err(|e| e.to_string())? else {
        return Err("channel not found".into());
    };
    ch.tvg_id = Some(tvg_id);
    if apply_logo {
        if let Some(l) = logo.filter(|s| !s.is_empty()) {
            ch.tvg_logo = Some(l);
        }
    }
    store.upsert_managed(&ch).map_err(|e| e.to_string())
}

#[tauri::command]
fn epg_auto_match(
    state: tauri::State<AppState>,
    groups: Vec<String>,
    min_score: f64,
) -> Result<i32, String> {
    let store = lock_store(&state)?;
    let channels = store.list_managed(None).map_err(|e| e.to_string())?;
    let catalog = store.list_catalog().map_err(|e| e.to_string())?;
    let rows = epg::build_epg_audit(&channels, &catalog);
    let want: std::collections::HashSet<String> = groups
        .into_iter()
        .map(|g| g.trim().to_ascii_lowercase())
        .collect();
    let mut applied = 0;
    for row in rows {
        if !want.contains(&row.group_title.trim().to_ascii_lowercase()) {
            continue;
        }
        if !epg::should_auto_apply(&row, min_score, true) {
            continue;
        }
        if !store.is_known_tvg_id(row.suggested_tvg_id.as_deref()) {
            continue;
        }
        if let Some(mut ch) = store.get_managed(&row.managed_channel_id).map_err(|e| e.to_string())? {
            ch.tvg_id = row.suggested_tvg_id;
            store.upsert_managed(&ch).map_err(|e| e.to_string())?;
            applied += 1;
        }
    }
    Ok(applied)
}

#[tauri::command]
fn epg_browse_catalog(state: tauri::State<AppState>) -> Result<Vec<CatalogEntry>, String> {
    lock_store(&state)?.list_catalog().map_err(|e| e.to_string())
}

#[tauri::command]
fn logo_scan(state: tauri::State<AppState>, probe: bool) -> Result<Vec<logo::LogoIssue>, String> {
    let channels = {
        let store = lock_store(&state)?;
        store.list_managed(None).map_err(|e| e.to_string())?
    };
    let mut out = Vec::new();
    for ch in channels {
        let mut issue = logo::classify_channel(&ch);
        if probe && issue.issue.is_empty() {
            if let Some(url) = ch.tvg_logo.as_deref() {
                let check = logo::probe_url(url);
                if !check.is_ok() {
                    issue.issue = check
                        .issue
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "broken".into());
                    issue.reason = check.reason;
                }
            }
        }
        out.push(issue);
    }
    logo::sort_issues(&mut out);
    Ok(out)
}

fn reject_logo_url(url: &str) -> Result<(), String> {
    let check = logo::classify_url(url);
    if !check.is_ok() {
        return Err(if check.reason.is_empty() {
            "Logo URL must be http(s)".into()
        } else {
            check.reason
        });
    }
    Ok(())
}

#[tauri::command]
fn logo_set(
    state: tauri::State<AppState>,
    managed_id: String,
    url: Option<String>,
) -> Result<(), String> {
    let trimmed = url.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if let Some(u) = trimmed {
        reject_logo_url(u)?;
    }
    let store = lock_store(&state)?;
    let Some(mut ch) = store.get_managed(&managed_id).map_err(|e| e.to_string())? else {
        return Err("channel not found".into());
    };
    ch.tvg_logo = trimmed.map(|s| s.to_string());
    store.upsert_managed(&ch).map_err(|e| e.to_string())
}

#[tauri::command]
fn logo_batch_set(
    state: tauri::State<AppState>,
    ids: Vec<String>,
    url: String,
) -> Result<i32, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("Paste a logo URL first".into());
    }
    reject_logo_url(&url)?;
    let store = lock_store(&state)?;
    let mut n = 0;
    for id in ids {
        if let Some(mut ch) = store.get_managed(&id).map_err(|e| e.to_string())? {
            ch.tvg_logo = Some(url.clone());
            store.upsert_managed(&ch).map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    Ok(n)
}

#[tauri::command]
fn logo_default_dir() -> String {
    logo::default_logo_dir().to_string_lossy().into_owned()
}

#[tauri::command]
fn logo_save_plan(
    state: tauri::State<AppState>,
    root: Option<String>,
) -> Result<(String, Vec<logo::LogoSaveItem>), String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let dir = root
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            if settings.logo_save_directory.trim().is_empty() {
                logo::default_logo_dir()
            } else {
                std::path::PathBuf::from(&settings.logo_save_directory)
            }
        });
    let channels = store.list_managed(None).map_err(|e| e.to_string())?;
    Ok((dir.to_string_lossy().into_owned(), logo::plan_save(&channels, &dir)))
}

#[tauri::command]
fn logo_save_one(
    state: tauri::State<AppState>,
    mut item: logo::LogoSaveItem,
) -> Result<logo::LogoSaveItem, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    logo::save_one(&mut item, &settings.ffmpeg_path);
    Ok(item)
}

#[tauri::command]
fn logo_save_tracker(root: String, items: Vec<logo::LogoSaveItem>) -> Result<(), String> {
    if root.trim().is_empty() {
        return Err("Pick a save folder first.".into());
    }
    logo::save_tracker(std::path::Path::new(root.trim()), &items);
    Ok(())
}

#[tauri::command]
fn logo_search_urls(name: String) -> (String, String, String) {
    logo::search_urls(&name)
}

#[tauri::command]
fn audit_snapshot(state: tauri::State<AppState>) -> Result<audit::AuditSnapshot, String> {
    let process = lock_audit(&state)?;
    audit::snapshot(&process).map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_begin(
    state: tauri::State<AppState>,
    visible_only: bool,
    auto_swap: bool,
    channel_ids: Option<Vec<String>>,
) -> Result<audit::AuditJob, String> {
    let store = lock_store(&state)?;
    let process = lock_audit(&state)?;
    if let Ok(Some(job)) = process.load_job() {
        if job.state == "running" {
            return Err("Audit already running.".into());
        }
    }
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    audit::begin_job(
        &store,
        &process,
        &settings,
        auto_swap,
        visible_only,
        channel_ids.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_next(state: tauri::State<AppState>) -> Result<audit::AuditStep, String> {
    let store = lock_store(&state)?;
    let process = lock_audit(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    audit::next_step(&store, &process, &settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_set_state(state: tauri::State<AppState>, next: String) -> Result<Option<audit::AuditJob>, String> {
    let process = lock_audit(&state)?;
    let Some(mut job) = process.load_job().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    match next.as_str() {
        "paused" => {
            job.state = "paused".into();
        }
        "running" => {
            if !job.has_remaining() {
                return Err("Nothing to resume.".into());
            }
            job.state = "running".into();
            job.pid = std::process::id();
        }
        "cancelled" => {
            job.state = "cancelled".into();
            job.finished_at = Some(audit::now_iso());
        }
        _ => return Err("unknown audit state".into()),
    }
    process.update_job(&job).map_err(|e| e.to_string())?;
    Ok(Some(job))
}

#[tauri::command]
fn audit_discard(state: tauri::State<AppState>) -> Result<(), String> {
    lock_audit(&state)?.clear().map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_undo(state: tauri::State<AppState>) -> Result<bool, String> {
    lock_store(&state)?
        .undo_last_swap(None)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_today_groups(state: tauri::State<AppState>) -> Result<(String, Vec<String>, Vec<String>), String> {
    let store = lock_store(&state)?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let day = audit::today_name();
    let plan = audit::parse_weekly(&settings.weekly_audit_json);
    let groups = audit::groups_for(&plan, &day);
    let set: std::collections::HashSet<_> = groups.iter().map(|g| g.to_ascii_lowercase()).collect();
    let ids = store
        .list_managed(None)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|c| set.contains(&c.group_title.to_ascii_lowercase()))
        .map(|c| c.id)
        .collect();
    Ok((day, groups, ids))
}

#[tauri::command]
fn audit_mark_today_ran(state: tauri::State<AppState>) -> Result<(), String> {
    let store = lock_store(&state)?;
    let mut settings = store.load_settings().map_err(|e| e.to_string())?;
    settings.weekly_audit_last_run = audit::day_key();
    store.save_settings(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn audit_results(state: tauri::State<AppState>, job_id: Option<String>) -> Result<Vec<audit::AuditResult>, String> {
    lock_store(&state)?
        .list_audit_results(job_id.as_deref(), 4000)
        .map_err(|e| e.to_string())
}

fn make_snapshot(store: &SqliteStore) -> TunerSnapshot {
    let settings = store.load_settings().unwrap_or_default();
    let channels = store.list_managed(None).unwrap_or_default();
    let ids: Vec<String> = studio_core::lineup::ordered_lineup(&channels)
        .into_iter()
        .map(|c| studio_core::hdhr::channel_xml_id(&c))
        .filter(|s| !s.is_empty())
        .collect();
    let programmes = store
        .list_programmes(&ids, "1970-01-01T00:00:00Z", "2099-01-01T00:00:00Z")
        .unwrap_or_default();
    let mut snap = manager::snapshot_from_settings(channels, programmes, &settings);
    snap.variant_headers = store.headers_for_channels(&snap.channels);
    snap
}

fn persist_settings(state: &tauri::State<AppState>, settings: &AppSettings) -> Result<(), String> {
    lock_store(state)?.save_settings(settings).map_err(|e| e.to_string())
}

#[tauri::command]
fn tuner_statuses(state: tauri::State<AppState>) -> Result<Vec<manager::TunerRuntimeStatus>, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    Ok(state.tuner.lock().map_err(|e| e.to_string())?.all_statuses(&settings))
}

#[tauri::command]
fn tuner_start(state: tauri::State<AppState>, kind: String) -> Result<String, String> {
    let store = Arc::clone(&state.store);
    let snap: Arc<dyn Fn() -> TunerSnapshot + Send + Sync> = Arc::new(move || {
        let g = store.lock().ok();
        match g {
            Some(s) => {
                let mut snap = make_snapshot(&s);
                let store2 = Arc::clone(&store);
                snap.note_failover = Some(Arc::new(move |ch, var| {
                    let Ok(st) = store2.lock() else {
                        return;
                    };
                    let auto = st
                        .load_settings()
                        .map(|cfg| cfg.auto_swap_on_audit_fail)
                        .unwrap_or(true);
                    if !auto {
                        return;
                    }
                    let Some(vis) = ch.variants.iter().find(|v| v.visibility == "visible") else {
                        return;
                    };
                    if vis.id == var.id {
                        return;
                    }
                    let _ = st.swap_visible(&ch.id, &vis.id, &var.id, "live failover");
                }));
                snap
            }
            None => TunerSnapshot::default(),
        }
    });
    let mut settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    let mut tuner = state.tuner.lock().map_err(|e| e.to_string())?;
    tuner.try_start(&mut settings, &kind, snap)?;
    drop(tuner);
    persist_settings(&state, &settings)?;
    Ok(format!("{kind} tuner started"))
}

#[tauri::command]
fn tuner_stop(state: tauri::State<AppState>, kind: String) -> Result<(), String> {
    let mut settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    state.tuner.lock().map_err(|e| e.to_string())?.stop(&mut settings, &kind);
    persist_settings(&state, &settings)
}

#[tauri::command]
fn tuner_start_all(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    let kinds: Vec<String> = [
        &settings.plex_tuner,
        &settings.jellyfin_tuner,
        &settings.emby_tuner,
        &settings.iptv_tuner,
    ]
    .into_iter()
    .filter(|p| p.enabled && !p.running)
    .map(|p| p.kind.clone())
    .collect();
    drop(settings);
    let mut errors = Vec::new();
    for k in kinds {
        if let Err(e) = tuner_start(state.clone(), k.clone()) {
            errors.push(format!("{k}: {e}"));
        }
    }
    Ok(errors)
}

#[tauri::command]
fn tuner_stop_all(state: tauri::State<AppState>) -> Result<(), String> {
    let mut settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    state.tuner.lock().map_err(|e| e.to_string())?.stop_all(&mut settings);
    persist_settings(&state, &settings)
}

#[tauri::command]
fn tuner_set_max(state: tauri::State<AppState>, kind: String, max: i32) -> Result<(), String> {
    let mut settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    state
        .tuner
        .lock()
        .map_err(|e| e.to_string())?
        .set_max(&mut settings, &kind, max)?;
    persist_settings(&state, &settings)
}

#[tauri::command]
fn tuner_self_test(state: tauri::State<AppState>) -> Result<String, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    let statuses = state.tuner.lock().map_err(|e| e.to_string())?.all_statuses(&settings);
    drop(settings);
    let (_reports, json) = manager::self_test(&statuses);
    Ok(json)
}

#[tauri::command]
fn tuner_logs(state: tauri::State<AppState>) -> Result<Vec<manager::TunerLogLine>, String> {
    Ok(state.tuner.lock().map_err(|e| e.to_string())?.logs().to_vec())
}

#[tauri::command]
fn tuner_graphs(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    Ok(state.tuner.lock().map_err(|e| e.to_string())?.graphs())
}

#[tauri::command]
fn tuner_help(state: tauri::State<AppState>, kind: String) -> Result<String, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    let st = state
        .tuner
        .lock()
        .map_err(|e| e.to_string())?
        .status(&settings, &kind)
        .ok_or_else(|| "unknown tuner".to_string())?;
    Ok(studio_tuner::help::text_for(
        &st.kind,
        st.base_url.trim_end_matches('/'),
        &st.device_id,
        st.port,
        st.enabled,
        st.running,
        settings.jellyfin_tuner.downspiral_enabled,
    ))
}

#[tauri::command]
fn epg_search_images_url(name: String) -> String {
    let q = if name.trim().is_empty() {
        "channel logo".into()
    } else {
        format!("{} logo", name.trim())
    };
    epg::google_images_transparent_url(&q)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = database_path();
    let store = SqliteStore::open(&db).expect("open studio database");
    let audit_store = audit::ProcessStore::open(None).expect("open auditprocess database");
    let _ = app_data_directory();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            store: Arc::new(Mutex::new(store)),
            audit: Mutex::new(audit_store),
            tuner: Mutex::new(TunerManager::new()),
        })
        .setup(|app| {
            studio_core::crash::append_log("Info", "App", "OnLaunched");
            if let Some(w) = app.get_webview_window("main") {
                apply_window_chrome(&w, true);
            }
            let menu = tauri::menu::MenuBuilder::new(app)
                .text("audit", "Add Sources")
                .text("editor", "Playlist Editor")
                .text("epg", "EPG Audit")
                .text("logoaudit", "Logo Audit")
                .text("autoaudit", "Stream Audit")
                .text("output", "Managed Output")
                .text("tuner", "TV Tuner")
                .text("settings", "Settings")
                .separator()
                .text("toggle", "Show / Hide")
                .text("quit", "Close app")
                .build()?;
            let mut tray = tauri::tray::TrayIconBuilder::with_id("main")
                .tooltip(DISPLAY_NAME)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    match event.id().as_ref() {
                        "quit" => {
                            studio_core::crash::mark_clean_exit();
                            app.exit(0);
                        }
                        "toggle" => toggle_main_window(app),
                        id => {
                            show_main_window(app);
                            let _ = app.emit("studio-navigate", id);
                        }
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let _ = tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
                studio_core::crash::mark_tray_state();
                let _ = window.app_handle().emit("studio-hidden-to-tray", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_studio_info,
            splash_checks,
            splash_epg_status,
            check_app_update,
            check_github_issues,
            promote_main_window,
            detect_bundled_tools,
            tools_missing,
            tools_ensure,
            list_sources,
            list_groups,
            list_channels,
            search_sources,
            remove_source,
            pick_source_file,
            pick_playlist_path,
            add_source_file,
            add_source_url,
            play_url,
            refresh_source,
            add_backup_from_source,
            managed_count,
            list_managed_groups,
            list_managed,
            get_managed,
            save_managed,
            delete_managed,
            rename_managed_group,
            add_stream,
            delete_variant,
            move_variant,
            suggest_tvg,
            now_playing,
            is_known_tvg,
            add_from_source,
            import_curated,
            export_managed,
            output_summary,
            lineup_candidates,
            save_tuner_lineup,
            export_channels_json,
            publish_channels,
            tuner_statuses,
            tuner_start,
            tuner_stop,
            tuner_start_all,
            tuner_stop_all,
            tuner_set_max,
            tuner_self_test,
            tuner_logs,
            tuner_graphs,
            tuner_help,
            clear_managed,
            load_settings,
            save_settings,
            detect_tool_paths,
            members_ping,
            settings_folders,
            list_slates,
            add_slate,
            remove_slate,
            open_folder,
            consume_pending_crash,
            write_crash_report,
            log_heartbeat,
            mark_tray_state,
            mark_clean_exit,
            post_issue,
            epg_catalog_count,
            epg_guide_url,
            fetch_epg_catalog,
            epg_refresh_schedule,
            rebuild_now_playing,
            epg_audit,
            epg_apply,
            epg_auto_match,
            epg_browse_catalog,
            epg_search_images_url,
            logo_scan,
            logo_set,
            logo_batch_set,
            logo_default_dir,
            logo_save_plan,
            logo_save_one,
            logo_save_tracker,
            logo_search_urls,
            audit_snapshot,
            audit_begin,
            audit_next,
            audit_set_state,
            audit_discard,
            audit_undo,
            audit_today_groups,
            audit_mark_today_ran,
            audit_results,
        ])
        .run(tauri::generate_context!())
        .expect("error while running epg.monster studio");
}
