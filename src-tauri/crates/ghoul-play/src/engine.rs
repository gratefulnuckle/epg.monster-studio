// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineId {
    Gstreamer,
    MpvIpc,
    Libmpv,
}

impl EngineId {
    pub fn as_str(self) -> &'static str {
        match self {
            EngineId::Gstreamer => "gstreamer",
            EngineId::MpvIpc => "mpv-ipc",
            EngineId::Libmpv => "libmpv",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "gstreamer" => Some(Self::Gstreamer),
            "mpv-ipc" => Some(Self::MpvIpc),
            "libmpv" => Some(Self::Libmpv),
            _ => None,
        }
    }

    pub fn fallback_order() -> [EngineId; 3] {
        [EngineId::Gstreamer, EngineId::MpvIpc, EngineId::Libmpv]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Availability {
    pub id: EngineId,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlayState {
    Stopped,
    Playing,
    Paused,
    Buffering(u8),
    Error(String),
}

#[derive(Clone, Debug)]
pub struct PlayRequest {
    pub url: String,
    pub ua: String,
    pub headers: BTreeMap<String, String>,
    /// Native pane HWND / window id. Zero means "no pane" (status-only).
    pub pane: usize,
}

impl PlayRequest {
    /// Extra headers with User-Agent stripped so it is not sent twice.
    pub fn extra_headers(&self) -> BTreeMap<String, String> {
        self.headers
            .iter()
            .filter(|(k, v)| !k.eq_ignore_ascii_case("user-agent") && !v.trim().is_empty())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn http_header_fields(&self) -> String {
        self.extra_headers()
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("\r\n")
    }
}

pub trait Engine: Send {
    fn id(&self) -> EngineId;
    fn start(&mut self, req: &PlayRequest) -> Result<(), String>;
    fn stop(&mut self);
    fn reload(&mut self, req: &PlayRequest) -> Result<(), String> {
        self.stop();
        self.start(req)
    }
    fn pause(&mut self, paused: bool);
    fn mute(&mut self, muted: bool);
    fn volume(&mut self, vol: f64);
    fn status(&self) -> PlayState;
}

pub fn availability(app_root: &Path, gst_dir: Option<&str>) -> Vec<Availability> {
    vec![
        gst_availability(app_root, gst_dir),
        mpv_availability(app_root),
        libmpv_availability(app_root),
    ]
}

fn gst_availability(app_root: &Path, gst_dir: Option<&str>) -> Availability {
    let sidecar = paths::ghoul_gst_exe(app_root);
    if !sidecar.is_file() {
        return Availability {
            id: EngineId::Gstreamer,
            available: false,
            reason: Some("GStreamer not installed".into()),
        };
    }
    if paths::find_gst_root(gst_dir, app_root).is_none() {
        return Availability {
            id: EngineId::Gstreamer,
            available: false,
            reason: Some("GStreamer not installed".into()),
        };
    }
    Availability {
        id: EngineId::Gstreamer,
        available: true,
        reason: None,
    }
}

fn mpv_availability(app_root: &Path) -> Availability {
    let exe = paths::mpv_exe(app_root);
    if exe.is_file() {
        Availability {
            id: EngineId::MpvIpc,
            available: true,
            reason: None,
        }
    } else {
        Availability {
            id: EngineId::MpvIpc,
            available: false,
            reason: Some("mpv.exe not found".into()),
        }
    }
}

fn libmpv_availability(app_root: &Path) -> Availability {
    let dll = paths::libmpv_path(app_root);
    if dll.is_file() {
        Availability {
            id: EngineId::Libmpv,
            available: true,
            reason: None,
        }
    } else {
        Availability {
            id: EngineId::Libmpv,
            available: false,
            reason: Some("libmpv missing".into()),
        }
    }
}

/// Saved/default engine if present, else first available in fallback order.
pub fn choose_engine(saved: Option<EngineId>, avail: &[Availability]) -> Option<EngineId> {
    let ok = |id: EngineId| {
        avail
            .iter()
            .any(|a| a.id == id && a.available)
    };
    if let Some(id) = saved {
        if ok(id) {
            return Some(id);
        }
    }
    EngineId::fallback_order().into_iter().find(|id| ok(*id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_headers_drop_user_agent() {
        let mut headers = BTreeMap::new();
        headers.insert("User-Agent".into(), "ShouldNot".into());
        headers.insert("Referer".into(), "http://ref".into());
        let req = PlayRequest {
            url: "http://example/live".into(),
            ua: "Foo/1".into(),
            headers,
            pane: 0,
        };
        let extra = req.extra_headers();
        assert_eq!(extra.get("Referer").map(String::as_str), Some("http://ref"));
        assert!(!extra.keys().any(|k| k.eq_ignore_ascii_case("user-agent")));
        assert_eq!(req.http_header_fields(), "Referer: http://ref");
    }

    #[test]
    fn choose_falls_back_in_order() {
        let avail = vec![
            Availability {
                id: EngineId::Gstreamer,
                available: false,
                reason: Some("GStreamer not installed".into()),
            },
            Availability {
                id: EngineId::MpvIpc,
                available: true,
                reason: None,
            },
            Availability {
                id: EngineId::Libmpv,
                available: true,
                reason: None,
            },
        ];
        assert_eq!(
            choose_engine(Some(EngineId::Gstreamer), &avail),
            Some(EngineId::MpvIpc)
        );
        assert_eq!(
            choose_engine(Some(EngineId::Libmpv), &avail),
            Some(EngineId::Libmpv)
        );
        assert_eq!(choose_engine(None, &avail), Some(EngineId::MpvIpc));
    }

    #[test]
    fn choose_none_when_all_missing() {
        let avail = vec![
            Availability {
                id: EngineId::Gstreamer,
                available: false,
                reason: Some("GStreamer not installed".into()),
            },
            Availability {
                id: EngineId::MpvIpc,
                available: false,
                reason: Some("mpv.exe not found".into()),
            },
            Availability {
                id: EngineId::Libmpv,
                available: false,
                reason: Some("libmpv missing".into()),
            },
        ];
        assert_eq!(choose_engine(Some(EngineId::Gstreamer), &avail), None);
    }

    #[test]
    fn engine_id_roundtrip() {
        for id in EngineId::fallback_order() {
            assert_eq!(EngineId::parse(id.as_str()), Some(id));
        }
        assert_eq!(
            serde_json::to_string(&EngineId::MpvIpc).unwrap(),
            "\"mpv-ipc\""
        );
    }
}
