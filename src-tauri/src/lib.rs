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
use studio_core::{DISPLAY_NAME, VERSION};
use studio_tuner::manager::{self, TunerManager};
use studio_tuner::host::TunerSnapshot;
use tauri::Manager;
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

fn lock_audit<'a>(
    state: &'a tauri::State<'a, AppState>,
) -> Result<std::sync::MutexGuard<'a, audit::ProcessStore>, String> {
    state.audit.lock().map_err(|e| e.to_string())
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
    for u in &urls {
        let bytes = epg::fetch_xmltv(u)?;
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
    lock_store(&state)?
        .replace_programmes(&all)
        .map_err(|e| e.to_string())?;
    Ok(format!("Reindexed {} programmes from cache", all.len()))
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
    manager::snapshot_from_settings(channels, programmes, &settings)
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
            Some(s) => make_snapshot(&s),
            None => TunerSnapshot {
                channels: vec![],
                programmes: vec![],
                remux: true,
                epg_url: None,
                host_logos: false,
                use_local_logos: false,
                logo_root: String::new(),
                video_codec: "H264".into(),
                audio_codec: "AAC".into(),
                ffmpeg_path: String::new(),
            },
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
            epg_catalog_count,
            epg_guide_url,
            fetch_epg_catalog,
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
