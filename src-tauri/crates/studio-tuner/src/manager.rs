// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use studio_core::settings::{AppSettings, TunerServerProfile};

use crate::discovery::DiscoveryHost;
use crate::host::{self, TunerHost, TunerSnapshot};
use crate::probe;
use crate::remux;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunerRuntimeStatus {
    pub kind: String,
    pub friendly_name: String,
    pub enabled: bool,
    pub running: bool,
    pub active_connections: u32,
    pub max_connections: i32,
    pub port: i32,
    pub base_url: String,
    pub device_id: String,
    pub error: Option<String>,
    pub status_label: String,
    pub advertised_epg: String,
}

impl TunerRuntimeStatus {
    fn from_profile(
        p: &TunerServerProfile,
        host: Option<&TunerHost>,
        err: Option<String>,
        advertised_epg: String,
    ) -> Self {
        let running = host.is_some();
        Self {
            kind: p.kind.clone(),
            friendly_name: p.friendly_name.clone(),
            enabled: p.enabled,
            running,
            active_connections: host.map(|h| h.active()).unwrap_or(0),
            max_connections: host.map(|h| h.max() as i32).unwrap_or(p.tuner_count),
            port: p.port,
            base_url: p.base_url(),
            device_id: p.device_id.clone(),
            error: host.and_then(|h| h.last_error.lock().ok().and_then(|g| g.clone())).or(err),
            status_label: if !p.enabled {
                "Not enabled in Settings".into()
            } else if running {
                "Running".into()
            } else {
                "Stopped".into()
            },
            advertised_epg,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunerGraphRow {
    pub kind: String,
    pub live: u32,
    pub max: u32,
    pub discover: u32,
    pub lineup: u32,
    pub guide: u32,
    pub m3u: u32,
    pub stream: u32,
    pub not_found: u32,
    pub bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunerLogLine {
    pub at: String,
    pub kind: String,
    pub line: String,
}

pub struct TunerManager {
    hosts: HashMap<String, Arc<TunerHost>>,
    errors: HashMap<String, String>,
    logs: Vec<TunerLogLine>,
    discovery: DiscoveryHost,
}

impl TunerManager {
    pub fn new() -> Self {
        Self {
            hosts: HashMap::new(),
            errors: HashMap::new(),
            logs: Vec::new(),
            discovery: DiscoveryHost::new(),
        }
    }

    fn log(&mut self, kind: &str, line: &str) {
        self.logs.push(TunerLogLine {
            at: studio_core::audit::now_iso(),
            kind: kind.into(),
            line: line.into(),
        });
        if self.logs.len() > 500 {
            self.logs.drain(0..self.logs.len() - 400);
        }
    }

    pub fn logs(&self) -> &[TunerLogLine] {
        &self.logs
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn try_start(
        &mut self,
        settings: &mut AppSettings,
        kind: &str,
        snapshot: Arc<dyn Fn() -> TunerSnapshot + Send + Sync>,
    ) -> Result<(), String> {
        settings.ensure_tuner_profiles();
        let cloned = {
            let profile = profile_mut(settings, kind)?;
            if !profile.enabled {
                return Err(
                    "This tuner is off in Settings. Check Plex / Jellyfin / Emby / IPTV and click Save."
                        .into(),
                );
            }
            self.log(kind, &format!("Start requested on port {}", profile.port));
            if self.hosts.contains_key(kind) {
                profile.running = true;
                None
            } else {
                Some(profile.clone())
            }
        };
        if cloned.is_none() {
            self.sync_discovery(settings);
            return Ok(());
        }
        let host = Arc::new(TunerHost::new(cloned.unwrap(), snapshot));
        match host.start() {
            Ok(()) => {
                if let Ok(p) = profile_mut(settings, kind) {
                    p.running = true;
                }
                self.errors.remove(kind);
                self.hosts.insert(kind.to_string(), host);
                self.log(kind, "Listening");
                self.sync_discovery(settings);
                Ok(())
            }
            Err(e) => {
                if let Ok(p) = profile_mut(settings, kind) {
                    p.running = false;
                }
                self.errors.insert(kind.to_string(), e.clone());
                self.log(kind, &e);
                Err(e)
            }
        }
    }

    pub fn stop(&mut self, settings: &mut AppSettings, kind: &str) {
        if let Ok(p) = profile_mut(settings, kind) {
            p.running = false;
        }
        if let Some(h) = self.hosts.remove(kind) {
            h.stop();
        }
        self.log(kind, "Stop requested");
        self.sync_discovery(settings);
    }

    pub fn stop_all(&mut self, settings: &mut AppSettings) {
        settings.ensure_tuner_profiles();
        for p in [
            &mut settings.plex_tuner,
            &mut settings.jellyfin_tuner,
            &mut settings.emby_tuner,
            &mut settings.iptv_tuner,
        ] {
            p.running = false;
        }
        for (_, h) in self.hosts.drain() {
            h.stop();
        }
        self.log("", "Stop all requested");
        self.sync_discovery(settings);
    }

    pub fn set_max(&mut self, settings: &mut AppSettings, kind: &str, max: i32) -> Result<(), String> {
        let p = profile_mut(settings, kind)?;
        p.tuner_count = max.clamp(1, 16);
        p.ensure_identity();
        if let Some(h) = self.hosts.get(kind) {
            h.set_max(p.tuner_count);
        }
        self.log(kind, &format!("Max connections set to {}", p.tuner_count));
        Ok(())
    }

    /// C# `TunerHostManager.Apply`: start enabled+running hosts, dispose the rest.
    pub fn apply(
        &mut self,
        settings: &mut AppSettings,
        snapshot: Arc<dyn Fn() -> TunerSnapshot + Send + Sync>,
    ) {
        settings.ensure_tuner_profiles();
        let want: Vec<String> = ["Plex", "Jellyfin", "Emby", "Iptv"]
            .into_iter()
            .filter(|k| {
                profile_ref(settings, k)
                    .map(|p| p.enabled && p.running)
                    .unwrap_or(false)
            })
            .map(|k| k.to_string())
            .collect();
        let stale: Vec<String> = self
            .hosts
            .keys()
            .filter(|k| !want.iter().any(|w| w == *k))
            .cloned()
            .collect();
        for k in stale {
            if let Some(h) = self.hosts.remove(&k) {
                h.stop();
                self.log(&k, "Stopped (settings apply)");
            }
        }
        for k in want {
            if self.hosts.contains_key(&k) {
                if let Ok(p) = profile_mut(settings, &k) {
                    if let Some(h) = self.hosts.get(&k) {
                        h.set_max(p.tuner_count);
                    }
                }
                continue;
            }
            if let Err(e) = self.try_start(settings, &k, snapshot.clone()) {
                self.log(&k, &e);
            }
        }
        self.sync_discovery(settings);
    }

    pub fn status(&self, settings: &AppSettings, kind: &str) -> Option<TunerRuntimeStatus> {
        let p = profile_ref(settings, kind)?;
        Some(TunerRuntimeStatus::from_profile(
            p,
            self.hosts.get(kind).map(|a| a.as_ref()),
            self.errors.get(kind).cloned(),
            settings.tuner_advertised_epg(p),
        ))
    }

    pub fn all_statuses(&self, settings: &AppSettings) -> Vec<TunerRuntimeStatus> {
        ["Plex", "Jellyfin", "Emby", "Iptv"]
            .into_iter()
            .filter_map(|k| self.status(settings, k))
            .collect()
    }

    fn sync_discovery(&mut self, settings: &AppSettings) {
        for line in self.discovery.take_logs() {
            self.log("", &line);
        }
        let running: Vec<&TunerServerProfile> = [
            &settings.plex_tuner,
            &settings.jellyfin_tuner,
            &settings.emby_tuner,
            &settings.iptv_tuner,
        ]
        .into_iter()
        .filter(|p| p.enabled && p.running)
        .collect();
        let want = settings.discovery_enabled && !running.is_empty();
        self.discovery.stop();
        if !want {
            return;
        }
        let allow_lan = running.iter().any(|p| p.allow_lan);
        let targets: Vec<(TunerServerProfile, String)> = running
            .into_iter()
            .map(|p| (p.clone(), advertise_base(p)))
            .collect();
        self.discovery.set_targets(targets);
        self.discovery.start(allow_lan);
        for line in self.discovery.take_logs() {
            self.log("", &line);
        }
    }

    pub fn graph_rows(&self, settings: &AppSettings) -> Vec<TunerGraphRow> {
        ["Plex", "Jellyfin", "Emby", "Iptv"]
            .into_iter()
            .filter_map(|k| {
                let p = profile_ref(settings, k)?;
                let host = self.hosts.get(k);
                let st = host.and_then(|h| h.stats.lock().ok());
                Some(TunerGraphRow {
                    kind: k.into(),
                    live: host.map(|h| h.active()).unwrap_or(0),
                    max: host.map(|h| h.max()).unwrap_or(p.tuner_count.max(1) as u32),
                    discover: st.as_ref().map(|s| s.discover).unwrap_or(0),
                    lineup: st.as_ref().map(|s| s.lineup).unwrap_or(0),
                    guide: st.as_ref().map(|s| s.guide).unwrap_or(0),
                    m3u: st.as_ref().map(|s| s.m3u).unwrap_or(0),
                    stream: st.as_ref().map(|s| s.stream).unwrap_or(0),
                    not_found: st.as_ref().map(|s| s.not_found).unwrap_or(0),
                    bytes: st.as_ref().map(|s| s.bytes).unwrap_or(0),
                })
            })
            .collect()
    }

    pub fn graphs(&self) -> Vec<String> {
        self.hosts
            .iter()
            .map(|(k, h)| {
                let s = h.stats.lock().ok();
                match s {
                    Some(st) => format!(
                        "{k}: discover {} · lineup {} · guide {} · m3u {} · stream {} · 404 {} · {} bytes · {} live",
                        st.discover, st.lineup, st.guide, st.m3u, st.stream, st.not_found, st.bytes, h.active()
                    ),
                    None => format!("{k}: (no stats)"),
                }
            })
            .collect()
    }
}

impl Default for TunerManager {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedTuner = Arc<Mutex<TunerManager>>;

fn profile_mut<'a>(s: &'a mut AppSettings, kind: &str) -> Result<&'a mut TunerServerProfile, String> {
    s.ensure_tuner_profiles();
    match kind {
        "Plex" => Ok(&mut s.plex_tuner),
        "Jellyfin" => Ok(&mut s.jellyfin_tuner),
        "Emby" => Ok(&mut s.emby_tuner),
        "Iptv" => Ok(&mut s.iptv_tuner),
        _ => Err("unknown tuner".into()),
    }
}

fn profile_ref<'a>(s: &'a AppSettings, kind: &str) -> Option<&'a TunerServerProfile> {
    match kind {
        "Plex" => Some(&s.plex_tuner),
        "Jellyfin" => Some(&s.jellyfin_tuner),
        "Emby" => Some(&s.emby_tuner),
        "Iptv" => Some(&s.iptv_tuner),
        _ => None,
    }
}

pub fn self_test(statuses: &[TunerRuntimeStatus]) -> (Vec<probe::TunerProbeReport>, String) {
    let mut reports = Vec::new();
    for s in statuses {
        if !s.running {
            let mut r = probe::TunerProbeReport {
                kind: s.kind.clone(),
                base_url: s.base_url.clone(),
                steps: Vec::new(),
            };
            r.steps.push(probe::TunerProbeStep {
                client: match s.kind.as_str() {
                    "Plex" => "Plex DVR".into(),
                    "Jellyfin" => "Jellyfin Live TV".into(),
                    "Emby" => "Emby Live TV".into(),
                    _ => "TiviMate".into(),
                },
                name: "listen".into(),
                ok: false,
                detail: "Not running — Start this tuner first".into(),
            });
            reports.push(r);
            continue;
        }
        reports.push(probe::run(&s.kind, &s.base_url));
    }
    let json = probe::to_json(&reports);
    let _ = probe::write_json(&reports, None);
    (reports, json)
}

pub fn snapshot_from_settings(
    channels: Vec<studio_core::models::ManagedChannel>,
    programmes: Vec<studio_core::models::EpgProgramme>,
    settings: &AppSettings,
) -> TunerSnapshot {
    let (v, a) = settings.lineup_codecs();
    let logo_root = if settings.logo_save_directory.trim().is_empty() {
        studio_core::logo::default_logo_dir().to_string_lossy().into_owned()
    } else {
        settings.logo_save_directory.clone()
    };
    TunerSnapshot {
        channels,
        programmes,
        remux: settings.iptv_tuner.remux_enabled,
        epg_url: if settings.tuner_use_member_epg {
            settings.advertised_member_epg_url()
        } else {
            None
        },
        host_logos: settings.host_logos_on_tuner || settings.use_local_logos,
        use_local_logos: settings.use_local_logos,
        logo_root,
        video_codec: v.into(),
        audio_codec: a.into(),
        ffmpeg_path: settings.ffmpeg_path.clone(),
        vlc_path: settings.vlc_path.clone(),
        remux_engine: settings.remux_engine.clone(),
        remux_profile: settings.remux_profile.clone(),
        remux_buffer_bytes: remux::clamp_buffer_kb(settings.remux_buffer_kb) * 1024,
        user_agent: settings.default_user_agent.clone(),
        variant_headers: HashMap::new(),
        note_failover: None,
    }
}

fn advertise_base(p: &TunerServerProfile) -> String {
    if p.allow_lan {
        if let Some(ip) = host::lan_ipv4().into_iter().next() {
            return format!("http://{ip}:{}", p.port);
        }
    }
    p.base_url()
}
