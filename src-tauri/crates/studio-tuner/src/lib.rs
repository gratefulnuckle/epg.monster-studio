// SPDX-License-Identifier: GPL-3.0-or-later

pub mod discovery;
pub mod help;
pub mod host;
pub mod http;
pub mod manager;
pub mod probe;

pub const PLEX_PORT: u16 = 8080;
pub const JELLYFIN_PORT: u16 = 8081;
pub const EMBY_PORT: u16 = 8082;
pub const IPTV_PORT: u16 = 8083;

pub fn default_port(kind: &str) -> u16 {
    match kind {
        "Plex" => PLEX_PORT,
        "Jellyfin" => JELLYFIN_PORT,
        "Emby" => EMBY_PORT,
        "Iptv" => IPTV_PORT,
        _ => PLEX_PORT,
    }
}

pub fn is_legacy_reserved_port(port: u16) -> bool {
    matches!(port, 5004 | 5005 | 5006 | 5007)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use studio_core::models::{ManagedChannel, StreamVariant};
    use studio_core::settings::TunerServerProfile;

    use crate::host::{TunerHost, TunerSnapshot};
    use crate::probe;

    #[test]
    fn default_ports_match_csharp() {
        assert_eq!(default_port("Plex"), 8080);
        assert_eq!(default_port("Jellyfin"), 8081);
        assert_eq!(default_port("Emby"), 8082);
        assert_eq!(default_port("Iptv"), 8083);
    }

    #[test]
    fn legacy_hdhomerun_ports_are_reserved() {
        assert!(is_legacy_reserved_port(5004));
        assert!(!is_legacy_reserved_port(8080));
    }

    fn ch() -> ManagedChannel {
        ManagedChannel {
            id: "cnn".into(),
            name: "CNN".into(),
            group_title: "News".into(),
            tvg_id: Some("CNN.us".into()),
            tvg_logo: Some("http://logo.example/cnn.png".into()),
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: true,
            tuner_number: Some(5),
            variants: vec![StreamVariant {
                id: "v".into(),
                managed_channel_id: "cnn".into(),
                url: "http://provider.example/secret".into(),
                label: None,
                source_entry_id: None,
                origin_name: None,
                origin_tvg_id: None,
                visibility: "visible".into(),
                priority: 0,
                last_audit_ok: None,
                last_audit_at: None,
            }],
            has_epg_match: false,
        }
    }

    fn start_host(kind: &str) -> (Arc<TunerHost>, String) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let profile = TunerServerProfile {
            kind: kind.into(),
            enabled: true,
            running: false,
            friendly_name: format!("epg.monster studio ({})", kind.to_lowercase()),
            device_id: "C0FFEE01".into(),
            tuner_count: 1,
            bind_address: "127.0.0.1".into(),
            port: port as i32,
            allow_lan: false,
            remux_enabled: true,
            downspiral_enabled: kind == "Jellyfin",
        };
        let channels = vec![ch()];
        let snap = TunerSnapshot {
            channels,
            programmes: vec![],
            remux: true,
            epg_url: None,
            host_logos: false,
            use_local_logos: false,
            logo_root: String::new(),
            video_codec: "H264".into(),
            audio_codec: "AAC".into(),
            ffmpeg_path: String::new(),
        };
        let host = Arc::new(TunerHost::new(profile, Arc::new(move || TunerSnapshot {
            channels: snap.channels.clone(),
            programmes: snap.programmes.clone(),
            remux: snap.remux,
            epg_url: snap.epg_url.clone(),
            host_logos: snap.host_logos,
            use_local_logos: snap.use_local_logos,
            logo_root: snap.logo_root.clone(),
            video_codec: snap.video_codec.clone(),
            audio_codec: snap.audio_codec.clone(),
            ffmpeg_path: snap.ffmpeg_path.clone(),
        })));
        host.start().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        (host, format!("http://127.0.0.1:{port}"))
    }

    #[test]
    fn host_serves_discover_and_lineup_without_provider_urls() {
        let (_host, base) = start_host("Plex");
        let discover = ureq::get(&format!("{base}/discover.json")).call().unwrap().into_string().unwrap();
        assert!(discover.contains("C0FFEE01"));
        let lineup = ureq::get(&format!("{base}/lineup.json")).call().unwrap().into_string().unwrap();
        assert!(lineup.contains("/auto/v5"));
        assert!(!lineup.contains("provider.example"));
        let guide = ureq::get(&format!("{base}/guide.xml")).call().unwrap().into_string().unwrap();
        assert!(guide.contains("CNN.us"));
        let missing = ureq::get(&format!("{base}/auto/v99")).call();
        assert!(matches!(missing, Err(ureq::Error::Status(404, _))));
        let m3u = ureq::get(&format!("{base}/playlist.m3u8")).call().unwrap().into_string().unwrap();
        assert!(m3u.contains("url-tvg="));
        let scan = ureq::post(&format!("{base}/lineup.post?scan=start")).call();
        assert!(scan.is_ok());
        let head = ureq::request("HEAD", &format!("{base}/auto/v5")).call().unwrap();
        assert_eq!(head.status(), 200);
        assert!(head.content_type().contains("mp2t"));
    }

    #[test]
    fn plex_mimic_accepts_discover_lineup_guide() {
        let (_h, base) = start_host("Plex");
        let report = probe::run("Plex", &base);
        assert!(
            report.passed(),
            "{}",
            report
                .steps
                .iter()
                .map(|s| format!("{}: {}", s.name, s.detail))
                .collect::<Vec<_>>()
                .join("; ")
        );
        assert!(report.steps.iter().any(|s| s.name == "discover.json" && s.ok));
        assert!(report.steps.iter().any(|s| s.name == "lineup.json" && s.ok));
        assert!(report.steps.iter().any(|s| s.name == "guide.xml" && s.ok));
        assert_eq!(report.steps.len(), 5);
    }

    #[test]
    fn tivimate_mimic_uses_playlist_not_discover() {
        let (_h, base) = start_host("Iptv");
        let report = probe::run("Iptv", &base);
        assert!(
            report.passed(),
            "{}",
            report
                .steps
                .iter()
                .map(|s| format!("{}: {}", s.name, s.detail))
                .collect::<Vec<_>>()
                .join("; ")
        );
        assert!(report.steps.iter().any(|s| s.name == "playlist.m3u8" && s.ok));
        assert!(!report.steps.iter().any(|s| s.name == "discover.json"));
        assert_eq!(report.steps.len(), 4);
    }

    #[test]
    fn jellyfin_has_six_probe_steps() {
        let (_h, base) = start_host("Jellyfin");
        let report = probe::run("Jellyfin", &base);
        assert!(report.passed(), "{}", report.summary());
        assert_eq!(report.steps.len(), 6);
    }

    #[test]
    fn to_json_includes_kind_and_steps() {
        let mut report = probe::TunerProbeReport {
            kind: "Plex".into(),
            base_url: "http://127.0.0.1:8080".into(),
            steps: vec![],
        };
        report.steps.push(probe::TunerProbeStep {
            client: "Plex DVR".into(),
            name: "discover.json".into(),
            ok: true,
            detail: "ok".into(),
        });
        let json = probe::to_json(&[report]);
        assert!(json.contains("\"Kind\": \"Plex\""));
        assert!(json.contains("discover.json"));
        assert!(json.contains("epg.monster studio"));
    }

    #[test]
    fn lineup_without_provider_urls_passes_leak_check() {
        let (_h, base) = start_host("Emby");
        let report = probe::run("Emby", &base);
        assert!(!report.steps.iter().any(|s| s.detail.contains("Provider URL leaked")));
        assert!(report.steps.iter().find(|s| s.name == "lineup.json").unwrap().ok);
        assert_eq!(report.steps.len(), 5);
    }
}
