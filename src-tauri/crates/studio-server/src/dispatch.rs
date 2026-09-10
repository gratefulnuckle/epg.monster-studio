// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use studio_core::audit;
use studio_core::epg;
use studio_core::export::{export_all, export_visible_only};
use studio_core::logo;
use studio_core::members;
use studio_core::models::{ChannelEntry, ManagedChannel, PlaylistSource};

fn source_dto(s: PlaylistSource) -> Value {
    json!({
        "id": s.id,
        "name": s.name,
        "kind": s.kind,
        "location": s.location,
        "headersJson": s.headers_json,
        "channelCount": s.channel_count,
        "expiresAt": s.expires_at,
    })
}

fn channel_dto(c: ChannelEntry) -> Value {
    json!({
        "id": c.id,
        "sourceId": c.source_id,
        "groupTitle": c.group_title,
        "name": c.name,
        "tvgId": c.tvg_id,
        "tvgLogo": c.tvg_logo,
        "url": c.url,
    })
}
use crate::auth;
use studio_core::paths::{
    app_data_directory, crashes_directory, current_log_path, database_path, logs_directory,
    offline_slates_directory,
};
use studio_core::settings::AppSettings;
use studio_core::store::SqliteStore;
use studio_core::tools::{
    default_ffmpeg_path, default_ffprobe_path, default_mpv_path, default_vlc_path,
};
use studio_core::{
    display_version, latest_github_release, remote_is_newer, DISPLAY_NAME, EDITION,
    GITHUB_RELEASES_LATEST, VERSION,
};
use studio_tuner::manager::TunerManager;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct Host {
    pub store: Arc<Mutex<SqliteStore>>,
    pub audit: Arc<Mutex<audit::ProcessStore>>,
    pub tuner: Arc<Mutex<TunerManager>>,
    pub events: broadcast::Sender<String>,
    pub root: PathBuf,
}

impl Host {
    fn emit(&self, event: &str, payload: &impl serde::Serialize) {
        let line = json!({ "event": event, "payload": payload }).to_string();
        let _ = self.events.send(line);
    }

