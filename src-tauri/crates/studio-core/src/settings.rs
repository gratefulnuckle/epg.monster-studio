// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn deserialize_tuner_kind<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum KindIn {
        Name(String),
        Code(i64),
    }
    Ok(match KindIn::deserialize(d)? {
        KindIn::Name(s) if s.eq_ignore_ascii_case("jellyfin") || s == "1" => "Jellyfin".into(),
        KindIn::Name(s) if s.eq_ignore_ascii_case("emby") || s == "2" => "Emby".into(),
        KindIn::Name(s) if s.eq_ignore_ascii_case("iptv") || s == "3" => "Iptv".into(),
        KindIn::Name(s) if !s.is_empty() => s,
        KindIn::Name(_) | KindIn::Code(0) => "Plex".into(),
        KindIn::Code(1) => "Jellyfin".into(),
        KindIn::Code(2) => "Emby".into(),
        KindIn::Code(3) => "Iptv".into(),
        KindIn::Code(_) => "Plex".into(),
    })
}

use crate::info::USER_AGENT;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum PlayerEngine {
    #[default]
    Mpv = 0,
    Vlc = 1,
}

impl Serialize for PlayerEngine {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(*self as i32)
    }
}

impl<'de> Deserialize<'de> for PlayerEngine {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = i32::deserialize(deserializer)?;
        Ok(if n == 1 {
            PlayerEngine::Vlc
        } else {
            PlayerEngine::Mpv
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct TunerServerProfile {
    #[serde(deserialize_with = "deserialize_tuner_kind")]
    pub kind: String,
    pub enabled: bool,
    pub running: bool,
    pub friendly_name: String,
    pub device_id: String,
    pub tuner_count: i32,
    pub bind_address: String,
    pub port: i32,
    pub allow_lan: bool,
    pub remux_enabled: bool,
    pub downspiral_enabled: bool,
}

impl TunerServerProfile {
    pub fn create_default(kind: &str) -> Self {
        let port = match kind {
            "Plex" => 8080,
            "Jellyfin" => 8081,
            "Emby" => 8082,
            _ => 8083,
        };
        Self {
            kind: kind.to_string(),
            enabled: kind == "Iptv",
            running: false,
            friendly_name: if kind == "Iptv" {
                "epg.monster studio (iptv)".into()
            } else {
                format!("epg.monster studio ({})", kind.to_lowercase())
            },
            device_id: new_device_id(),
            tuner_count: 2,
            bind_address: "127.0.0.1".into(),
            port,
            allow_lan: false,
            remux_enabled: true,
            downspiral_enabled: false,
        }
    }

    pub fn ensure_identity(&mut self) {
        if self.device_id.trim().is_empty() {
            self.device_id = new_device_id();
        }
        self.device_id = self.device_id.trim().to_uppercase();
        if self.tuner_count < 1 {
            self.tuner_count = 1;
        }
        if self.tuner_count > 16 {
            self.tuner_count = 16;
        }
        if self.port < 1
            || self.port > 65535
            || matches!(self.port, 5004 | 5005 | 5006 | 5007)
        {
            self.port = match self.kind.as_str() {
                "Plex" => 8080,
                "Jellyfin" => 8081,
                "Emby" => 8082,
                _ => 8083,
            };
        }
        if self.bind_address.trim().is_empty() {
            self.bind_address = "127.0.0.1".into();
        }
    }

    pub fn base_url(&self) -> String {
        let host = if self.allow_lan
            && matches!(self.bind_address.as_str(), "0.0.0.0" | "+" | "*")
        {
            "127.0.0.1"
        } else if self.bind_address.trim().is_empty() {
            "127.0.0.1"
        } else {
            self.bind_address.as_str()
        };
        format!("http://{host}:{}", self.port)
    }
}

fn new_device_id() -> String {
    uuid::Uuid::new_v4().as_bytes()[..4]
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct AppSettings {
    pub default_player: PlayerEngine,
    pub mpv_path: String,
    pub vlc_path: String,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub audit_delay_ms: i32,
    pub audit_timeout_ms: i32,
    pub auto_swap_on_audit_fail: bool,
    pub pause_audit_while_playing: bool,
    pub default_user_agent: String,
    pub python_path: Option<String>,
    pub epg_share_url: String,
    pub epg_xml_url: String,
    pub epg_xml_urls: Option<Vec<String>>,
    pub plex_tuner: TunerServerProfile,
    pub jellyfin_tuner: TunerServerProfile,
    pub emby_tuner: TunerServerProfile,
    pub iptv_tuner: TunerServerProfile,
    pub tuner_use_member_epg: bool,
    pub discovery_enabled: bool,
    pub remux_engine: String,
    pub remux_profile: String,
    pub remux_buffer_kb: i32,
    pub weekly_audit_json: String,
    pub weekly_audit_auto_run: bool,
    pub black_detect_enabled: bool,
    pub weekly_audit_last_run: String,
    pub logo_save_directory: String,
    pub host_logos_on_tuner: bool,
    pub use_local_logos: bool,
    /// Local PNG copies share the Save Logos folder. Off by default.
    #[serde(default)]
    pub cache_logos: bool,
    pub member_email: String,
    pub member_username: String,
    pub member_access_key: String,
    pub member_api_base: String,
    pub member_feed_url: String,
    pub member_feed_url_gz: String,
    pub member_max_channels: i32,
    pub member_max_body_bytes: i32,
    pub member_last_published_at: String,
    pub member_last_ping_at: String,
    /// When on, splash starts a GitHub latest-release check. Default off.
    #[serde(default)]
    pub check_for_app_updates: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            default_player: PlayerEngine::Mpv,
            mpv_path: String::new(),
            vlc_path: String::new(),
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            audit_delay_ms: 6000,
            audit_timeout_ms: 15000,
            auto_swap_on_audit_fail: true,
            pause_audit_while_playing: true,
            default_user_agent: USER_AGENT.into(),
            python_path: None,
            epg_share_url: String::new(),
            epg_xml_url: "https://epg.monster/epg.xml".into(),
            epg_xml_urls: None,
            plex_tuner: TunerServerProfile::create_default("Plex"),
            jellyfin_tuner: TunerServerProfile::create_default("Jellyfin"),
            emby_tuner: TunerServerProfile::create_default("Emby"),
            iptv_tuner: TunerServerProfile::create_default("Iptv"),
            tuner_use_member_epg: false,
            discovery_enabled: true,
            remux_engine: "ffmpeg".into(),
            remux_profile: "mpeg2_ac3".into(),
            remux_buffer_kb: 2048,
            weekly_audit_json: String::new(),
            weekly_audit_auto_run: false,
            black_detect_enabled: false,
            weekly_audit_last_run: String::new(),
            logo_save_directory: String::new(),
            host_logos_on_tuner: false,
            use_local_logos: false,
            cache_logos: false,
            member_email: String::new(),
            member_username: String::new(),
            member_access_key: String::new(),
            member_api_base: "https://epg.monster".into(),
            member_feed_url: String::new(),
            member_feed_url_gz: String::new(),
            member_max_channels: 2500,
            member_max_body_bytes: 3_145_728,
            member_last_published_at: String::new(),
            member_last_ping_at: String::new(),
            check_for_app_updates: false,
        }
    }
}

impl AppSettings {
    pub fn advertised_member_epg_url(&self) -> Option<String> {
        let gz = self.member_feed_url_gz.trim();
        if !gz.is_empty() {
            return Some(gz.to_string());
        }
        let u = self.member_feed_url.trim();
        if !u.is_empty() {
            Some(u.to_string())
        } else {
            None
        }
    }

    pub fn tuner_advertised_epg(&self, p: &TunerServerProfile) -> String {
        if self.tuner_use_member_epg {
            if let Some(u) = self.advertised_member_epg_url() {
                return u;
            }
        }
        format!("{}/guide.xml", p.base_url().trim_end_matches('/'))
    }

    pub fn lineup_codecs(&self) -> (&'static str, &'static str) {
        // Mpeg2Ac3 forces ffmpeg; VLC+copy_aac stays H264/AAC.
        if self.remux_profile.eq_ignore_ascii_case("copy_aac") {
            ("H264", "AAC")
        } else {
            ("MPEG2", "AC3")
        }
    }

    pub fn enabled_tuner_count(&self) -> i32 {
        [
            self.plex_tuner.enabled,
            self.jellyfin_tuner.enabled,
            self.emby_tuner.enabled,
            self.iptv_tuner.enabled,
        ]
        .into_iter()
        .filter(|e| *e)
        .count() as i32
    }

    pub fn ensure_tuner_profiles(&mut self) {
        self.plex_tuner.kind = "Plex".into();
        self.jellyfin_tuner.kind = "Jellyfin".into();
        self.emby_tuner.kind = "Emby".into();
        self.iptv_tuner.kind = "Iptv".into();
        for p in [
            &mut self.plex_tuner,
            &mut self.jellyfin_tuner,
            &mut self.emby_tuner,
            &mut self.iptv_tuner,
        ] {
            p.ensure_identity();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_app_settings() {
        let s = AppSettings::default();
        assert_eq!(s.audit_delay_ms, 6000);
        assert_eq!(s.audit_timeout_ms, 15000);
        assert!(s.auto_swap_on_audit_fail);
        assert!(s.iptv_tuner.enabled);
        assert!(!s.plex_tuner.enabled);
        assert_eq!(s.plex_tuner.port, 8080);
        assert_eq!(s.iptv_tuner.port, 8083);
        assert_eq!(s.epg_xml_url, "https://epg.monster/epg.xml");
        assert_eq!(s.default_user_agent, USER_AGENT);
    }

    #[test]
    fn json_round_trip_pascal_case() {
        let s = AppSettings::default();
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"AuditDelayMs\""));
        assert!(json.contains("\"IptvTuner\""));
        let back: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.audit_delay_ms, 6000);
        assert_eq!(back.default_player, PlayerEngine::Mpv);
        let legacy: PlayerEngine = serde_json::from_str("2").unwrap();
        assert_eq!(legacy, PlayerEngine::Mpv);
        assert!(!back.cache_logos);
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("CacheLogos");
        let old: AppSettings = serde_json::from_value(v).unwrap();
        assert!(!old.cache_logos);
    }

    #[test]
    fn migrates_legacy_ports() {
        let mut p = TunerServerProfile::create_default("Plex");
        p.port = 5004;
        p.ensure_identity();
        assert_eq!(p.port, 8080);
    }

    #[test]
    fn tuner_kind_accepts_integer_or_string() {
        let p: TunerServerProfile = serde_json::from_str(
            r#"{"Kind":0,"Enabled":true,"Running":false,"FriendlyName":"x","DeviceId":"AA","TunerCount":2,"BindAddress":"127.0.0.1","Port":8080,"AllowLan":false,"RemuxEnabled":true,"DownspiralEnabled":false}"#,
        )
        .unwrap();
        assert_eq!(p.kind, "Plex");
        let j: TunerServerProfile = serde_json::from_str(
            r#"{"Kind":1,"Enabled":true,"Running":false,"FriendlyName":"x","DeviceId":"AA","TunerCount":2,"BindAddress":"127.0.0.1","Port":8081,"AllowLan":false,"RemuxEnabled":true,"DownspiralEnabled":false}"#,
        )
        .unwrap();
        assert_eq!(j.kind, "Jellyfin");
    }
}
