// SPDX-License-Identifier: GPL-3.0-or-later

//! Studio host for G-houl: curated snapshot, member XMLTV, pane, engines.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use ghoul_guide::{categories, parse_m3u, Category, Channel, Handle as GuideHandle, Snapshot};
use ghoul_pane::{Pane, Rect};
use ghoul_play::{availability, choose_engine, Availability, EngineId, PlayRequest, Session};
use studio_core::epg::now_playing_coverage_is_enough;
use studio_core::export::{
    export_ghoul_snapshot, ghoul_mount_gate, programme_lookup_ids, write_guide_xmltv,
};
use studio_core::info::VERSION;
use studio_core::members;
use studio_core::models::{EpgProgramme, ManagedChannel};
use studio_core::paths::app_data_directory;
use studio_core::settings::AppSettings;
use studio_core::store::SqliteStore;

use crate::AppState;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub engine: String,
    pub ua_preset: String,
    pub custom_ua: String,
    pub last_channel: usize,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            engine: "gstreamer".into(),
            ua_preset: "tivimate".into(),
            custom_ua: String::new(),
            last_channel: 0,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    pub curated_count: i32,
    pub playable_count: i32,
    pub has_key: bool,
    pub feed_url: Option<String>,
    pub engines: Vec<Availability>,
    pub prefs: Prefs,
    pub can_mount: bool,
    pub gate: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDto {
    pub channels: Vec<Channel>,
    pub categories: Vec<Category>,
    pub engines: Vec<Availability>,
    pub prefs: Prefs,
    pub xml_ok: bool,
    pub xml_message: Option<String>,
}

struct Live {
    guide: Option<GuideHandle>,
    session: Option<Session>,
    pane: Option<Pane>,
    channels: Vec<Channel>,
    prefs: Prefs,
    app_root: PathBuf,
}

impl Live {
    fn empty() -> Self {
        Self {
            guide: None,
            session: None,
            pane: None,
            channels: Vec::new(),
            prefs: Prefs::default(),
            app_root: PathBuf::new(),
        }
    }
}

static LIVE: Mutex<Live> = Mutex::new(Live {
    guide: None,
    session: None,
    pane: None,
    channels: Vec::new(),
    prefs: Prefs {
        engine: String::new(),
        ua_preset: String::new(),
        custom_ua: String::new(),
        last_channel: 0,
    },
    app_root: PathBuf::new(),
});