    fn store(&self) -> Result<std::sync::MutexGuard<'_, SqliteStore>, String> {
        Ok(self.store.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

fn ok<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn arg<T: DeserializeOwned>(args: &Value, names: &[&str]) -> Result<T, String> {
    for n in names {
        if let Some(v) = args.get(*n) {
            if !v.is_null() {
                return serde_json::from_value(v.clone()).map_err(|e| format!("{n}: {e}"));
            }
        }
    }
    serde_json::from_value(Value::Null).map_err(|e| e.to_string())
}

fn opt<T: DeserializeOwned>(args: &Value, names: &[&str]) -> Option<T> {
    arg(args, names).ok()
}

pub async fn invoke(host: &Host, cmd: &str, args: Value) -> Result<Value, String> {
    let host = host.clone();
    let cmd = cmd.to_string();
    tokio::task::spawn_blocking(move || invoke_sync(&host, &cmd, args))
        .await
        .map_err(|e| e.to_string())?
}

fn invoke_sync(host: &Host, cmd: &str, args: Value) -> Result<Value, String> {
    match cmd {
        "get_studio_info" => {
            let n = host.store()?.managed_count().unwrap_or(0);
            ok(json!({
                "version": VERSION,
                "displayVersion": display_version(),
                "displayName": DISPLAY_NAME,
                "edition": EDITION,
                "databasePath": database_path().to_string_lossy(),
                "managedCount": n,
            }))
        }
        "host_info" => ok(json!({
            "os": if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
            "arch": std::env::consts::ARCH,
            "host": studio_core::bootstrap::current_host(),
            "exeSuffix": if cfg!(windows) { ".exe" } else { "" },
        })),
        "promote_main_window" | "mark_tray_state" | "mark_clean_exit" | "log_heartbeat" => {
            Ok(Value::Null)
        }
        "open_epg_catalog_window" | "open_source_search_window" => Ok(Value::Null),
        "splash_checks" => splash_checks(host),
        "splash_epg_status" => {
            let s = host.store()?;
            ok(json!({
                "catalog": s.catalog_count().unwrap_or(0),
                "programmes": s.programme_count().unwrap_or(0),
            }))
        }
        "check_studio_update" | "check_app_update" => check_update(),
        "apply_studio_update" => ok(studio_core::update::stage_update_and_relaunch()?),
        "open_latest_release" => {
            let url = latest_github_release()
                .ok()
                .map(|r| r.html_url)
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| GITHUB_RELEASES_LATEST.to_string());
            Ok(json!(url))
        }
        "check_github_issues" => {
            let (n, title) = studio_core::github_open_studio_issues().unwrap_or((0, None));
            let detail = match (n, title) {
                (0, _) => "0 open".into(),
                (1, Some(t)) => format!("1 open · {t}"),
                (n, Some(t)) => format!("{n} open · {t}"),
                (n, None) => format!("{n} open"),
            };
            ok(json!({ "label": "GitHub open issues", "ok": true, "detail": detail }))
        }
        "detect_bundled_tools" => {
            Ok(json!(studio_core::tools::detect_bundled(&host.root).len() as i64))
        }
        "studio_tools_status" => {
            let settings = host.store()?.load_settings().unwrap_or_default();
            let root = &host.root;
            let file_ok = |stored: &str, fb: PathBuf| {
                let t = stored.trim();
                if !t.is_empty() && std::path::Path::new(t).is_file() {
                    true
                } else {
                    fb.is_file()
                }
            };
            ok(json!({
                "ffmpeg": file_ok(&settings.ffmpeg_path, default_ffmpeg_path(root)),
                "ffprobe": file_ok(&settings.ffprobe_path, default_ffprobe_path(root)),
                "mpv": file_ok(&settings.mpv_path, default_mpv_path(root)),
                "vlc": file_ok(&settings.vlc_path, default_vlc_path()),
            }))
        }
        "tools_missing" => ok(studio_core::bootstrap::missing_tools(&host.root).unwrap_or_default()),
        "tools_ensure" => {
            studio_core::bootstrap::ensure(&host.root, |_| {}).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "list_sources" => {
            let rows = host.store()?.list_sources().map_err(|e| e.to_string())?;
            ok(rows.into_iter().map(source_dto).collect::<Vec<_>>())
        }
        "list_groups" => {
            let sid: String = arg(&args, &["sourceId", "source_id"])?;
            let rows = host.store()?.groups_with_counts(&sid).map_err(|e| e.to_string())?;
            ok(rows
                .into_iter()
                .map(|(title, count)| json!({ "title": title, "count": count }))
                .collect::<Vec<_>>())
        }
        "list_channels" => {
            let sid: String = arg(&args, &["sourceId", "source_id"])?;
            let group: String = arg(&args, &["group", "groupTitle"])?;
            let limit: i32 = arg(&args, &["limit"]).unwrap_or(5000);
            let rows = host
                .store()?
                .channels_by_group(&sid, &group, limit)
                .map_err(|e| e.to_string())?;
            ok(rows.into_iter().map(channel_dto).collect::<Vec<_>>())
        }
        "search_sources" => {
            let q: String = arg(&args, &["query"]).unwrap_or_default();
            ok(host
                .store()?
                .search_sources(&q)
                .map_err(|e| e.to_string())?
                .into_iter()
                .map(channel_dto)
                .collect::<Vec<_>>())
        }
        "remove_source" => {
            let id: String = arg(&args, &["id", "sourceId"])?;
            host.store()?.remove_source(&id).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "pick_source_file" | "pick_playlist_path" => {
            Err("WEB_PICK:open:m3u,m3u8,txt".into())
        }
        "add_source_file" => {
            let path: String = arg(&args, &["path"])
                .or_else(|_| arg(&args.get("args").unwrap_or(&Value::Null), &["path"]))?;
            let name: Option<String> = opt(&args, &["name"]);
            let p = PathBuf::from(path.trim().trim_matches('"'));
            let src = if let Some(n) = name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                host.store()?
                    .add_file_source_named(&p, Some(n))
                    .map_err(|e| e.to_string())?
            } else {
                host.store()?.add_file_source(&p).map_err(|e| e.to_string())?
            };
            host.emit("source-progress", &json!({
                "id": src.id, "name": src.name, "channelCount": src.channel_count,
                "done": true, "error": null, "op": "add"
            }));
            ok(source_dto(src))
        }
        "add_source_url" => {
            let inner = args.get("args").cloned().unwrap_or(args.clone());
            let url: String = arg(&inner, &["url"])?;
            let name: Option<String> = opt(&inner, &["name"]);
            let headers: BTreeMap<String, String> = opt(&inner, &["headers"]).unwrap_or_default();
            let cache = app_data_directory().join("cache");
            let src = host
                .store()?
                .add_url_source(&url, name.as_deref(), &headers, &cache)
                .map_err(|e| e.to_string())?;
            host.emit("source-progress", &json!({
                "id": src.id, "name": src.name, "channelCount": src.channel_count,
                "done": true, "error": null, "op": "add"
            }));
            ok(source_dto(src))
        }
        "add_source_xtream" => {
            let inner = args.get("args").cloned().unwrap_or(args.clone());
            let server: String = arg(&inner, &["server"])?;
            let username: String = arg(&inner, &["username"])?;
            let password: String = arg(&inner, &["password"])?;
            let output: String = opt(&inner, &["output"]).unwrap_or_else(|| "ts".into());
            let name: Option<String> = opt(&inner, &["name"]);
            let headers: BTreeMap<String, String> = opt(&inner, &["headers"]).unwrap_or_default();
            let cache = app_data_directory().join("cache");
            let src = host
                .store()?
                .add_xtream_source(
                    &server,
                    &username,
                    &password,
                    &output,
                    name.as_deref(),
                    &headers,
                    &cache,
                )
                .map_err(|e| e.to_string())?;
            ok(source_dto(src))
        }
        "probe_xtream_expiry" => {
            let server: String = arg(&args, &["server"])?;
            let username: String = arg(&args, &["username"])?;
            let password: String = arg(&args, &["password"])?;
            Ok(json!(studio_core::xtream::fetch_exp_date(
                &server,
                &username,
                &password,
                &BTreeMap::new(),
            )))
        }
        "update_source" => {
            let inner = args.get("args").cloned().unwrap_or(args.clone());
            let id: String = arg(&inner, &["id"])?;
            let name: String = arg(&inner, &["name"])?;
            let kind: String = arg(&inner, &["kind"])?;
            let location: String = arg(&inner, &["location"])?;
            let headers: BTreeMap<String, String> = opt(&inner, &["headers"]).unwrap_or_default();
            let hj = serde_json::to_string(&headers).unwrap_or_else(|_| "{}".into());
            let src = host
                .store()?
                .update_source_meta(&id, &name, &kind, &location, &hj)
                .map_err(|e| e.to_string())?;
            ok(source_dto(src))
        }
        "play_url" => {
            // Web host: the browser plays. Never spawn mpv/VLC on this machine.
            Ok(Value::Null)
        }
        "refresh_source" => {
            let id: String = arg(&args, &["sourceId", "source_id", "id"])?;
            let cache = app_data_directory().join("cache");
            let src = host
                .store()?
                .refresh_source(&id, &cache)
                .map_err(|e| e.to_string())?;
            host.emit("source-progress", &json!({
                "id": src.id, "name": src.name, "channelCount": src.channel_count,
                "done": true, "error": null, "op": "refresh"
            }));
            ok(source_dto(src))
        }
        "add_backup_from_source" => {
            let mid: String = arg(&args, &["managedId", "managed_id"])?;
            let eid: String = arg(&args, &["entryId", "entry_id"])?;
            ok(host
                .store()?
                .add_backup_from_entry(&mid, &eid)
                .map_err(|e| e.to_string())?)
        }
        "managed_count" => ok(host.store()?.managed_count().map_err(|e| e.to_string())?),
        "list_managed_groups" => {
            let rows = host.store()?.managed_groups().map_err(|e| e.to_string())?;
            ok(rows
                .into_iter()
                .map(|(title, count)| json!({ "title": title, "count": count }))
                .collect::<Vec<_>>())
        }
        "list_managed" => {
            let group: Option<String> = opt(&args, &["group"]);
            let hydrate: Option<bool> = opt(&args, &["hydrate"]);
            ok(host
                .store()?
                .list_managed_opt(group.as_deref(), hydrate.unwrap_or(false))
                .map_err(|e| e.to_string())?)
        }
        "get_managed" => {
            let id: String = arg(&args, &["id"])?;
            ok(host.store()?.get_managed(&id).map_err(|e| e.to_string())?)
        }
        "save_managed" => {
            let ch: ManagedChannel = arg(&args, &["channel"])?;
            let primary: Option<String> = opt(&args, &["primaryUrl", "primary_url"]);
            host.store()?
                .save_managed_channel(&ch, primary.as_deref())
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "delete_managed" => {
            let id: String = arg(&args, &["id"])?;
            host.store()?.delete_managed(&id).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "delete_managed_group" => {
            let g: String = arg(&args, &["group"])?;
            ok(host.store()?.delete_managed_group(&g).map_err(|e| e.to_string())?)
        }
        "reorder_managed_groups" => {
            let titles: Vec<String> = arg(&args, &["titles"])?;
            host.store()?
                .reorder_managed_groups(&titles)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "reorder_managed_channels" => {
            let group: String = arg(&args, &["group"])?;
            let ids: Vec<String> = arg(&args, &["ids"])?;
            host.store()?
                .reorder_managed_channels(&group, &ids)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "set_channel_hidden" => {
            let id: String = arg(&args, &["id"])?;
            let hidden: bool = arg(&args, &["hidden"])?;
            host.store()?
                .set_channel_hidden(&id, hidden)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "set_group_hidden" => {
            let g: String = arg(&args, &["group"])?;
            let hidden: bool = arg(&args, &["hidden"])?;
            host.store()?
                .set_group_hidden(&g, hidden)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "rename_managed_group" => {
            let old: String = arg(&args, &["oldName", "old_name"])?;
            let new: String = arg(&args, &["newName", "new_name"])?;
            ok(host
                .store()?
                .rename_managed_group(&old, &new)
                .map_err(|e| e.to_string())?)
        }
        "apply_managed_group_case" => {
            let g: String = arg(&args, &["group"])?;
            let upper: bool = arg(&args, &["upper"])?;
            ok(host
                .store()?
                .apply_managed_group_case(&g, upper)
                .map_err(|e| e.to_string())?)
        }
        "add_stream" => {
            let mid: String = arg(&args, &["managedId", "managed_id"])?;
            let url: String = arg(&args, &["url"])?;
            let label: Option<String> = opt(&args, &["label"]);
            ok(host
                .store()?
                .add_stream(&mid, &url, label.as_deref())
                .map_err(|e| e.to_string())?)
        }
        "delete_variant" => {
            let id: String = arg(&args, &["id"])?;
            host.store()?.delete_variant(&id).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "move_variant" => {
            let mid: String = arg(&args, &["managedId", "managed_id"])?;
            let vid: String = arg(&args, &["variantId", "variant_id"])?;
            let delta: i32 = arg(&args, &["delta"]).unwrap_or(0);
            let mut vars = host.store()?.get_variants(&mid).map_err(|e| e.to_string())?;
            let i = vars.iter().position(|v| v.id == vid).ok_or("variant")?;
            let j = (i as i32 + delta).clamp(0, vars.len() as i32 - 1) as usize;
            vars.swap(i, j);
            let ids: Vec<String> = vars.into_iter().map(|v| v.id).collect();
            host.store()?
                .apply_variant_order(&mid, &ids)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "suggest_tvg" => {
            let q: String = arg(&args, &["query"]).unwrap_or_default();
            ok(host.store()?.suggest_tvg(&q).map_err(|e| e.to_string())?)
        }
        "now_playing" => {
            let tvg: String = arg(&args, &["tvgId", "tvg_id"])?;
            let shift: f64 = arg(&args, &["shiftHours", "shift_hours"]).unwrap_or(0.0);
            ok(host
                .store()?
                .now_playing(&tvg, shift)
                .map_err(|e| e.to_string())?)
        }
        "is_known_tvg" => {
            let tvg: Option<String> = opt(&args, &["tvgId", "tvg_id"]);
            Ok(json!(host.store()?.is_known_tvg_id(tvg.as_deref())))
        }
        "add_from_source" => {
            let id: String = arg(&args, &["entryId", "entry_id"])?;
            ok(host
                .store()?
                .add_from_source_entry(&id)
                .map_err(|e| e.to_string())?)
        }
        "add_missing_from_source" => {
            let ids: Vec<String> = arg(&args, &["entryIds", "entry_ids"])?;
            let label: Option<String> = opt(&args, &["sourceLabel", "source_label"]);
            let (added, skipped) = host
                .store()?
                .add_missing_from_source_entries(&ids, label.as_deref())
                .map_err(|e| e.to_string())?;
            Ok(json!(format!(
                "Added {added} new channel(s); skipped {skipped} already managed"
            )))
        }
        "import_curated" => {
            let path: Option<String> = opt(&args, &["path"]);
            let Some(path) = path else {
                return Err("WEB_PICK:open:m3u,m3u8,txt".into());
            };
            let replace: bool = arg(&args, &["replace"]).unwrap_or(false);
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let label = std::path::Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("curated");
            let (added, skipped) = host
                .store()?
                .import_curated(&content, replace, label)
                .map_err(|e| e.to_string())?;
            Ok(json!(format!(
                "Imported +{added} channels ({skipped} skipped as duplicates)"
            )))
        }
        "export_managed" => {
            let include_backups: bool = arg(&args, &["includeBackups", "include_backups"]).unwrap_or(false);
            let channels = host.store()?.list_managed(None).map_err(|e| e.to_string())?;
            let body = if include_backups {
                export_all(&channels)
            } else {
                export_visible_only(&channels)
            };
            Ok(json!({ "__download": true, "name": "playlist.m3u8", "body": body }))
        }
        "export_channels_json" => {
            let store = host.store()?;
            let settings = store.load_settings().map_err(|e| e.to_string())?;
            let channels = store.list_managed(None).map_err(|e| e.to_string())?;
            let cap = if settings.member_max_channels > 0 {
                settings.member_max_channels
            } else {
                members::DEFAULT_MAX_CHANNELS
            };
            let built = studio_core::curation::build(&channels, VERSION, None, Some(cap));
            let body = studio_core::curation::to_json(&built.document);
            Ok(json!({ "__download": true, "name": "channels.json", "body": body }))
        }
        "output_summary" => {
            let n = host.store()?.managed_count().map_err(|e| e.to_string())?;
            ok(json!({ "managedCount": n }))
        }
        "clear_managed" => {
            host.store()?.clear_managed().map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "load_settings" => {
            let mut settings = host.store()?.load_settings().map_err(|e| e.to_string())?;
            if !std::path::Path::new(settings.ffmpeg_path.trim()).is_file() {
                settings.ffmpeg_path = default_ffmpeg_path(&host.root).to_string_lossy().into_owned();
            }
            ok(settings)
        }
        "save_settings" => {
            let settings: AppSettings = arg(&args, &["settings"])?;
            host.store()?
                .save_settings(&settings)
                .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "detect_tool_paths" => ok(json!({
            "mpv": default_mpv_path(&host.root).to_string_lossy(),
            "vlc": default_vlc_path().to_string_lossy(),
            "ffmpeg": default_ffmpeg_path(&host.root).to_string_lossy(),
            "ffprobe": default_ffprobe_path(&host.root).to_string_lossy(),
        })),
        "members_ping" => {
            let api: String = arg(&args, &["apiBase", "api_base"]).unwrap_or_default();
            let key: String = arg(&args, &["accessKey", "access_key"]).unwrap_or_default();
            ok(members::ping(&api, &key, Some(VERSION)))
        }
        "create_api_key" => {
            let name: Option<String> = opt(&args, &["name"]);
            ok(auth::create_api_key(name.as_deref())?)
        }
        "list_api_keys" => ok(auth::list_api_keys()),
        "revoke_api_key" => {
            let id: String = arg(&args, &["id"])?;
            auth::revoke_api_key(&id)?;
            Ok(Value::Null)
        }
        "settings_folders" => ok(json!({
            "logs": logs_directory().to_string_lossy(),
            "crashes": crashes_directory().to_string_lossy(),
            "slates": offline_slates_directory().to_string_lossy(),
            "currentLog": current_log_path().to_string_lossy(),
            "logoDir": logo::default_logo_dir().to_string_lossy(),
            "data": app_data_directory().to_string_lossy(),
        })),
        "list_slates" => {
            let dir = offline_slates_directory();
            let mut names = Vec::new();
            if let Ok(rd) = std::fs::read_dir(dir) {
                for e in rd.flatten() {
                    if e.path().is_file() {
                        if let Some(n) = e.file_name().to_str() {
                            names.push(n.to_string());
                        }
                    }
                }
            }
            ok(names)
        }
        "add_slate" => Err("WEB_PICK:open:png,jpg,jpeg".into()),
        "remove_slate" => {
            let name: String = arg(&args, &["name"])?;
            let p = offline_slates_directory().join(name);
            let _ = std::fs::remove_file(p);
            Ok(Value::Null)
        }
        "open_folder" => Ok(Value::Null),
        "consume_pending_crash" => ok(studio_core::crash::consume_pending_crash()),
        "write_crash_report" => {
            let kind: String = opt(&args, &["kind"]).unwrap_or_else(|| "managed".into());
            let title: String = opt(&args, &["title"]).unwrap_or_default();
            let summary: String = opt(&args, &["summary"]).unwrap_or_default();
            let details: String = opt(&args, &["details"]).unwrap_or_default();
            studio_core::crash::write_crash_report(&kind, &title, &summary, &details, "Web");
            Ok(Value::Null)
        }
        "post_issue" => ok(json!({ "ok": false, "message": "GitHub post is desktop-only in this build." })),
        "epg_catalog_count" => ok(host.store()?.catalog_count().map_err(|e| e.to_string())?),
        "epg_guide_url" => {
            let settings = host.store()?.load_settings().unwrap_or_default();
            Ok(json!(epg::resolve_xml_urls(&settings)
                .into_iter()
                .next()
                .unwrap_or_else(|| epg::DEFAULT_XML_URL.into())))
        }
        "epg_search_images_url" => {
            let name: String = opt(&args, &["name"]).unwrap_or_default();
            let q = if name.trim().is_empty() {
                "channel logo".into()
            } else {
                format!("{} logo", name.trim())
            };
            Ok(json!(epg::google_images_transparent_url(&q)))
        }
        "rebuild_now_playing" => {
            let n = host
                .store()?
                .refresh_now_playing_snapshot()
                .map_err(|e| e.to_string())?;
            Ok(json!(format!("Reindexed {n} now-playing rows")))
        }
        "epg_audit" => {
            let s = host.store()?;
            let channels = s.list_managed_opt(None, false).map_err(|e| e.to_string())?;
            let catalog = s.list_catalog_for_match().map_err(|e| e.to_string())?;
            ok(epg::build_epg_audit(&channels, &catalog))
        }
        "epg_apply" => {
            let id: String = arg(&args, &["managedId", "managed_id"])?;
            let tvg: String = arg(&args, &["tvgId", "tvg_id"])?;
            let mut ch = host
                .store()?
                .get_managed(&id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "channel not found".to_string())?;
            ch.tvg_id = Some(tvg);
            host.store()?.upsert_managed(&ch).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "epg_auto_match" => Ok(json!(0)),
        "epg_browse_catalog" => {
            let _q: Option<String> = opt(&args, &["query", "q"]);
            ok(host.store()?.list_catalog().map_err(|e| e.to_string())?.into_iter().take(200).collect::<Vec<_>>())
        }
        "epg_refresh_schedule" | "fetch_epg_catalog" => {
            Ok(json!("skipped"))
        }
        "logo_scan" => {
            let probe: bool = arg(&args, &["probe"]).unwrap_or(false);
            let channels = host.store()?.list_managed(None).map_err(|e| e.to_string())?;
            let mut out = Vec::with_capacity(channels.len());
            for ch in channels {
                let mut issue = logo::classify_channel(&ch);
                if probe && issue.issue.is_empty() {
                    if let Some(url) = ch.tvg_logo.as_deref() {
                        let check = logo::probe_url(url);
                        if !check.is_ok() {
                            issue.issue = check.issue.filter(|s| !s.is_empty()).unwrap_or_else(|| "broken".into());
                            issue.reason = check.reason;
                        }
                    }
                }
                out.push(issue);
            }
            logo::sort_issues(&mut out);
            ok(out)
        }
        "logo_set" => {
            let id: String = arg(&args, &["managedId", "managed_id"])?;
            let url: Option<String> = opt(&args, &["url"]);
            let mut ch = host
                .store()?
                .get_managed(&id)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "channel not found".to_string())?;
            ch.tvg_logo = url.filter(|s| !s.trim().is_empty());
            host.store()?.upsert_managed(&ch).map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        "logo_batch_set" => {
            let ids: Vec<String> = arg(&args, &["ids"])?;
            let url: String = arg(&args, &["url"])?;
            let mut n = 0;
            for id in ids {
                if let Ok(Some(mut ch)) = host.store()?.get_managed(&id) {
                    ch.tvg_logo = Some(url.clone());
                    host.store()?.upsert_managed(&ch).ok();
                    n += 1;
                }
            }
            Ok(json!(n))
        }
        "logo_default_dir" => Ok(json!(logo::default_logo_dir().to_string_lossy())),
        "logo_search_urls" => {
            let name: String = opt(&args, &["name"]).unwrap_or_default();
            let q = format!("{} logo", name.trim());
            Ok(json!([
                epg::google_images_transparent_url(&q),
                format!("https://duckduckgo.com/?q={}&iax=images&ia=images", urlencoding_lite(&q)),
                format!("https://github.com/tv-logo/tv-logos/search?q={}", urlencoding_lite(&name)),
            ]))
        }
        "logo_preview_data" | "logo_save_plan" | "logo_save_one" | "logo_save_tracker" => {
            Ok(Value::Null)
        }
        "audit_snapshot" => {
            let g = host.audit.lock().map_err(|e| e.to_string())?;
            ok(audit::snapshot(&g).map_err(|e| e.to_string())?)
        }
        "audit_source_channel"
        | "audit_begin"
        | "audit_interrupt"
        | "audit_next"
        | "audit_set_state"
        | "audit_discard"
        | "audit_undo"
        | "audit_today_groups"
        | "audit_mark_today_ran"
        | "audit_results" => Ok(Value::Null),
        "tuner_statuses" => {
            let settings = host.store()?.load_settings().unwrap_or_default();
            let st = host.tuner.lock().map_err(|e| e.to_string())?;
            ok(st.all_statuses(&settings))
        }
        "tuner_logs" => ok(host.tuner.lock().map_err(|e| e.to_string())?.logs().to_vec()),
        "tuner_clear_logs" => {
            host.tuner.lock().map_err(|e| e.to_string())?.clear_logs();
            Ok(Value::Null)
        }
        "tuner_start" | "tuner_stop" | "tuner_start_all" | "tuner_stop_all" | "tuner_set_max"
        | "tuner_self_test" | "tuner_graphs" | "tuner_help" | "lineup_candidates"
        | "save_tuner_lineup" | "publish_channels" => Ok(Value::Null),
        c if c.starts_with("ghoul_") => Ok(json!({ "canMount": false, "err": "IPTV Player embed is desktop-only. Play still uses mpv/VLC on this host." })),
        "ghoul_status" => ok(json!({ "canMount": false })),
        other => Err(format!("unknown command: {other}")),
    }
}

fn splash_checks(host: &Host) -> Result<Value, String> {
    let settings = host.store()?.load_settings().unwrap_or_default();
    let file = |label: &str, p: &str, req: bool| {
        let ok = std::path::Path::new(p).is_file();
        json!({ "label": label, "ok": if req { ok } else { true }, "detail": if ok { p } else if req { "Not found - set in Settings" } else { "Not found (optional)" } })
    };
    let ffmpeg = if settings.ffmpeg_path.trim().is_empty() {
        default_ffmpeg_path(&host.root).to_string_lossy().into_owned()
    } else {
        settings.ffmpeg_path.clone()
    };
    let ffprobe = if settings.ffprobe_path.trim().is_empty() {
        default_ffprobe_path(&host.root).to_string_lossy().into_owned()
    } else {
        settings.ffprobe_path.clone()
    };
    let mpv = if settings.mpv_path.trim().is_empty() {
        default_mpv_path(&host.root).to_string_lossy().into_owned()
    } else {
        settings.mpv_path.clone()
    };
    let vlc = if settings.vlc_path.trim().is_empty() {
        default_vlc_path().to_string_lossy().into_owned()
    } else {
        settings.vlc_path.clone()
    };
    ok(vec![
        file("ffmpeg", &ffmpeg, true),
        file("ffprobe", &ffprobe, true),
        file("mpv", &mpv, false),
        file("VLC", &vlc, false),
        json!({ "label": "Database", "ok": database_path().is_file() || true, "detail": database_path().to_string_lossy() }),
    ])
}

fn check_update() -> Result<Value, String> {
    let current = VERSION.to_string();
    match latest_github_release() {
        Ok(rel) => {
            let newer = remote_is_newer(&rel.tag, VERSION);
            let flavor = studio_core::update::flavor_from_process();
            let asset_name = studio_core::update::pick_binary_asset(&rel, flavor)
                .map(|a| a.name.clone());
            let release_url = if rel.html_url.is_empty() {
                GITHUB_RELEASES_LATEST.to_string()
            } else {
                rel.html_url.clone()
            };
            ok(json!({
                "current": current,
                "displayVersion": display_version(),
                "edition": EDITION,
                "latest": rel.tag,
                "updateAvailable": newer,
                "releaseUrl": release_url,
                "notes": rel.body,
                "error": null,
                "canApply": newer && asset_name.is_some(),
                "assetName": asset_name,
            }))
        }
        Err(e) => ok(json!({
            "current": current,
            "displayVersion": display_version(),
            "edition": EDITION,
            "latest": null,
            "updateAvailable": false,
            "releaseUrl": GITHUB_RELEASES_LATEST,
            "notes": null,
            "error": e,
            "canApply": false,
            "assetName": null,
        })),
    }
}

fn urlencoding_lite(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => o.push(b as char),
            b' ' => o.push('+'),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}
