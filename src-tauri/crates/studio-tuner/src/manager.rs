// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use studio_core::settings::{AppSettings, TunerServerProfile};

use crate::host::{TunerHost, TunerSnapshot};
use crate::probe;

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
}

impl TunerRuntimeStatus {
    fn from_profile(p: &TunerServerProfile, host: Option<&TunerHost>, err: Option<String>) -> Self {
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
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunerLogLine {
    pub kind: String,
    pub line: String,
}

pub struct TunerManager {
    hosts: HashMap<String, Arc<TunerHost>>,
    errors: HashMap<String, String>,
    logs: Vec<TunerLogLine>,
}

impl TunerManager {
    pub fn new() -> Self {
        Self {
            hosts: HashMap::new(),
            errors: HashMap::new(),
            logs: Vec::new(),
        }
    }

    fn log(&mut self, kind: &str, line: &str) {
        self.logs.push(TunerLogLine {
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

    pub fn try_start(
        &mut self,
        settings: &mut AppSettings,
        kind: &str,
        snapshot: Arc<dyn Fn() -> TunerSnapshot + Send + Sync>,
    ) -> Result<(), String> {
        settings.ensure_tuner_profiles();
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
            return Ok(());
        }
        let host = Arc::new(TunerHost::new(profile.clone(), snapshot));
        match host.start() {
            Ok(()) => {
                profile.running = true;
                self.errors.remove(kind);
                self.hosts.insert(kind.to_string(), host);
                self.log(kind, "Listening");
                Ok(())
            }
            Err(e) => {
                profile.running = false;
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

    pub fn status(&self, settings: &AppSettings, kind: &str) -> Option<TunerRuntimeStatus> {
        let p = profile_ref(settings, kind)?;
        Some(TunerRuntimeStatus::from_profile(
            p,
            self.hosts.get(kind).map(|a| a.as_ref()),
            self.errors.get(kind).cloned(),
        ))
    }

    pub fn all_statuses(&self, settings: &AppSettings) -> Vec<TunerRuntimeStatus> {
        ["Plex", "Jellyfin", "Emby", "Iptv"]
            .into_iter()
            .filter_map(|k| self.status(settings, k))
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
                client: s.kind.clone(),
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
    }
}