fn ghoul_dir() -> PathBuf {
    let dir = app_data_directory().join("ghoul");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn prefs_path() -> PathBuf {
    ghoul_dir().join("player.json")
}

fn load_prefs() -> Prefs {
    let raw = std::fs::read_to_string(prefs_path()).unwrap_or_default();
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_prefs(p: &Prefs) {
    if let Ok(json) = serde_json::to_string_pretty(p) {
        let _ = std::fs::write(prefs_path(), json);
    }
}

fn lock_live() -> Result<std::sync::MutexGuard<'static, Live>, String> {
    LIVE.lock().map_err(|_| "ghoul busy".to_string())
}

#[tauri::command]
pub fn ghoul_status(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<StatusDto, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let channels = store.list_managed(None).map_err(|e| e.to_string())?;
    let settings = store.load_settings().map_err(|e| e.to_string())?;
    let playable = channels
        .iter()
        .filter(|ch| {
            ch.variants
                .iter()
                .any(|v| v.visibility == "visible" && !v.url.trim().is_empty())
        })
        .count();
    let gate = ghoul_mount_gate(&channels).err();
    let root = crate::app_root(&app);
    let engines = availability(&root, None);
    let engine_ok = choose_engine(None, &engines).is_some();
    let mut gate = gate;
    if gate.is_none() && !engine_ok {
        let parts: Vec<String> = engines
            .iter()
            .filter(|e| !e.available)
            .filter_map(|e| e.reason.clone())
            .collect();
        gate = Some(if parts.is_empty() {
            "no engine available".into()
        } else {
            parts.join("; ")
        });
    }
    let feed = settings.advertised_member_epg_url();
    Ok(StatusDto {
        curated_count: channels.len() as i32,
        playable_count: playable as i32,
        has_key: !settings.member_access_key.trim().is_empty(),
        feed_url: feed,
        engines,
        prefs: load_prefs(),
        can_mount: gate.is_none(),
        gate,
    })
}

#[tauri::command]
pub async fn ghoul_prepare(
    state: tauri::State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<PrepareDto, String> {
    let store = Arc::clone(&state.store);
    let root = crate::app_root(&app);
    tauri::async_runtime::spawn_blocking(move || ghoul_prepare_inner(&store, root))
        .await
        .map_err(|e| e.to_string())?
}

fn ghoul_prepare_inner(
    store: &Arc<Mutex<SqliteStore>>,
    root: PathBuf,
) -> Result<PrepareDto, String> {
    let (channels, headers, settings, covering, managed_with_tvg) = {
        let g = store.lock().map_err(|e| e.to_string())?;
        let channels = g.list_managed(None).map_err(|e| e.to_string())?;
        ghoul_mount_gate(&channels)?;
        let headers = g.headers_for_channels(&channels);
        let settings = g.load_settings().map_err(|e| e.to_string())?;
        let covering = g.covering_now_count().unwrap_or(0);
        let managed_with_tvg = g.managed_with_tvg_count().unwrap_or(0);
        (channels, headers, settings, covering, managed_with_tvg)
    };

    if !settings.player_uses_member_epg()
        && !now_playing_coverage_is_enough(covering, managed_with_tvg)
    {
        let _ = crate::rebuild_now_playing_inner(store);
    }

    let programmes = {
        let g = store.lock().map_err(|e| e.to_string())?;
        let ids = programme_lookup_ids(&channels);
        g.list_programmes_nearby(&ids).map_err(|e| e.to_string())?
    };

    let m3u = export_ghoul_snapshot(&channels, &headers);
    let dir = ghoul_dir();
    let m3u_path = dir.join("iptv.m3u");
    std::fs::write(&m3u_path, &m3u).map_err(|e| e.to_string())?;

    let parsed = parse_m3u(&m3u);
    if parsed.iter().all(|c| c.url.trim().is_empty()) {
        return Err("Curated channels have no playable stream URLs.".into());
    }

    let (xml_ok, xml_message) = write_player_xml(&settings, &dir, &channels, &programmes);
    let xml_path = dir.join("iptv.xml");
    let guide = ghoul_guide::start();
    let wanted: Vec<String> = parsed.iter().map(|c| c.tvg_id.clone()).collect();
    guide.set_wanted(wanted);
    if xml_ok {
        guide.set_xml(&xml_path.to_string_lossy());
    }

    let engines = availability(&root, None);
    if choose_engine(None, &engines).is_none() {
        let parts: Vec<String> = engines
            .iter()
            .filter(|e| !e.available)
            .filter_map(|e| e.reason.clone())
            .collect();
        return Err(parts.join("; "));
    }
    let prefs = load_prefs();
    let cats = categories(&parsed);
    {
        let mut live = lock_live()?;
        if let Some(g) = live.guide.take() {
            g.stop();
        }
        live.guide = Some(guide);
        live.channels = parsed.clone();
        live.prefs = prefs.clone();
        live.app_root = root;
        if live.session.is_none() {
            live.session = Some(Session::new(live.app_root.clone()));
        }
    }
    Ok(PrepareDto {
        channels: parsed,
        categories: cats,
        engines,
        prefs,
        xml_ok,
        xml_message,
    })
}

fn write_player_xml(
    settings: &AppSettings,
    dir: &std::path::Path,
    channels: &[ManagedChannel],
    programmes: &[EpgProgramme],
) -> (bool, Option<String>) {
    if settings.player_uses_member_epg() {
        return fetch_member_xml(settings, dir);
    }
    let path = dir.join("iptv.xml");
    match write_guide_xmltv(&path, channels, programmes) {
        Ok(()) => {
            if programmes.is_empty() {
                (
                    false,
                    Some("No programmes in the epg.monster index for this lineup.".into()),
                )
            } else {
                (true, None)
            }
        }
        Err(_) => (false, Some("EPG failed".into())),
    }
}

fn fetch_member_xml(settings: &AppSettings, dir: &std::path::Path) -> (bool, Option<String>) {
    let key = settings.member_access_key.trim();
    let Some(url) = settings.advertised_member_epg_url() else {
        return (false, Some("EPG failed".into()));
    };
    match members::fetch_member_xml(&url, key, Some(VERSION)) {
        Ok(bytes) => {
            let path = dir.join("iptv.xml");
            if std::fs::write(&path, bytes).is_err() {
                return (false, Some("EPG failed".into()));
            }
            (true, None)
        }
        Err(_) => (false, Some("EPG failed".into())),
    }
}

fn parent_hwnd(window: &tauri::WebviewWindow) -> Result<usize, String> {
    #[cfg(windows)]
    {
        window
            .hwnd()
            .map(|h| h.0 as isize as usize)
            .map_err(|_| "parent hwnd".to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        Err("video pane unsupported on this OS".into())
    }
}

#[tauri::command]
pub fn ghoul_mount(window: tauri::WebviewWindow) -> Result<usize, String> {
    let hwnd = parent_hwnd(&window)?;
    let mut live = lock_live()?;
    if live.pane.is_some() {
        return Ok(live.pane.as_ref().map(|p| p.hwnd()).unwrap_or(0));
    }
    let pane = Pane::create(hwnd).map_err(|e| e.to_string())?;
    let id = pane.hwnd();
    live.pane = Some(pane);
    Ok(id)
}

#[tauri::command]
pub fn ghoul_unmount() -> Result<(), String> {
    let mut live = lock_live()?;
    if let Some(s) = live.session.as_mut() {
        s.stop();
    }
    if let Some(p) = live.pane.as_mut() {
        p.destroy();
    }
    live.pane = None;
    if let Some(g) = live.guide.take() {
        g.stop();
    }
    save_prefs(&live.prefs);
    Ok(())
}

#[tauri::command]
pub fn ghoul_set_rect(x: i32, y: i32, w: i32, h: i32) -> Result<(), String> {
    let mut live = lock_live()?;
    if let Some(p) = live.pane.as_mut() {
        p.set_rect(Rect { x, y, w, h });
    }
    Ok(())
}

fn current_request(live: &Live, index: usize) -> Result<PlayRequest, String> {
    let ch = live
        .channels
        .get(index)
        .ok_or_else(|| "no channel".to_string())?;
    let ua = ghoul_play::resolve_ua(&ch.ua, &live.prefs.ua_preset, &live.prefs.custom_ua);
    let pane = live.pane.as_ref().map(|p| p.hwnd()).unwrap_or(0);
    Ok(PlayRequest {
        url: ch.url.clone(),
        ua,
        headers: ch.headers.clone(),
        pane,
    })
}

#[tauri::command]
pub fn ghoul_play(index: usize) -> Result<(), String> {
    let mut live = lock_live()?;
    if live.channels.is_empty() {
        return Err("nothing to play".into());
    }
    let idx = ghoul_guide::wrap_index(live.channels.len(), index, 0);
    let req = current_request(&live, idx)?;
    let engines = availability(&live.app_root, None);
    let saved = EngineId::parse(&live.prefs.engine);
    let app_root = live.app_root.clone();
    let session = live.session.get_or_insert_with(|| Session::new(app_root));
    let id = session.start_saved_or_fallback(saved, &engines, req)?;
    live.prefs.engine = id.as_str().to_string();
    live.prefs.last_channel = idx;
    save_prefs(&live.prefs);
    Ok(())
}

#[tauri::command]
pub fn ghoul_stop() -> Result<(), String> {
    let mut live = lock_live()?;
    if let Some(s) = live.session.as_mut() {
        s.stop();
    }
    Ok(())
}

#[tauri::command]
pub fn ghoul_pause(paused: bool) -> Result<(), String> {
    let mut live = lock_live()?;
    if let Some(s) = live.session.as_mut() {
        s.pause(paused);
    }
    Ok(())
}

#[tauri::command]
pub fn ghoul_mute(muted: bool) -> Result<(), String> {
    let mut live = lock_live()?;
    if let Some(s) = live.session.as_mut() {
        s.mute(muted);
    }
    Ok(())
}

#[tauri::command]
pub fn ghoul_volume(vol: f64) -> Result<(), String> {
    let mut live = lock_live()?;
    if let Some(s) = live.session.as_mut() {
        s.volume(vol);
    }
    Ok(())
}

#[tauri::command]
pub fn ghoul_set_engine(id: String) -> Result<(), String> {
    let mut live = lock_live()?;
    let engine = EngineId::parse(&id).ok_or_else(|| "unknown engine".to_string())?;
    live.prefs.engine = engine.as_str().to_string();
    save_prefs(&live.prefs);
    if let Some(s) = live.session.as_mut() {
        if s.last_request().is_some() {
            s.switch(engine)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn ghoul_set_ua(preset: String, custom: String) -> Result<(), String> {
    let mut live = lock_live()?;
    live.prefs.ua_preset = preset;
    live.prefs.custom_ua = custom;
    save_prefs(&live.prefs);
    let idx = live.prefs.last_channel;
    if live.session.as_ref().and_then(|s| s.last_request()).is_some() {
        let req = current_request(&live, idx)?;
        if let Some(s) = live.session.as_mut() {
            s.reload(req)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn ghoul_snapshot() -> Result<Snapshot, String> {
    let live = lock_live()?;
    Ok(live
        .guide
        .as_ref()
        .map(|g| g.snapshot())
        .unwrap_or_default())
}

#[tauri::command]
pub fn ghoul_prefs() -> Prefs {
    load_prefs()
}

pub fn teardown() {
    if let Ok(mut live) = LIVE.lock() {
        if let Some(s) = live.session.as_mut() {
            s.stop();
        }
        if let Some(p) = live.pane.as_mut() {
            p.destroy();
        }
        live.pane = None;
        if let Some(g) = live.guide.take() {
            g.stop();
        }
        save_prefs(&live.prefs);
        *live = Live::empty();
    }
}
