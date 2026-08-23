// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;

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
    display_version, github_open_studio_issues, latest_github_release, latest_github_tag,
    remote_is_newer, DISPLAY_NAME, EDITION, GITHUB_RELEASES_LATEST, VERSION,
};
use tauri_plugin_opener::OpenerExt;
use studio_tuner::manager::{self, TunerManager};
use studio_tuner::host::TunerSnapshot;
use tauri::{Emitter, Manager};
use std::sync::Arc;

struct AppState {
    store: Arc<Mutex<SqliteStore>>,
    audit: Arc<Mutex<audit::ProcessStore>>,
    tuner: Mutex<TunerManager>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SelfTestDto {
    json: String,
    path: String,
    reports: Vec<studio_tuner::probe::TunerProbeReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioInfoDto {
    version: String,
    display_version: String,
    display_name: String,
    edition: String,
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
    headers_json: String,
    channel_count: i32,
    expires_at: Option<i64>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceProgress {
    id: String,
    name: String,
    channel_count: i32,
    done: bool,
    error: Option<String>,
    expires_at: Option<i64>,
    /// "add" or "refresh" â€” refresh failures must not drop the source tab.
    op: String,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct XtreamSourceArgs {
    server: String,
    username: String,
    password: String,
    output: Option<String>,
    name: Option<String>,
    headers: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSourceArgs {
    id: String,
    name: String,
    kind: String,
    location: String,
    headers: Option<BTreeMap<String, String>>,
    refetch: Option<bool>,
}

impl From<PlaylistSource> for SourceDto {
    fn from(s: PlaylistSource) -> Self {
        Self {
            id: s.id,
            name: s.name,
            kind: s.kind,
            location: s.location,
            headers_json: s.headers_json,
            channel_count: s.channel_count,
            expires_at: s.expires_at,
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
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
        let _ = w.set_always_on_top(false);
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
    Ok(state.store.lock().unwrap_or_else(|e| e.into_inner()))
}

fn emit_source_progress(app: &tauri::AppHandle, p: SourceProgress) {
    let _ = app.emit("source-progress", &p);
}

fn source_progress(
    id: String,
    name: String,
    channel_count: i32,
    done: bool,
    error: Option<String>,
    expires_at: Option<i64>,
    op: &str,
) -> SourceProgress {
    SourceProgress {
        id,
        name,
        channel_count,
        done,
        error,
        expires_at,
        op: op.into(),
    }
}

fn spawn_refresh_source(
    app: tauri::AppHandle,
    store: Arc<Mutex<SqliteStore>>,
    source_id: String,
    cache: std::path::PathBuf,
) {
    tauri::async_runtime::spawn_blocking(move || {
        let fail = |name: String, msg: String| {
            emit_source_progress(
                &app,
                source_progress(
                    source_id.clone(),
                    name,
                    0,
                    true,
                    Some(msg),
                    None,
                    "refresh",
                ),
            );
        };
        let src = match store.lock() {
            Ok(s) => match s.get_source(&source_id) {
                Ok(Some(src)) => src,
                Ok(None) => {
                    fail("Source".into(), "source not found".into());
                    return;
                }
                Err(e) => {
                    fail("Source".into(), e.to_string());
                    return;
                }
            },
            Err(e) => {
                fail("Source".into(), e.to_string());
                return;
            }
        };
        let shown = src.name.clone();
        let body = if src.kind == "url" || src.kind == "xtream" {
            let headers: std::collections::BTreeMap<String, String> =
                serde_json::from_str(&src.headers_json).unwrap_or_default();
            match SqliteStore::fetch_url_playlist(&src.location, &headers, &cache, &source_id) {
                Ok(b) => b,
                Err(e) => {
                    fail(shown, e.to_string());
                    return;
                }
            }
        } else {
            match std::fs::read_to_string(&src.location) {
                Ok(b) => b,
                Err(e) => {
                    fail(shown, e.to_string());
                    return;
                }
            }
        };
        loop {
            let more = match store.lock() {
                Ok(s) => s.clear_source_entries_chunk(&source_id, 2_000),
                Err(e) => {
                    fail(shown.clone(), e.to_string());
                    return;
                }
            };
            match more {
                Ok(true) => continue,
                Ok(false) => break,
                Err(e) => {
                    fail(shown, e.to_string());
                    return;
                }
            }
        }
        if let Ok(s) = store.lock() {
            let _ = s.set_channel_count(&source_id, 0);
        }
        let count = match import_m3u_unlocked(&store, &source_id, &body, |_| {}) {
            Ok(n) => n,
            Err(e) => {
                fail(shown, e);
                return;
            }
        };
        let mut exp = src.expires_at;
        if src.kind == "xtream" {
            if let Ok(s) = store.lock() {
                exp = s.refresh_xtream_expiry(&source_id, &src.location, &src.headers_json);
            }
        } else if let Ok(s) = store.lock() {
            let _ = s.set_channel_count(&source_id, count);
        }
        emit_source_progress(
            &app,
            source_progress(source_id, shown, count, true, None, exp, "refresh"),
        );
    });
}

fn append_batch(
    store: &Arc<Mutex<SqliteStore>>,
    id: &str,
    batch: &[ChannelEntry],
) -> Result<i32, String> {
    store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .append_channel_batch(id, batch)
        .map_err(|e| e.to_string())
}

fn import_m3u_unlocked(
    store: &Arc<Mutex<SqliteStore>>,
    id: &str,
    content: &str,
    mut on_count: impl FnMut(i32),
) -> Result<i32, String> {
    let mut batch = Vec::with_capacity(400);
    let mut total = 0i32;
    let mut err = None;
    studio_core::parser::for_each_m3u_channel(content, id, |ch| {
        if err.is_some() {
            return;
        }
        batch.push(ch);
        if batch.len() >= 400 {
            match append_batch(store, id, &batch) {
                Ok(n) => {
                    total += n;
                    on_count(total);
                }
                Err(e) => err = Some(e),
            }
            batch.clear();
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    if !batch.is_empty() {
        total += append_batch(store, id, &batch)?;
        on_count(total);
    }
    Ok(total)
}

async fn with_store<T, F>(store: Arc<Mutex<SqliteStore>>, f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(&SqliteStore) -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let g = store.lock().unwrap_or_else(|e| e.into_inner());
        f(&g)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn lock_audit<'a>(
    state: &'a tauri::State<'a, AppState>,
) -> Result<std::sync::MutexGuard<'a, audit::ProcessStore>, String> {
    state.audit.lock().map_err(|e| e.to_string())
}

fn audit_worker_slot() -> &'static Mutex<Option<JoinHandle<()>>> {
    static SLOT: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

fn join_audit_worker() {
    audit::request_interrupt(audit::Interrupt::Cancel);
    if let Some(h) = audit_worker_slot()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        let _ = h.join();
    }
    audit::clear_interrupt();
}

fn spawn_audit_worker(store: Arc<Mutex<SqliteStore>>) {
    join_audit_worker();
    let handle = std::thread::Builder::new()
        .name("stream-audit".into())
        .spawn(move || {
            let process = match audit::ProcessStore::open(None) {
                Ok(p) => p,
                Err(_) => return,
            };
            loop {
                match audit::interrupt_kind() {
                    audit::Interrupt::None => {}
                    kind => {
                        if let Ok(Some(mut job)) = process.load_job() {
                            if kind == audit::Interrupt::Cancel {
                                job.state = "cancelled".into();
                                job.finished_at = Some(audit::now_iso());
                            } else {
                                job.state = "paused".into();
                            }
                            let _ = process.update_job(&job);
                        }
                        break;
                    }
                }
                let settings = match store.lock() {
                    Ok(s) => s.load_settings().ok(),
                    Err(_) => None,
                };
                let Some(settings) = settings else {
                    break;
                };
                if audit::player_is_active() && settings.pause_audit_while_playing {
                    let _ = process.add_elapsed_ms(400);
                    std::thread::sleep(std::time::Duration::from_millis(400));
                    continue;
                }
                let step = audit::next_step(&store, &process, &settings);
                match step {
                    Ok(s) if s.done || s.job.state != "running" => break,
                    Ok(_) => {
                        let delay = settings.audit_delay_ms.max(0) as u64;
                        let mut left = delay;
                        while left > 0 {
                            if audit::interrupt_kind() != audit::Interrupt::None {
                                break;
                            }
                            let d = 100u64.min(left);
                            std::thread::sleep(std::time::Duration::from_millis(d));
                            let _ = process.add_elapsed_ms(d as i64);
                            left -= d;
                        }
                    }
                    Err(_) => {
                        if audit::interrupt_kind() != audit::Interrupt::None {
                            if let Ok(Some(mut job)) = process.load_job() {
                                if audit::interrupt_kind() == audit::Interrupt::Cancel {
                                    job.state = "cancelled".into();
                                    job.finished_at = Some(audit::now_iso());
                                } else {
                                    job.state = "paused".into();
                                }
                                let _ = process.update_job(&job);
                            }
                        }
                        break;
                    }
                }
            }
        });
    if let Ok(h) = handle {
        *audit_worker_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(h);
    }
}

fn app_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    let mut hints = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        hints.push(cwd);
    }
    if let Ok(p) = app.path().resource_dir() {
        if let Some(parent) = p.parent() {
            hints.push(parent.to_path_buf());
        }
        hints.push(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            hints.push(dir.to_path_buf());
        }
    }
    if hints.is_empty() {
        return studio_core::bootstrap::find_app_root(&std::path::PathBuf::from("."));
    }
    let mut fallback = studio_core::bootstrap::find_app_root(&hints[0]);
    for hint in &hints {
        let found = studio_core::bootstrap::find_app_root(hint);
        if found.join("package.json").is_file() && found.join("src-tauri").is_dir() {
            return found;
        }
        let ffplay = found
            .join("tools")
            .join("ffmpeg")
            .join(studio_core::tools::tool_file_name("ffplay"));
        if ffplay.is_file() {
            return found;
        }
        fallback = found;
    }
    fallback
}

#[tauri::command]
async fn get_studio_info(state: tauri::State<'_, AppState>) -> Result<StudioInfoDto, String> {
    with_store(Arc::clone(&state.store), |s| {
        Ok(StudioInfoDto {
            version: VERSION.to_string(),
            display_version: display_version(),
            display_name: DISPLAY_NAME.to_string(),
            edition: EDITION.to_string(),
            database_path: database_path().to_string_lossy().into_owned(),
            managed_count: s.managed_count().unwrap_or(0),
        })
    })
    .await
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
        format!("...{}", &s[s.len() - 52..])
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
            detail: "Not found - set in Settings".into(),
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
async fn splash_checks(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<Vec<SplashCheck>, String> {
    let root = app_root(&app);
    let settings = with_store(Arc::clone(&state.store), |s| {
        Ok(s.load_settings().unwrap_or_default())
    })
    .await
    .unwrap_or_default();
    let pick = |stored: &str, fallback: std::path::PathBuf| {
        let t = stored.trim();
        if !t.is_empty() {
            let p = std::path::PathBuf::from(t);
            if p.is_file() {
                return p;
            }
        }
        fallback
    };
    let data = app_data_directory();
    let db = database_path();
    let cache = data.join("cache");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(&cache);
    Ok(vec![
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
            "mpv (optional)",
            &pick(&settings.mpv_path, default_mpv_path(&root)),
            false,
        ),
        splash_check_path(
            "ffmpeg (auto-audit)",
            &pick(&settings.ffmpeg_path, default_ffmpeg_path(&root)),
            true,
        ),
        splash_check_path(
            "ffprobe",
            &pick(&settings.ffprobe_path, default_ffprobe_path(&root)),
            true,
        ),
        splash_check_path("VLC (optional)", &pick(&settings.vlc_path, default_vlc_path()), false),
        SplashCheck {
            label: "Playlist cache folder".into(),
            ok: cache.is_dir(),
            detail: splash_shorten(&cache.display().to_string()),
        },
    ])
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SplashEpgStatus {
    catalog: i32,
    programmes: i32,
    cached: bool,
}

#[tauri::command]
async fn check_app_update() -> Result<SplashCheck, String> {
    tauri::async_runtime::spawn_blocking(|| match latest_github_tag() {
        Ok(tag) => {
            if remote_is_newer(&tag, VERSION) {
                Ok(SplashCheck {
                    label: "Checking github for updates".into(),
                    ok: true,
                    detail: format!("update {tag}"),
                })
            } else {
                Ok(SplashCheck {
                    label: "Checking github for updates".into(),
                    ok: true,
                    detail: format!("up to date ({tag})"),
                })
            }
        }
        Err(e) => Ok(SplashCheck {
            label: "Checking github for updates".into(),
            ok: false,
            detail: splash_shorten(&e),
        }),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioUpdateDto {
    current: String,
    display_version: String,
    edition: String,
    latest: Option<String>,
    update_available: bool,
    release_url: String,
    notes: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HostInfoDto {
    os: String,
    arch: String,
    host: String,
    exe_suffix: String,
}

#[tauri::command]
fn host_info() -> HostInfoDto {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    HostInfoDto {
        os: os.into(),
        arch: std::env::consts::ARCH.into(),
        host: studio_core::bootstrap::current_host(),
        exe_suffix: if cfg!(windows) { ".exe".into() } else { String::new() },
    }
}

#[tauri::command]
async fn check_studio_update() -> StudioUpdateDto {
    tauri::async_runtime::spawn_blocking(|| {
    let current = VERSION.to_string();
    match latest_github_release() {
        Ok(rel) => {
            let notes = rel.body.map(|b| {
                let t = b.trim();
                if t.chars().count() > 400 {
                    format!("{}…", t.chars().take(400).collect::<String>())
                } else {
                    t.to_string()
                }
            });
            StudioUpdateDto {
                current,
                display_version: display_version(),
                edition: EDITION.to_string(),
                latest: Some(rel.tag.clone()),
                update_available: remote_is_newer(&rel.tag, VERSION),
                release_url: if rel.html_url.is_empty() {
                    GITHUB_RELEASES_LATEST.to_string()
                } else {
                    rel.html_url
                },
                notes,
                error: None,
            }
        }
        Err(e) => StudioUpdateDto {
            current,
            display_version: display_version(),
            edition: EDITION.to_string(),
            latest: None,
            update_available: false,
            release_url: GITHUB_RELEASES_LATEST.to_string(),
            notes: None,
            error: Some(e),
        },
    }
    })
    .await
    .unwrap_or_else(|e| StudioUpdateDto {
        current: VERSION.to_string(),
        display_version: display_version(),
        edition: EDITION.to_string(),
        latest: None,
        update_available: false,
        release_url: GITHUB_RELEASES_LATEST.to_string(),
        notes: None,
        error: Some(e.to_string()),
    })
}

#[tauri::command]
async fn open_latest_release(app: tauri::AppHandle) -> Result<String, String> {
    let url = tauri::async_runtime::spawn_blocking(|| match latest_github_release() {
        Ok(rel) if !rel.html_url.is_empty() => rel.html_url,
        _ => GITHUB_RELEASES_LATEST.to_string(),
    })
    .await
    .map_err(|e| e.to_string())?;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(url)
}

#[tauri::command]
async fn check_github_issues() -> Result<SplashCheck, String> {
    tauri::async_runtime::spawn_blocking(|| match github_open_studio_issues() {
        Ok((n, title)) => {
            let detail = match (n, title) {
                (0, _) => "0 open".into(),
                (1, Some(t)) => format!("1 open Â· {t}"),
                (n, Some(t)) => format!("{n} open Â· {t}"),
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
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn splash_epg_status(state: tauri::State<'_, AppState>) -> Result<SplashEpgStatus, String> {
    with_store(Arc::clone(&state.store), |s| {
        let catalog = s.catalog_count().unwrap_or(0);
        let programmes = s.programme_count().unwrap_or(0);
        Ok(SplashEpgStatus {
            catalog,
            programmes,
            cached: catalog > 0,
        })
    })
    .await
}

#[tauri::command]
fn open_epg_catalog_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("epg-catalog") {
        let _ = existing.eval(
            "if (!/catalog\\.html/i.test(location.pathname + location.href)) { location.replace('catalog.html'); }",
        );
        let _ = existing.unminimize();
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let w = tauri::WebviewWindowBuilder::new(
        &app,
        "epg-catalog",
        tauri::WebviewUrl::App("catalog.html".into()),
    )
    .title("EPG catalog")
    .inner_size(680.0, 820.0)
    .min_inner_size(420.0, 480.0)
    .resizable(true)
    .maximizable(true)
    .minimizable(true)
    .decorations(false)
    .shadow(true)
    .center()
    .background_color(tauri::window::Color(0x0c, 0x0c, 0x10, 0xff))
    .initialization_script("window.__STUDIO_VIEW='catalog';")
    .build()
    .map_err(|e| e.to_string())?;
    apply_window_chrome(&w, false);
    let _ = w.show();
    let _ = w.set_focus();
    Ok(())
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
    for _ in 0..3 {
        let _ = w.set_size(tauri::LogicalSize::new(1400.0, 900.0));
        if let Ok(sz) = w.inner_size() {
            if sz.width >= 900 && sz.height >= 600 {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
    let _ = w.center();
    apply_window_chrome(&w, false);
    let _ = w.show();
    let _ = w.unminimize();
    let _ = w.set_always_on_top(true);
    let _ = w.set_focus();
    let _ = w.set_always_on_top(false);
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
async fn tools_missing(app: tauri::AppHandle) -> Result<Vec<studio_core::bootstrap::ToolSpec>, String> {
    let root = app_root(&app);
    tauri::async_runtime::spawn_blocking(move || studio_core::bootstrap::missing_tools(&root))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn tools_ensure(app: tauri::AppHandle) -> Result<(), String> {
    let root = app_root(&app);
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        studio_core::bootstrap::ensure(&root, |p| {
            let _ = handle.emit("tools-progress", &p);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_sources(state: tauri::State<'_, AppState>) -> Result<Vec<SourceDto>, String> {
    with_store(Arc::clone(&state.store), |s| {
        Ok(s.list_sources()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(SourceDto::from)
            .collect())
    })
    .await
}

#[tauri::command]
async fn list_groups(
    state: tauri::State<'_, AppState>,
    source_id: String,
) -> Result<Vec<GroupDto>, String> {
    with_store(Arc::clone(&state.store), move |s| {
        Ok(s.groups_with_counts(&source_id)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(title, count)| GroupDto { title, count })
            .collect())
    })
    .await
}

#[tauri::command]
async fn list_channels(
    state: tauri::State<'_, AppState>,
    source_id: String,
    group_title: String,
    limit: Option<i32>,
) -> Result<Vec<ChannelDto>, String> {
    with_store(Arc::clone(&state.store), move |s| {
        Ok(s.channels_by_group(&source_id, &group_title, limit.unwrap_or(5000))
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(ChannelDto::from)
            .collect())
    })
    .await
}

#[tauri::command]
async fn search_sources(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<ChannelDto>, String> {
    with_store(Arc::clone(&state.store), move |s| {
        Ok(s.search_sources(&query)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(ChannelDto::from)
            .collect())
    })
    .await
}

#[tauri::command]
async fn remove_source(
    state: tauri::State<'_, AppState>,
    source_id: String,
) -> Result<(), String> {
    let store = Arc::clone(&state.store);
    loop {
        let more = with_store(Arc::clone(&store), {
            let id = source_id.clone();
            move |s| s.remove_source_chunk(&id, 2_000).map_err(|e| e.to_string())
        })
        .await?;
        if !more {
            break;
        }
    }
    Ok(())
}

#[tauri::command]
fn pick_source_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    pick_playlist_path(app)
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
async fn add_source_file(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: FileSourceArgs,
) -> Result<SourceDto, String> {
    let path = std::path::PathBuf::from(args.path.trim().trim_matches('"'));
    if !path.is_file() {
        return Err("File path (required â€” browse or paste a valid path)".into());
    }
    let name = args
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("playlist")
                .to_string()
        });
    let loc = path.to_string_lossy().into_owned();
    let stub = with_store(Arc::clone(&state.store), {
        let name = name.clone();
        let loc = loc.clone();
        move |s| {
            s.begin_source(&name, "file", &loc, "{}", None)
                .map(SourceDto::from)
                .map_err(|e| e.to_string())
        }
    })
    .await?;
    let store = Arc::clone(&state.store);
    let id = stub.id.clone();
    let shown = stub.name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fail = |msg: String| {
            emit_source_progress(
                &app,
                source_progress(
                    id.clone(),
                    shown.clone(),
                    0,
                    true,
                    Some(msg),
                    None,
                    "add",
                ),
            );
            if let Ok(s) = store.lock() {
                let _ = s.remove_source(&id);
            }
        };
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                fail(e.to_string());
                return;
            }
        };
        match import_m3u_unlocked(&store, &id, &content, |_| {}) {
            Ok(n) => emit_source_progress(
                &app,
                source_progress(id, shown, n, true, None, None, "add"),
            ),
            Err(e) => fail(e),
        }
    });
    Ok(stub)
}

#[tauri::command]
async fn add_source_url(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: UrlSourceArgs,
) -> Result<SourceDto, String> {
    let headers = args.headers.unwrap_or_default();
    let headers_json = serde_json::to_string(&headers).map_err(|e| e.to_string())?;
    let display = args
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("URL")
        .to_string();
    let url = args.url.clone();
    let stub = with_store(Arc::clone(&state.store), {
        let display = display.clone();
        let url = url.clone();
        let headers_json = headers_json.clone();
        move |s| {
            s.begin_source(&display, "url", &url, &headers_json, None)
                .map(SourceDto::from)
                .map_err(|e| e.to_string())
        }
    })
    .await?;
    let store = Arc::clone(&state.store);
    let cache = app_data_directory().join("cache");
    let id = stub.id.clone();
    let shown = stub.name.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fail = |msg: String| {
            emit_source_progress(
                &app,
                source_progress(
                    id.clone(),
                    shown.clone(),
                    0,
                    true,
                    Some(msg),
                    None,
                    "add",
                ),
            );
            if let Ok(s) = store.lock() {
                let _ = s.remove_source(&id);
            }
        };
        let body = match studio_core::store::SqliteStore::fetch_url_playlist(
            &url, &headers, &cache, &id,
        ) {
            Ok(b) => b,
            Err(e) => {
                fail(e.to_string());
                return;
            }
        };
        match import_m3u_unlocked(&store, &id, &body, |_| {}) {
            Ok(n) => emit_source_progress(
                &app,
                source_progress(id, shown, n, true, None, None, "add"),
            ),
            Err(e) => fail(e),
        }
    });
    Ok(stub)
}

#[tauri::command]
async fn add_source_xtream(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: XtreamSourceArgs,
) -> Result<SourceDto, String> {
    let headers = args.headers.unwrap_or_default();
    let headers_json = serde_json::to_string(&headers).map_err(|e| e.to_string())?;
    let url = studio_core::xtream::playlist_url(
        &args.server,
        &args.username,
        &args.password,
        args.output.as_deref().unwrap_or("ts"),
    )?;
    let display = args
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            studio_core::xtream::normalize_base(&args.server).unwrap_or_else(|_| "Xtream".into())
        });
    let stub = with_store(Arc::clone(&state.store), {
        let display = display.clone();
        let url = url.clone();
        let headers_json = headers_json.clone();
        move |s| {
            s.begin_source(&display, "xtream", &url, &headers_json, None)
                .map(SourceDto::from)
                .map_err(|e| e.to_string())
        }
    })
    .await?;
    let store = Arc::clone(&state.store);
    let cache = app_data_directory().join("cache");
    let id = stub.id.clone();
    let shown = stub.name.clone();
    let server = args.server.clone();
    let username = args.username.clone();
    let password = args.password.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let fail = |msg: String| {
            emit_source_progress(
                &app,
                source_progress(
                    id.clone(),
                    shown.clone(),
                    0,
                    true,
                    Some(msg),
                    None,
                    "add",
                ),
            );
            if let Ok(s) = store.lock() {
                let _ = s.remove_source(&id);
            }
        };
        let body = match studio_core::store::SqliteStore::fetch_url_playlist(
            &url, &headers, &cache, &id,
        ) {
            Ok(b) => b,
            Err(e) => {
                fail(e.to_string());
                return;
            }
        };
        let count = match import_m3u_unlocked(&store, &id, &body, |_| {}) {
            Ok(n) => n,
            Err(e) => {
                fail(e);
                return;
            }
        };
        if count == 0 {
            fail("Xtream login failed or the server returned an empty playlist. Check host, username, and password.".into());
            return;
        }
        let exp = studio_core::xtream::fetch_exp_date(&server, &username, &password, &headers);
        if let Ok(s) = store.lock() {
            let _ = s.set_expires_at(&id, exp);
        }
        emit_source_progress(
            &app,
            source_progress(id, shown, count, true, None, exp, "add"),
        );
    });
    Ok(stub)
}

#[tauri::command]
async fn probe_xtream_expiry(
    state: tauri::State<'_, AppState>,
    source_id: String,
) -> Result<Option<i64>, String> {
    let src = with_store(Arc::clone(&state.store), {
        let source_id = source_id.clone();
        move |s| {
            s.list_sources()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|x| x.id == source_id)
                .ok_or_else(|| "source not found".to_string())
        }
    })
    .await?;
    if src.kind != "xtream" {
        return Ok(src.expires_at);
    }
    let location = src.location.clone();
    let headers_json = src.headers_json.clone();
    let exp = tauri::async_runtime::spawn_blocking(move || {
        let (server, user, pass) = studio_core::xtream::parse_login(&location)?;
        let headers: BTreeMap<String, String> =
            serde_json::from_str(&headers_json).unwrap_or_default();
        studio_core::xtream::fetch_exp_date(&server, &user, &pass, &headers)
    })
    .await
    .map_err(|e| e.to_string())?;
    let id = src.id.clone();
    with_store(Arc::clone(&state.store), move |s| {
        s.set_source_expiry(&id, exp).map_err(|e| e.to_string())?;
        Ok(exp)
    })
    .await
}

#[tauri::command]
async fn update_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    args: UpdateSourceArgs,
) -> Result<SourceDto, String> {
    let store = Arc::clone(&state.store);
    let headers = args.headers.unwrap_or_default();
    let headers_json = serde_json::to_string(&headers).map_err(|e| e.to_string())?;
    let src = with_store(Arc::clone(&store), {
        let id = args.id.clone();
        let name = args.name.clone();
        let kind = args.kind.clone();
        let location = args.location.clone();
        move |s| {
            s.update_source_meta(&id, &name, &kind, &location, &headers_json)
                .map(SourceDto::from)
                .map_err(|e| e.to_string())
        }
    })
    .await?;
    if args.refetch.unwrap_or(false) {
        let cache = app_data_directory().join("cache");
        spawn_refresh_source(app, store, src.id.clone(), cache);
    }
    Ok(src)
}

#[tauri::command]
async fn play_url(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    url: String,
    source_id: Option<String>,
) -> Result<(), String> {
    let store = Arc::clone(&state.store);
    let root = app_root(&app);
    let (settings, headers) = with_store(Arc::clone(&store), {
        let source_id = source_id.clone();
        move |s| {
            let settings = s.load_settings().map_err(|e| e.to_string())?;
            let headers = source_id.and_then(|id| {
                s.list_sources()
                    .ok()?
                    .into_iter()
                    .find(|src| src.id == id)
                    .and_then(|src| serde_json::from_str(&src.headers_json).ok())
            });
            Ok((settings, headers))
        }
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        audit::set_player_active(true);
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(30));
            audit::set_player_active(false);
        });
        player::play(&url, &settings, headers.as_ref(), &root)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn refresh_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    source_id: String,
) -> Result<SourceDto, String> {
    let store = Arc::clone(&state.store);
    let src = with_store(Arc::clone(&store), {
        let id = source_id.clone();
        move |s| {
            s.get_source(&id)
                .map_err(|e| e.to_string())?
                .map(SourceDto::from)
                .ok_or_else(|| "source not found".into())
        }
    })
    .await?;
    spawn_refresh_source(app, store, source_id, app_data_directory().join("cache"));
    Ok(src)
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
async fn managed_count(state: tauri::State<'_, AppState>) -> Result<i32, String> {
    with_store(Arc::clone(&state.store), |s| {
        s.managed_count().map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn list_managed_groups(state: tauri::State<'_, AppState>) -> Result<Vec<GroupDto>, String> {
    with_store(Arc::clone(&state.store), |s| {
        Ok(s.managed_groups()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(title, count)| GroupDto { title, count })
            .collect())
    })
    .await
}

#[tauri::command]
async fn list_managed(
    state: tauri::State<'_, AppState>,
    group: Option<String>,
    hydrate: Option<bool>,
) -> Result<Vec<ManagedChannel>, String> {
    with_store(Arc::clone(&state.store), move |s| {
        s.list_managed_opt(group.as_deref(), hydrate.unwrap_or(false))
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn get_managed(
    state: tauri::State<'_, AppState>,
    id: String,
) -> Result<Option<ManagedChannel>, String> {
    with_store(Arc::clone(&state.store), move |s| {
        s.get_managed(&id).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn save_managed(
    state: tauri::State<'_, AppState>,
    channel: ManagedChannel,
    primary_url: Option<String>,
) -> Result<(), String> {
    with_store(Arc::clone(&state.store), move |s| {
        s.save_managed_channel(&channel, primary_url.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn delete_managed(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    with_store(Arc::clone(&state.store), move |s| {
        s.delete_managed(&id).map_err(|e| e.to_string())
    })
    .await
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
async fn add_from_source(
    state: tauri::State<'_, AppState>,
    entry_id: String,
) -> Result<ManagedChannel, String> {
    with_store(Arc::clone(&state.store), move |s| {
        s.add_from_source_entry(&entry_id).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn import_curated(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    replace: bool,
) -> Result<String, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("Playlists", &["m3u", "m3u8", "txt"])
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok("cancelled".into());
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    with_store(Arc::clone(&state.store), move |s| {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let label = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("curated");
        let (added, skipped) = s
            .import_curated(&content, replace, label)
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Imported +{added} channels ({skipped} skipped as duplicates)"
        ))
    })
    .await
}

#[tauri::command]
async fn add_missing_from_source(
    state: tauri::State<'_, AppState>,
    entry_ids: Vec<String>,
    source_label: Option<String>,
) -> Result<String, String> {
    with_store(Arc::clone(&state.store), move |s| {
        let (added, skipped) = s
            .add_missing_from_source_entries(&entry_ids, source_label.as_deref())
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "Added {added} new channel(s); skipped {skipped} already managed"
        ))
    })
    .await
}

#[tauri::command]
async fn export_managed(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
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
    with_store(Arc::clone(&state.store), move |s| {
        let channels = s.list_managed(None).map_err(|e| e.to_string())?;
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
    })
    .await
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
async fn output_summary(
    state: tauri::State<'_, AppState>,
    filter: Option<String>,
) -> Result<OutputSummary, String> {
    with_store(Arc::clone(&state.store), move |store| {
        let settings = store.load_settings().map_err(|e| e.to_string())?;
        let listed = store
            .list_output_rows(filter.as_deref())
            .map_err(|e| e.to_string())?;
        let tuner_count = listed.iter().filter(|c| c.in_tuner).count() as i32;
        let rows = listed
            .into_iter()
            .map(|c| {
                let audit = match c.last_audit_ok {
                    None => "Unknown",
                    Some(true) => "OK",
                    Some(false) => "Fail",
                };
                OutputRow {
                    id: c.id,
                    name: c.name,
                    group: c.group_title,
                    tvg_id: c.tvg_id,
                    visible_url: if c.visible_url.is_empty() {
                        "(none)".into()
                    } else {
                        c.visible_url
                    },
                    variants_summary: format!("1 vis + {} hid", c.hidden),
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
    })
    .await
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
async fn lineup_candidates(state: tauri::State<'_, AppState>) -> Result<Vec<TunerPickRow>, String> {
    with_store(Arc::clone(&state.store), |s| {
        let channels = s
            .list_managed_opt(None, false)
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
    })
    .await
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TunerPick {
    id: String,
    included: bool,
    number: Option<i32>,
}

#[tauri::command]
async fn save_tuner_lineup(
    state: tauri::State<'_, AppState>,
    picks: Vec<TunerPick>,
) -> Result<String, String> {
    with_store(Arc::clone(&state.store), move |store| {
        let mut channels = store
            .list_managed_opt(None, false)
            .map_err(|e| e.to_string())?;
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
            msg.push_str(" â€” Plex cannot save a mapping this large (it puts every channel in the URL). Keep the Plex lineup under ~400, or use the IPTV card.");
        }
        Ok(msg)
    })
    .await
}

#[tauri::command]
async fn export_channels_json(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let picked = tauri_plugin_dialog::DialogExt::dialog(&app)
        .file()
        .add_filter("channels.json", &["json"])
        .set_file_name("channels")
        .blocking_save_file();
    let Some(file) = picked else {
        return Ok("cancelled".into());
    };
    let path = file.into_path().map_err(|e| e.to_string())?;
    with_store(Arc::clone(&state.store), move |store| {
        let settings = store.load_settings().map_err(|e| e.to_string())?;
        let channels = store.list_managed(None).map_err(|e| e.to_string())?;
        let cap = if settings.member_max_channels > 0 {
            settings.member_max_channels
        } else {
            members::DEFAULT_MAX_CHANNELS
        };
        let built = curation::build(&channels, VERSION, None, Some(cap));
        std::fs::write(&path, curation::to_json(&built.document)).map_err(|e| e.to_string())?;
        Ok(format!(
            "channels.json Â· {} unique tvg-id Â· {} empty skipped Â· {} dups",
            built.included, built.skipped_no_tvg_id, built.skipped_duplicate
        ))
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublishReport {
    ok: bool,
    text: String,
}

#[tauri::command]
async fn publish_channels(state: tauri::State<'_, AppState>) -> Result<PublishReport, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let (mut settings, channels) = {
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            let settings = s.load_settings().map_err(|e| e.to_string())?;
            let key = settings.member_access_key.trim().to_string();
            if key.is_empty() {
                return Err("Add your my.epg.monster access key in Settings first.".into());
            }
            let channels = s.list_managed(None).map_err(|e| e.to_string())?;
            (settings, channels)
        };
        let key = settings.member_access_key.trim().to_string();
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
            store
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .save_settings(&settings)
                .map_err(|e| e.to_string())?;
        }
        Ok(PublishReport {
            ok: result.ok,
            text: members::format_publish_report(&built, &result),
        })
    })
    .await
    .map_err(|e| e.to_string())?
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
async fn clear_managed(state: tauri::State<'_, AppState>) -> Result<(), String> {
    with_store(Arc::clone(&state.store), |s| {
        s.clear_managed().map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn load_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, String> {
    let root = app_root(&app);
    with_store(Arc::clone(&state.store), move |s| {
        let mut settings = s.load_settings().map_err(|e| e.to_string())?;
        let mut dirty = false;
        let fill = |cur: &mut String, path: std::path::PathBuf| {
            if std::path::Path::new(cur.trim()).is_file() {
                return false;
            }
            if path.is_file() {
                *cur = path.to_string_lossy().into_owned();
                true
            } else if !cur.trim().is_empty() {
                cur.clear();
                true
            } else {
                false
            }
        };
        dirty |= fill(&mut settings.mpv_path, default_mpv_path(&root));
        dirty |= fill(&mut settings.ffmpeg_path, default_ffmpeg_path(&root));
        dirty |= fill(&mut settings.ffprobe_path, default_ffprobe_path(&root));
        if !std::path::Path::new(settings.ffprobe_path.trim()).is_file() {
            if let Some(sib) =
                studio_core::tools::sibling_tool(std::path::Path::new(settings.ffmpeg_path.trim()), "ffprobe")
            {
                settings.ffprobe_path = sib.to_string_lossy().into_owned();
                dirty = true;
            }
        }
        dirty |= fill(&mut settings.vlc_path, default_vlc_path());
        if dirty {
            s.save_settings(&settings).map_err(|e| e.to_string())?;
        }
        Ok(settings)
    })
    .await
}

#[tauri::command]
fn save_settings(state: tauri::State<AppState>, mut settings: AppSettings) -> Result<(), String> {
    let snap = tuner_snapshot_fn(Arc::clone(&state.store));
    state
        .tuner
        .lock()
        .map_err(|e| e.to_string())?
        .apply(&mut settings, snap);
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioToolsStatus {
    ffmpeg: bool,
    ffprobe: bool,
    mpv: bool,
    vlc: bool,
}

fn tool_file_ok(stored: &str, fallback: std::path::PathBuf) -> bool {
    let t = stored.trim();
    if !t.is_empty() {
        let p = std::path::PathBuf::from(t);
        if p.is_file() {
            return true;
        }
    }
    fallback.is_file()
}

#[tauri::command]
fn studio_tools_status(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<StudioToolsStatus, String> {
    let root = app_root(&app);
    let settings = lock_store(&state)?
        .load_settings()
        .unwrap_or_default();
    Ok(StudioToolsStatus {
        ffmpeg: tool_file_ok(&settings.ffmpeg_path, default_ffmpeg_path(&root)),
        ffprobe: tool_file_ok(&settings.ffprobe_path, default_ffprobe_path(&root)),
        mpv: tool_file_ok(&settings.mpv_path, default_mpv_path(&root)),
        vlc: tool_file_ok(&settings.vlc_path, default_vlc_path()),
    })
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
async fn members_ping(api_base: String, access_key: String) -> members::MemberPingResult {
    tauri::async_runtime::spawn_blocking(move || {
        members::ping(&api_base, &access_key, Some(VERSION))
    })
    .await
    .unwrap_or_else(|e| members::MemberPingResult {
        ok: false,
        message: e.to_string(),
        ..members::MemberPingResult::default()
    })
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
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer")
                .arg(&dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open")
                .arg(&dir)
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
fn log_heartbeat(app: tauri::AppHandle) {
    let vis = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    studio_core::crash::append_log(
        "Trace",
        "Watch",
        &format!("heartbeat visible={vis} tray={}", !vis),
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
async fn post_issue(
    state: tauri::State<'_, AppState>,
    kind: String,
    title: String,
    summary: String,
    details: String,
    notes: Option<String>,
) -> Result<members::MemberIssueResult, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let (settings, count) = {
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            let settings = s.load_settings().map_err(|e| e.to_string())?;
            let count = s.managed_count().ok();
            (settings, count)
        };
        let key = settings.member_access_key.trim().to_string();
        if key.is_empty() {
            return Ok(members::MemberIssueResult {
                message: "Add your access key in Settings â†’ my.epg.monster first.".into(),
                ..members::MemberIssueResult::default()
            });
        }
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
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn epg_catalog_count(state: tauri::State<'_, AppState>) -> Result<i32, String> {
    with_store(Arc::clone(&state.store), |s| {
        s.catalog_count().map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn epg_guide_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    with_store(Arc::clone(&state.store), |s| {
        let st = s.load_settings().map_err(|e| e.to_string())?;
        Ok(epg::resolve_xml_urls(&st)
            .into_iter()
            .next()
            .unwrap_or_else(|| epg::DEFAULT_XML_URL.into()))
    })
    .await
}

#[tauri::command]
async fn fetch_epg_catalog(
    state: tauri::State<'_, AppState>,
    url: Option<String>,
) -> Result<String, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || fetch_epg_catalog_inner(&store, url))
        .await
        .map_err(|e| e.to_string())?
}

fn fetch_epg_catalog_inner(
    store: &Arc<Mutex<SqliteStore>>,
    url: Option<String>,
) -> Result<String, String> {
    let settings = {
        let g = store.lock().unwrap_or_else(|e| e.into_inner());
        let catalog = g.catalog_count().unwrap_or(0);
        let programmes = g.programme_count().unwrap_or(0);
        if url.as_deref().map(str::trim).unwrap_or("").is_empty() && catalog > 0 && programmes > 0 {
            return Ok(format!(
                "using cached {catalog} catalog ids Â· {programmes} programmes"
            ));
        }
        g.load_settings().map_err(|e| e.to_string())?
    };
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
        let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        if epg::xmltv_too_large(meta.len()) {
            let _ = std::fs::remove_file(&path);
            return Err(format!(
                "XMLTV from {host} is {} MB (max {} MB) â€” refused.",
                meta.len() / (1024 * 1024),
                epg::XMLTV_MAX_BYTES / (1024 * 1024)
            ));
        }
        if meta.len() <= epg::XMLTV_MAX_BYTES {
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            all_ch.extend(epg::parse_xmltv_channels(&text, &host));
        }
        all_prog.extend(epg::index_programmes_from_path(&path).map_err(|e| e.to_string())?);
    }
    let mut seen = std::collections::HashSet::new();
    all_ch.retain(|c| seen.insert(c.tvg_id.to_ascii_lowercase()));
    {
        let store = store.lock().unwrap_or_else(|e| e.into_inner());
        store.replace_epg_catalog(&all_ch).map_err(|e| e.to_string())?;
        store.replace_programmes(&all_prog).map_err(|e| e.to_string())?;
        store
            .touch_epg_cache_meta(true, true, etag.as_deref(), last_mod.as_deref())
            .map_err(|e| e.to_string())?;
    }
    Ok(format!(
        "{} catalog ids Â· {} programmes indexed",
        all_ch.len(),
        all_prog.len()
    ))
}

#[tauri::command]
async fn rebuild_now_playing(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || rebuild_now_playing_inner(&store))
        .await
        .map_err(|e| e.to_string())?
}

fn cache_xml_files() -> Vec<std::path::PathBuf> {
    let dirs = vec![app_data_directory().join("cache")];
    let mut files: Vec<(std::time::SystemTime, u64, std::path::PathBuf)> = Vec::new();
    for cache in dirs {
        let Ok(rd) = std::fs::read_dir(&cache) else {
            continue;
        };
        for p in rd.flatten().map(|e| e.path()) {
            if !p
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
            {
                continue;
            }
            let n = p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if n.contains("epgshare") {
                continue;
            }
            let Ok(meta) = std::fs::metadata(&p) else {
                continue;
            };
            if meta.len() == 0 || epg::xmltv_too_large(meta.len()) {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
            files.push((modified, meta.len(), p));
        }
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().take(1).map(|(_, _, p)| p).collect()
}

fn rebuild_now_playing_inner(store: &Arc<Mutex<SqliteStore>>) -> Result<String, String> {
    {
        let g = store.lock().unwrap_or_else(|e| e.into_inner());
        let covering = g.covering_now_count().unwrap_or(0);
        let fresh = g.load_epg_cache_meta().index_is_fresh(6 * 3600);
        if covering > 0 && fresh {
            let n = g
                .refresh_now_playing_snapshot()
                .map_err(|e| e.to_string())?;
            return Ok(format!("{n} on now (index still fresh)"));
        }
    }
    let files = cache_xml_files();
    if files.is_empty() {
        let g = store.lock().unwrap_or_else(|e| e.into_inner());
        let n = g
            .refresh_now_playing_snapshot()
            .map_err(|e| e.to_string())?;
        return Ok(format!(
            "no XML cache to re-window; {n} on now from stored programmes"
        ));
    }
    let mut all = Vec::new();
    for f in &files {
        all.extend(epg::index_programmes_from_path(f).map_err(|e| e.to_string())?);
    }
    let g = store.lock().unwrap_or_else(|e| e.into_inner());
    g.replace_programmes(&all).map_err(|e| e.to_string())?;
    g.touch_epg_cache_meta(false, true, None, None)
        .map_err(|e| e.to_string())?;
    let on = g.covering_now_count().unwrap_or(0);
    Ok(format!(
        "Reindexed {} programmes Â· {on} on now",
        all.len()
    ))
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
async fn epg_refresh_schedule(
    state: tauri::State<'_, AppState>,
    only_if_stale: bool,
) -> Result<String, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let (catalog, programmes, covering, usable, meta, settings) = {
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            let catalog = s.catalog_count().unwrap_or(0);
            let programmes = s.programme_count().unwrap_or(0);
            let covering = s.covering_now_count().unwrap_or(0);
            let usable = catalog > 0 && programmes > 0 && cache_has_xml();
            let meta = s.load_epg_cache_meta();
            let settings = s.load_settings().ok();
            (catalog, programmes, covering, usable, meta, settings)
        };
        if only_if_stale && catalog > 0 && programmes > 0 && covering > 0 {
            return Ok("skipped".into());
        }
        if !usable {
            fetch_epg_catalog_inner(&store, None)?;
            return Ok("downloaded".into());
        }
        let settings = settings.unwrap_or_default();
        let url = epg::resolve_xml_urls(&settings)
            .into_iter()
            .next()
            .unwrap_or_else(|| epg::DEFAULT_XML_URL.into());
        match epg::fetch_xmltv_conditional(&url, meta.etag.as_deref(), meta.last_modified.as_deref())
        {
            Ok(epg::FetchXmltv::NotModified) => {
                rebuild_now_playing_inner(&store)?;
                Ok("reindexed".into())
            }
            Ok(epg::FetchXmltv::Body { .. }) => {
                fetch_epg_catalog_inner(&store, None)?;
                Ok("downloaded".into())
            }
            Err(_) => {
                rebuild_now_playing_inner(&store)?;
                Ok("reindexed".into())
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn epg_audit(state: tauri::State<'_, AppState>) -> Result<Vec<EpgAuditRow>, String> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let (channels, catalog) = {
            let g = store.lock().unwrap_or_else(|e| e.into_inner());
            let channels = g.list_managed_opt(None, false).map_err(|e| e.to_string())?;
            let catalog = g.list_catalog_for_match().map_err(|e| e.to_string())?;
            (channels, catalog)
        };
        Ok(epg::build_epg_audit(&channels, &catalog))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn epg_apply(
    state: tauri::State<'_, AppState>,
    managed_id: String,
    tvg_id: String,
    logo: Option<String>,
    apply_logo: bool,
) -> Result<(), String> {
    with_store(Arc::clone(&state.store), move |store| {
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
    })
    .await
}

#[tauri::command]
async fn epg_auto_match(
    state: tauri::State<'_, AppState>,
    groups: Vec<String>,
    min_score: f64,
) -> Result<i32, String> {
    with_store(Arc::clone(&state.store), move |store| {
        let channels = store
            .list_managed_opt(None, false)
            .map_err(|e| e.to_string())?;
        let catalog = store.list_catalog_for_match().map_err(|e| e.to_string())?;
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
            if let Some(mut ch) = store
                .get_managed(&row.managed_channel_id)
                .map_err(|e| e.to_string())?
            {
                ch.tvg_id = row.suggested_tvg_id;
                store.upsert_managed(&ch).map_err(|e| e.to_string())?;
                applied += 1;
            }
        }
        Ok(applied)
    })
    .await
}

#[tauri::command]
async fn epg_browse_catalog(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
) -> Result<Vec<CatalogEntry>, String> {
    with_store(Arc::clone(&state.store), move |s| {
        s.list_catalog_page(query.as_deref(), 2_000)
            .map_err(|e| e.to_string())
    })
    .await
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoScanProgress {
    current: u32,
    total: u32,
    issues: u32,
    name: String,
}

#[tauri::command]
async fn logo_scan(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    probe: bool,
) -> Result<Vec<logo::LogoIssue>, String> {
    let channels = {
        let store = lock_store(&state)?;
        store.list_managed(None).map_err(|e| e.to_string())?
    };
    tauri::async_runtime::spawn_blocking(move || {
        let total = channels.len() as u32;
        let mut out = Vec::with_capacity(channels.len());
        let mut issues = 0u32;
        for (i, ch) in channels.into_iter().enumerate() {
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
            if !issue.issue.is_empty() {
                issues += 1;
            }
            if probe && (i == 0 || (i + 1) % 8 == 0 || i + 1 == total as usize) {
                let _ = app.emit(
                    "logo-scan-progress",
                    LogoScanProgress {
                        current: (i + 1) as u32,
                        total,
                        issues,
                        name: ch.name.clone(),
                    },
                );
            }
            out.push(issue);
        }
        logo::sort_issues(&mut out);
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn logo_preview_data(url: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || logo::preview_data_url(&url))
        .await
        .map_err(|e| e.to_string())?
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
async fn logo_set(
    state: tauri::State<'_, AppState>,
    managed_id: String,
    url: Option<String>,
) -> Result<(), String> {
    let trimmed = url.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| s.to_string());
    if let Some(ref u) = trimmed {
        reject_logo_url(u)?;
    }
    with_store(Arc::clone(&state.store), move |store| {
        let Some(mut ch) = store.get_managed(&managed_id).map_err(|e| e.to_string())? else {
            return Err("channel not found".into());
        };
        ch.tvg_logo = trimmed;
        store.upsert_managed(&ch).map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
async fn logo_batch_set(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
    url: String,
) -> Result<i32, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("Paste a logo URL first".into());
    }
    reject_logo_url(&url)?;
    with_store(Arc::clone(&state.store), move |store| {
        let mut n = 0;
        for id in ids {
            if let Some(mut ch) = store.get_managed(&id).map_err(|e| e.to_string())? {
                ch.tvg_logo = Some(url.clone());
                store.upsert_managed(&ch).map_err(|e| e.to_string())?;
                n += 1;
            }
        }
        Ok(n)
    })
    .await
}

#[tauri::command]
fn logo_default_dir() -> String {
    logo::default_logo_dir().to_string_lossy().into_owned()
}

#[tauri::command]
async fn logo_save_plan(
    state: tauri::State<'_, AppState>,
    root: Option<String>,
) -> Result<(String, Vec<logo::LogoSaveItem>), String> {
    with_store(Arc::clone(&state.store), move |store| {
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
        let channels = store
            .list_managed_opt(None, false)
            .map_err(|e| e.to_string())?;
        Ok((
            dir.to_string_lossy().into_owned(),
            logo::plan_save(&channels, &dir),
        ))
    })
    .await
}

#[tauri::command]
async fn logo_save_one(
    state: tauri::State<'_, AppState>,
    mut item: logo::LogoSaveItem,
) -> Result<logo::LogoSaveItem, String> {
    let ffmpeg = with_store(Arc::clone(&state.store), |s| {
        Ok(s.load_settings().map(|st| st.ffmpeg_path).unwrap_or_default())
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        logo::save_one(&mut item, &ffmpeg);
        Ok(item)
    })
    .await
    .map_err(|e| e.to_string())?
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
async fn audit_source_channel(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    url: String,
    source_id: Option<String>,
) -> Result<audit::AuditResult, String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Err("URL is empty.".into());
    }
    let store = Arc::clone(&state.store);
    let root = app_root(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let locked = store.lock().unwrap_or_else(|e| e.into_inner());
        let settings = locked.load_settings().map_err(|e| e.to_string())?;
        // Source headers only (Referer, etc.). Do not inject settings.default_user_agent
        // â€” that is the app identity, and IPTV CDNs often refuse it with I/O error.
        // http_input_opts defaults to the VLC player UA used by Play / logo probe.
        let headers: std::collections::BTreeMap<String, String> = source_id
            .and_then(|id| {
                locked.list_sources().ok()?.into_iter().find(|s| s.id == id).and_then(|s| {
                    serde_json::from_str(&s.headers_json).ok()
                })
            })
            .unwrap_or_default();
        let headers_ref = if headers.is_empty() {
            None
        } else {
            Some(&headers)
        };
        drop(locked);
        let pick = |stored: &str, fallback: std::path::PathBuf| {
            let t = stored.trim();
            if !t.is_empty() && std::path::Path::new(t).is_file() {
                t.to_string()
            } else {
                fallback.to_string_lossy().into_owned()
            }
        };
        let ffmpeg = pick(&settings.ffmpeg_path, default_ffmpeg_path(&root));
        let ffprobe = pick(&settings.ffprobe_path, default_ffprobe_path(&root));
        Ok(audit::probe_one_with(
            &ffmpeg,
            &ffprobe,
            &trimmed,
            settings.audit_timeout_ms.max(1000),
            settings.black_detect_enabled,
            headers_ref,
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn audit_snapshot(state: tauri::State<AppState>) -> Result<audit::AuditSnapshot, String> {
    let process = lock_audit(&state)?;
    audit::snapshot(&process).map_err(|e| e.to_string())
}

#[tauri::command]
async fn audit_begin(
    state: tauri::State<'_, AppState>,
    visible_only: bool,
    auto_swap: bool,
    channel_ids: Option<Vec<String>>,
) -> Result<audit::AuditJob, String> {
    let store = Arc::clone(&state.store);
    let process = Arc::clone(&state.audit);
    let job = tauri::async_runtime::spawn_blocking(move || {
        let s = store.lock().unwrap_or_else(|e| e.into_inner());
        let p = process.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(Some(job)) = p.load_job() {
            if job.state == "running" {
                return Err("Audit already running.".into());
            }
        }
        let mut settings = s.load_settings().map_err(|e| e.to_string())?;
        settings.auto_swap_on_audit_fail = auto_swap;
        s.save_settings(&settings).map_err(|e| e.to_string())?;
        audit::begin_job(
            &s,
            &p,
            &settings,
            auto_swap,
            visible_only,
            channel_ids.as_deref(),
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    spawn_audit_worker(Arc::clone(&state.store));
    Ok(job)
}

#[tauri::command]
fn audit_interrupt(kind: String) -> Result<(), String> {
    match kind.as_str() {
        "paused" => audit::request_interrupt(audit::Interrupt::Pause),
        "cancelled" => audit::request_interrupt(audit::Interrupt::Cancel),
        _ => return Err("unknown interrupt".into()),
    }
    Ok(())
}

#[tauri::command]
fn audit_next(state: tauri::State<AppState>) -> Result<audit::AuditStep, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    let process = lock_audit(&state)?;
    audit::next_step(&state.store, &process, &settings).map_err(|e| e.to_string())
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
            process.update_job(&job).map_err(|e| e.to_string())?;
            drop(process);
            spawn_audit_worker(state.store.clone());
            return Ok(Some(job));
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
    join_audit_worker();
    lock_audit(&state)?.clear().map_err(|e| e.to_string())?;
    lock_store(&state)?
        .clear_audit_results()
        .map_err(|e| e.to_string())
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

fn tuner_snapshot_fn(store: Arc<Mutex<SqliteStore>>) -> Arc<dyn Fn() -> TunerSnapshot + Send + Sync> {
    Arc::new(move || {
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
    })
}

fn make_snapshot(store: &SqliteStore) -> TunerSnapshot {
    let settings = store.load_settings().unwrap_or_default();
    let channels = store.list_managed(None).unwrap_or_default();
    let ids: Vec<String> = studio_core::lineup::ordered_lineup(&channels)
        .into_iter()
        .map(|c| studio_core::hdhr::channel_xml_id(&c))
        .filter(|s| !s.is_empty())
        .collect();
    let programmes = store.list_programmes_nearby(&ids).unwrap_or_default();
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
    let snap = tuner_snapshot_fn(Arc::clone(&state.store));
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
async fn tuner_self_test(state: tauri::State<'_, AppState>) -> Result<SelfTestDto, String> {
    let settings = with_store(Arc::clone(&state.store), |s| {
        s.load_settings().map_err(|e| e.to_string())
    })
    .await?;
    let statuses = state
        .tuner
        .lock()
        .map_err(|e| e.to_string())?
        .all_statuses(&settings);
    tauri::async_runtime::spawn_blocking(move || {
        let (reports, json) = manager::self_test(&statuses);
        let path = studio_core::paths::app_data_directory()
            .join("tunertest.json")
            .to_string_lossy()
            .into_owned();
        Ok(SelfTestDto {
            json,
            path,
            reports,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn tuner_logs(state: tauri::State<AppState>) -> Result<Vec<manager::TunerLogLine>, String> {
    Ok(state.tuner.lock().map_err(|e| e.to_string())?.logs().to_vec())
}

#[tauri::command]
fn tuner_clear_logs(state: tauri::State<AppState>) -> Result<(), String> {
    state.tuner.lock().map_err(|e| e.to_string())?.clear_logs();
    Ok(())
}

#[tauri::command]
fn tuner_graphs(state: tauri::State<AppState>) -> Result<Vec<manager::TunerGraphRow>, String> {
    let settings = lock_store(&state)?.load_settings().map_err(|e| e.to_string())?;
    Ok(state.tuner.lock().map_err(|e| e.to_string())?.graph_rows(&settings))
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
    studio_core::paths::configure_from_launch_dir();
    std::panic::set_hook(Box::new(|info| {
        studio_core::crash::append_log("Fatal", "Panic", &info.to_string());
    }));
    studio_core::crash::append_log("Info", "App", "run() starting");
    studio_core::crash::write_session_lock("running");
    let data = app_data_directory();
    let dropped = studio_core::epg::purge_oversized_xmltv(&data.join("cache"));
    if dropped > 0 {
        studio_core::crash::append_log(
            "Warn",
            "App",
            &format!("deleted {dropped} oversized XMLTV cache file(s)"),
        );
    }
    let db = database_path();
    studio_core::crash::append_log("Info", "App", &format!("opening {}", db.display()));
    let store = match SqliteStore::open(&db) {
        Ok(s) => s,
        Err(e) => studio_core::crash::startup_fatal(
            "Could not open the studio database.",
            &format!("{}\n{e}", db.display()),
        ),
    };
    studio_core::crash::append_log("Info", "App", "database open");
    let audit_store = match audit::ProcessStore::open(None) {
        Ok(s) => s,
        Err(e) => studio_core::crash::startup_fatal(
            "Could not open the stream-audit database.",
            &e.to_string(),
        ),
    };
    studio_core::crash::append_log("Info", "App", "audit process store open");
    let store = Arc::new(Mutex::new(store));
    let mut tuner = TunerManager::new();
    // if-let keeps a MutexGuard alive for the whole block; a nested lock deadlocks.
    let loaded = store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .load_settings();
    if let Ok(mut settings) = loaded {
        studio_core::crash::append_log("Info", "App", "settings loaded");
        tuner.apply(&mut settings, tuner_snapshot_fn(Arc::clone(&store)));
        studio_core::crash::append_log("Info", "App", "tuner apply done");
        let _ = store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .save_settings(&settings);
    }
    studio_core::crash::append_log("Info", "App", "starting window");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            store,
            audit: Arc::new(Mutex::new(audit_store)),
            tuner: Mutex::new(tuner),
        })
        .setup(|app| {
            studio_core::crash::append_log("Info", "App", "OnLaunched");
            if let Some(w) = app.get_webview_window("main") {
                apply_window_chrome(&w, true);
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.center();
                let _ = w.set_always_on_top(true);
                let _ = w.set_focus();
                let _ = w.set_always_on_top(false);
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
            if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/mascot.png")) {
                tray = tray.icon(icon);
            } else if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let _ = tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
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
            check_studio_update,
            open_latest_release,
            host_info,
            check_github_issues,
            promote_main_window,
            open_epg_catalog_window,
            detect_bundled_tools,
            studio_tools_status,
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
            add_source_xtream,
            probe_xtream_expiry,
            update_source,
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
            add_missing_from_source,
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
            tuner_clear_logs,
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
            logo_preview_data,
            logo_set,
            logo_batch_set,
            logo_default_dir,
            logo_save_plan,
            logo_save_one,
            logo_save_tracker,
            logo_search_urls,
            audit_source_channel,
            audit_snapshot,
            audit_begin,
            audit_interrupt,
            audit_next,
            audit_set_state,
            audit_discard,
            audit_undo,
            audit_today_groups,
            audit_mark_today_ran,
            audit_results,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            studio_core::crash::startup_fatal(
                "epg.monster studio could not start the window.",
                &e.to_string(),
            )
        });
}
