// SPDX-License-Identifier: GPL-3.0-or-later
//! Local parity walk (13.2–13.4): open the machine AppData DB, exercise store
//! lists the UI pages use, serve lineup.json, self-test all four tuners.

use std::sync::Arc;

use studio_core::hdhr;
use studio_core::paths::database_path;
use studio_core::settings::TunerServerProfile;
use studio_core::store::SqliteStore;
use studio_tuner::host::{TunerHost, TunerSnapshot};
use studio_tuner::probe;

fn ephemeral_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn start_kind(kind: &str, channels: &[studio_core::ManagedChannel]) -> (Arc<TunerHost>, String) {
    let port = ephemeral_port();
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
    let snap_channels = channels.to_vec();
    let host = Arc::new(TunerHost::new(
        profile,
        Arc::new(move || TunerSnapshot {
            channels: snap_channels.clone(),
            ..TunerSnapshot::default()
        }),
    ));
    host.start().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(80));
    (host, format!("http://127.0.0.1:{port}"))
}

#[test]
fn walk_real_appdata_db_and_self_test_four_tuners() {
    let db = database_path();
    if !db.exists() {
        eprintln!("skip parity walk: no AppData DB at {}", db.display());
        return;
    }
    let store = SqliteStore::open(&db).expect("open live AppData DB");

    let sources = store.list_sources().expect("Add Sources");
    if sources.is_empty() {
        eprintln!("skip parity walk: AppData DB has no sources");
        return;
    }

    let groups = store.managed_groups().expect("Editor groups");
    let managed = store.list_managed(None).expect("Editor / Output list");
    assert!(
        !managed.is_empty(),
        "Playlist Editor has no managed channels"
    );
    let catalog = store.catalog_count().expect("EPG catalog");
    assert!(catalog > 0, "EPG catalog empty — splash/EPG page would be blank");
    let _programmes = store.programme_count().ok().unwrap_or(0);
    let _ = store.suggest_tvg("CNN");
    let known = managed.iter().filter(|c| c.has_epg_match).count();
    assert!(
        known > 0,
        "no tvg-id matches catalog — Editor green checks would never show"
    );

    let lineup_channels: Vec<_> = managed.iter().filter(|c| c.in_tuner).cloned().collect();
    assert!(
        !lineup_channels.is_empty(),
        "Tuner lineup empty — enable channels in Managed Output"
    );

    let json = hdhr::lineup_json(&lineup_channels, "http://127.0.0.1:8080", None, None);
    assert!(!json.to_ascii_lowercase().contains("http://") || json.contains("/auto/v"));
    for ch in &lineup_channels {
        for v in &ch.variants {
            let url = v.url.to_ascii_lowercase();
            if url.starts_with("http://") || url.starts_with("https://") {
                assert!(
                    !json.contains(&v.url),
                    "provider URL leaked into lineup.json: {}",
                    v.url
                );
            }
        }
    }

    let expected = [
        ("Plex", 5),
        ("Jellyfin", 6),
        ("Emby", 5),
        ("Iptv", 4),
    ];
    for (kind, steps) in expected {
        let (_host, base) = start_kind(kind, &lineup_channels);
        if kind != "Iptv" {
            let body = ureq::get(&format!("{base}/lineup.json"))
                .call()
                .unwrap()
                .into_string()
                .unwrap();
            assert!(
                !lineup_channels.iter().any(|ch| ch
                    .variants
                    .iter()
                    .any(|v| !v.url.is_empty() && body.contains(&v.url))),
                "{kind} /lineup.json leaked a variant URL"
            );
            assert!(body.contains("/auto/v"), "{kind} lineup missing /auto/v");
        }
        let report = probe::run(kind, &base);
        assert!(
            report.passed(),
            "{kind} self-test failed: {}",
            report
                .steps
                .iter()
                .map(|s| format!("{} ok={} {}", s.name, s.ok, s.detail))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        assert_eq!(report.steps.len(), steps, "{kind} step count");
    }

    eprintln!(
        "parity walk: {} sources, {} groups, {} managed, {} catalog, {} in tuner, {} epg matches",
        sources.len(),
        groups.len(),
        managed.len(),
        catalog,
        lineup_channels.len(),
        known
    );
}
