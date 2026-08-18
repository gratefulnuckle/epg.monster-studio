// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{HashMap, HashSet};

use serde_json::{json, Value};

use crate::lineup;
use crate::logo;
use crate::models::{EpgProgramme, ManagedChannel};
use crate::settings::TunerServerProfile;

pub fn discover_json(profile: &TunerServerProfile, base_url: &str) -> String {
    let mut p = profile.clone();
    p.ensure_identity();
    let root = base_url.trim_end_matches('/');
    serde_json::to_string(&json!({
        "FriendlyName": p.friendly_name,
        "ModelNumber": "HDHR3-US",
        "FirmwareName": "hdhomerun_atsc",
        "FirmwareVersion": "20200101",
        "DeviceID": p.device_id,
        "DeviceAuth": "epgmonster",
        "BaseURL": root,
        "LineupURL": format!("{root}/lineup.json"),
        "TunerCount": p.tuner_count
    }))
    .unwrap_or_else(|_| "{}".into())
}

pub fn lineup_status_json() -> &'static str {
    r#"{"ScanInProgress":0,"ScanPossible":1,"Source":"Cable","SourceList":["Cable"]}"#
}

pub fn lineup_json(
    channels: &[ManagedChannel],
    base_url: &str,
    video_codec: Option<&str>,
    audio_codec: Option<&str>,
) -> String {
    let root = base_url.trim_end_matches('/');
    let vcodec = video_codec.filter(|s| !s.is_empty()).unwrap_or("H264");
    let acodec = audio_codec.filter(|s| !s.is_empty()).unwrap_or("AAC");
    let rows: Vec<Value> = lineup::ordered_lineup(channels)
        .into_iter()
        .map(|ch| {
            let n = ch.tuner_number.unwrap_or(0);
            json!({
                "GuideNumber": n.to_string(),
                "GuideName": ch.name,
                "VideoCodec": vcodec,
                "AudioCodec": acodec,
                "URL": format!("{root}/auto/v{n}")
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

pub fn tuner_m3u(
    channels: &[ManagedChannel],
    base_url: &str,
    epg_url: Option<&str>,
    remux: bool,
    use_local_logos: bool,
) -> String {
    let root = base_url.trim_end_matches('/');
    let tvg = epg_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{root}/guide.xml"));
    let mut sb = format!("#EXTM3U url-tvg=\"{tvg}\"\n");
    for ch in lineup::ordered_lineup(channels) {
        let n = ch.tuner_number.unwrap_or(0);
        let mut attrs = vec![format!("tvg-chno=\"{n}\""), format!("channel-number=\"{n}\"")];
        if let Some(id) = ch.tvg_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            attrs.push(format!("tvg-id=\"{}\"", xml_escape_attr(id)));
        }
        if !ch.name.is_empty() {
            attrs.push(format!("tvg-name=\"{}\"", xml_escape_attr(&ch.name)));
        }
        if let Some(logo) = logo::playlist_logo(&ch, root, use_local_logos) {
            attrs.push(format!("tvg-logo=\"{}\"", xml_escape_attr(&logo)));
        }
        attrs.push(format!(
            "group-title=\"{}\"",
            xml_escape_attr(&ch.group_title)
        ));
        sb.push_str("#EXTINF:-1 ");
        sb.push_str(&attrs.join(" "));
        sb.push(',');
        sb.push_str(&ch.name);
        sb.push('\n');
        if remux {
            sb.push_str(root);
            sb.push_str("/auto/v");
            sb.push_str(&n.to_string());
            sb.push('\n');
        } else {
            let url = ch
                .variants
                .iter()
                .find(|v| v.visibility == "visible")
                .or_else(|| ch.variants.first())
                .map(|v| v.url.trim().to_string())
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| format!("{root}/auto/v{n}"));
            sb.push_str(&url);
            sb.push('\n');
        }
    }
    sb
}

pub fn channel_xml_id(ch: &ManagedChannel) -> String {
    ch.tvg_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&ch.id)
        .to_string()
}

pub fn xmltv_channel_id(ch: &ManagedChannel) -> String {
    if ch.tuner_number.unwrap_or(0) > 0 {
        ch.tuner_number.unwrap().to_string()
    } else if ch.id.trim().is_empty() {
        channel_xml_id(ch)
    } else {
        ch.id.clone()
    }
}

pub fn xmltv_time(rfc3339: &str) -> String {
    if let Ok(dt) = time::OffsetDateTime::parse(
        rfc3339,
        &time::format_description::well_known::Rfc3339,
    ) {
        let u = dt.to_offset(time::UtcOffset::UTC);
        return format!(
            "{:04}{:02}{:02}{:02}{:02}{:02} +0000",
            u.year(),
            u.month() as u8,
            u.day(),
            u.hour(),
            u.minute(),
            u.second()
        );
    }
    rfc3339.to_string()
}

pub fn guide_xml(
    channels: &[ManagedChannel],
    programmes: &[EpgProgramme],
    tuner_base: Option<&str>,
    use_local_logos: bool,
) -> String {
    let lineup = lineup::ordered_lineup(channels);
    let mut by_tvg: HashMap<String, Vec<&EpgProgramme>> = HashMap::new();
    for p in programmes {
        if p.tvg_id.trim().is_empty() {
            continue;
        }
        by_tvg
            .entry(p.tvg_id.trim().to_ascii_lowercase())
            .or_default()
            .push(p);
    }
    for list in by_tvg.values_mut() {
        list.sort_by(|a, b| a.start_utc.cmp(&b.start_utc));
    }
    let mut sb = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    sb.push_str("<!DOCTYPE tv SYSTEM \"xmltv.dtd\">\n");
    sb.push_str("<tv generator-info-name=\"epg.monster studio\">\n");
    let root = tuner_base.unwrap_or("");
    for ch in &lineup {
        let id = xmltv_channel_id(ch);
        sb.push_str("  <channel id=\"");
        sb.push_str(&xml_escape(&id));
        sb.push_str("\">\n");
        let name = if ch.name.trim().is_empty() {
            id.clone()
        } else {
            ch.name.trim().to_string()
        };
        sb.push_str("    <display-name>");
        sb.push_str(&xml_escape(&name));
        sb.push_str("</display-name>\n");
        let num = ch.tuner_number.unwrap_or(0).to_string();
        if !num.eq_ignore_ascii_case(&name) {
            sb.push_str("    <display-name>");
            sb.push_str(&xml_escape(&num));
            sb.push_str("</display-name>\n");
        }
        let tvg = channel_xml_id(ch);
        if ch.tvg_id.as_deref().map(str::trim).is_some_and(|s| !s.is_empty())
            && !tvg.eq_ignore_ascii_case(&name)
            && !tvg.eq_ignore_ascii_case(&num)
        {
            sb.push_str("    <display-name>");
            sb.push_str(&xml_escape(&tvg));
            sb.push_str("</display-name>\n");
        }
        if let Some(icon) = logo::playlist_logo(ch, root, use_local_logos) {
            sb.push_str("    <icon src=\"");
            sb.push_str(&xml_escape(&icon));
            sb.push_str("\" />\n");
        }
        sb.push_str("  </channel>\n");
    }
    for ch in &lineup {
        let xml_id = xmltv_channel_id(ch);
        let tvg = channel_xml_id(ch);
        let Some(list) = by_tvg.get(&tvg.to_ascii_lowercase()) else {
            continue;
        };
        for p in list {
            sb.push_str("  <programme start=\"");
            sb.push_str(&xmltv_time(&p.start_utc));
            sb.push_str("\" stop=\"");
            sb.push_str(&xmltv_time(&p.stop_utc));
            sb.push_str("\" channel=\"");
            sb.push_str(&xml_escape(&xml_id));
            sb.push_str("\">\n    <title>");
            sb.push_str(&xml_escape(&p.title));
            sb.push_str("</title>\n");
            if let Some(d) = p.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                sb.push_str("    <desc>");
                sb.push_str(&xml_escape(d));
                sb.push_str("</desc>\n");
            }
            sb.push_str("  </programme>\n");
        }
    }
    sb.push_str("</tv>\n");
    sb
}

pub fn group_slug(group_title: &str) -> String {
    let s = group_title.trim().to_lowercase();
    if s.is_empty() {
        return "ungrouped".into();
    }
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "ungrouped".into()
    } else {
        slug
    }
}

pub struct DownspiralList {
    pub slug: String,
    pub name: String,
    pub channels: Vec<ManagedChannel>,
}

pub fn downspiral_lists(channels: &[ManagedChannel]) -> Vec<DownspiralList> {
    let lineup = lineup::ordered_lineup(channels);
    let mut groups: Vec<(String, Vec<ManagedChannel>)> = Vec::new();
    for ch in lineup {
        let key = if ch.group_title.trim().is_empty() {
            "Ungrouped".to_string()
        } else {
            ch.group_title.trim().to_string()
        };
        if let Some((_, list)) = groups
            .iter_mut()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        {
            list.push(ch);
        } else {
            groups.push((key, vec![ch]));
        }
    }
    groups.sort_by(|a, b| a.0.to_ascii_lowercase().cmp(&b.0.to_ascii_lowercase()));
    let mut used = HashSet::new();
    let mut list = Vec::new();
    for (name, chans) in groups {
        let slug = group_slug(&name);
        let mut unique = slug.clone();
        let mut n = 2;
        while !used.insert(unique.to_ascii_lowercase()) {
            unique = format!("{slug}-{n}");
            n += 1;
        }
        list.push(DownspiralList {
            slug: unique,
            name,
            channels: chans,
        });
    }
    list
}

pub fn downspiral_index_json(channels: &[ManagedChannel], base_url: &str) -> String {
    let root = base_url.trim_end_matches('/');
    let lists: Vec<Value> = downspiral_lists(channels)
        .into_iter()
        .map(|g| {
            json!({
                "id": g.slug,
                "name": g.name,
                "channels": g.channels.len(),
                "playlist": format!("{root}/downspiral/{}.m3u8", g.slug),
                "epg": format!("{root}/downspiral/{}.xml", g.slug)
            })
        })
        .collect();
    serde_json::to_string(&json!({
        "schema": "epg.monster.downspiral",
        "version": 1,
        "lists": lists
    }))
    .unwrap_or_else(|_| "{}".into())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_escape_attr(s: &str) -> String {
    s.replace('"', "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StreamVariant;
    use crate::settings::AppSettings;

    fn ch(name: &str, tvg: &str, num: i32, url: &str) -> ManagedChannel {
        ManagedChannel {
            id: name.into(),
            name: name.into(),
            group_title: "Ungrouped".into(),
            tvg_id: Some(tvg.into()),
            tvg_logo: None,
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: true,
            tuner_number: Some(num),
            variants: if url.is_empty() {
                vec![]
            } else {
                vec![StreamVariant {
                    id: format!("{name}-v"),
                    managed_channel_id: name.into(),
                    url: url.into(),
                    label: None,
                    source_entry_id: None,
                    origin_name: None,
                    origin_tvg_id: None,
                    visibility: "visible".into(),
                    priority: 0,
                    last_audit_ok: None,
                    last_audit_at: None,
                }]
            },
            has_epg_match: false,
        }
    }

    #[test]
    fn lineup_json_uses_local_proxy_urls_sorted_by_number_never_provider_url() {
        let channels = [
            ch("ESPN", "ESPN.us", 12, "http://provider.example/secret"),
            ch("CNN", "CNN.us", 5, "http://provider.example/cnn"),
        ];
        let json = lineup_json(&channels, "http://127.0.0.1:5004", None, None);
        assert!(json.contains("\"GuideNumber\":\"5\""));
        assert!(json.contains("\"GuideName\":\"CNN\""));
        assert!(json.contains("http://127.0.0.1:5004/auto/v5"));
        assert!(json.contains("http://127.0.0.1:5004/auto/v12"));
        assert!(json.contains("\"VideoCodec\":\"H264\""));
        assert!(json.contains("\"AudioCodec\":\"AAC\""));
        let mpeg = lineup_json(&channels, "http://127.0.0.1:5004", Some("MPEG2"), Some("AC3"));
        assert!(mpeg.contains("\"VideoCodec\":\"MPEG2\""));
        assert!(mpeg.contains("\"AudioCodec\":\"AC3\""));
        assert!(!json.contains("provider.example"));
        assert!(
            json.find("\"GuideNumber\":\"5\"").unwrap() < json.find("\"GuideNumber\":\"12\"").unwrap()
        );
    }

    #[test]
    fn discover_json_includes_stable_device_and_tuner_count() {
        let profile = TunerServerProfile {
            kind: "Plex".into(),
            enabled: false,
            running: false,
            friendly_name: "epg.monster studio (plex)".into(),
            device_id: "A1B2C3D4".into(),
            tuner_count: 2,
            bind_address: "127.0.0.1".into(),
            port: 8080,
            allow_lan: false,
            remux_enabled: true,
            downspiral_enabled: false,
        };
        let json = discover_json(&profile, "http://127.0.0.1:5004");
        assert!(json.contains("\"FriendlyName\":\"epg.monster studio (plex)\""));
        assert!(json.contains("\"DeviceID\":\"A1B2C3D4\""));
        assert!(json.contains("\"TunerCount\":2"));
        assert!(json.contains("\"LineupURL\":\"http://127.0.0.1:5004/lineup.json\""));
    }

    #[test]
    fn tuner_m3u_uses_local_urls_in_number_order() {
        let mut espn = ch("ESPN", "ESPN.us", 12, "");
        espn.group_title = "Sports".into();
        let mut cnn = ch("CNN", "CNN.us", 5, "");
        cnn.group_title = "News".into();
        let m3u = tuner_m3u(&[espn, cnn], "http://127.0.0.1:5005", None, true, false);
        assert!(m3u.starts_with("#EXTM3U url-tvg=\"http://127.0.0.1:5005/guide.xml\""));
        assert!(m3u.contains("tvg-id=\"CNN.us\""));
        assert!(m3u.contains("tvg-chno=\"5\""));
        assert!(m3u.contains("http://127.0.0.1:5005/auto/v5"));
        assert!(!m3u.contains("provider"));
        assert!(m3u.find("CNN").unwrap() < m3u.find("ESPN").unwrap());
    }

    #[test]
    fn tuner_m3u_remux_off_uses_visible_urls_and_member_epg() {
        let mut cnn = ch("CNN", "CNN.us", 5, "http://provider.example/cnn");
        cnn.group_title = "News".into();
        let m3u = tuner_m3u(
            &[cnn],
            "http://127.0.0.1:8083",
            Some("https://my.epg.monster/example/epg.xml.gz"),
            false,
            false,
        );
        assert!(m3u.contains("url-tvg=\"https://my.epg.monster/example/epg.xml.gz\""));
        assert!(m3u.contains("http://provider.example/cnn"));
        assert!(!m3u.contains("/auto/v5"));
    }

    #[test]
    fn tuner_m3u_use_local_logos_rewrites_tvg_logo() {
        let mut cnn = ch("CNN", "CNN.us2", 5, "");
        cnn.group_title = "News".into();
        cnn.tvg_logo = Some("https://cdn.example/cnn.png".into());
        let m3u = tuner_m3u(&[cnn], "http://127.0.0.1:8083", None, true, true);
        assert!(m3u.contains("tvg-logo=\"http://127.0.0.1:8083/logos/cnn.us2.png\""));
        assert!(!m3u.contains("cdn.example"));
    }

    #[test]
    fn downspiral_index_lists_one_playlist_per_group() {
        let mut cnn = ch("CNN", "CNN.us", 1, "");
        cnn.group_title = "News".into();
        let mut espn = ch("ESPN", "ESPN.us", 2, "");
        espn.group_title = "Sports".into();
        let mut msnbc = ch("MSNBC", "MSNBC.us", 3, "");
        msnbc.group_title = "News".into();
        assert_eq!(group_slug("News"), "news");
        let json = downspiral_index_json(&[cnn.clone(), espn, msnbc.clone()], "http://127.0.0.1:8081");
        assert!(json.contains("epg.monster.downspiral"));
        assert!(json.contains("/downspiral/news.m3u8"));
        assert!(json.contains("/downspiral/sports.xml"));
        assert!(!json.contains("provider"));
        let news = downspiral_lists(&[cnn, msnbc])
            .into_iter()
            .find(|g| g.slug == "news")
            .unwrap();
        let m3u = tuner_m3u(
            &news.channels,
            "http://127.0.0.1:8081",
            Some("http://127.0.0.1:8081/downspiral/news.xml"),
            true,
            false,
        );
        assert!(m3u.contains("CNN"));
        assert!(m3u.contains("MSNBC"));
        assert!(!m3u.contains("ESPN"));
    }

    #[test]
    fn tuner_advertised_epg_uses_member_feed_when_selected() {
        let mut s = AppSettings::default();
        s.tuner_use_member_epg = true;
        s.member_feed_url_gz = "https://my.epg.monster/example/epg.xml.gz".into();
        s.ensure_tuner_profiles();
        assert_eq!(
            s.tuner_advertised_epg(&s.iptv_tuner),
            "https://my.epg.monster/example/epg.xml.gz"
        );
        s.tuner_use_member_epg = false;
        assert!(s.tuner_advertised_epg(&s.iptv_tuner).ends_with("/guide.xml"));
    }

    #[test]
    fn guide_xml_lists_channels_by_number_and_includes_programmes() {
        let mut cnn = ch("CNN", "CNN.us", 5, "");
        cnn.tvg_logo = Some("http://logo/cnn.png".into());
        let programmes = [EpgProgramme {
            tvg_id: "CNN.us".into(),
            title: "News Hour".into(),
            description: Some("Live".into()),
            start_utc: "2026-08-15T12:00:00Z".into(),
            stop_utc: "2026-08-15T13:00:00Z".into(),
        }];
        let xml = guide_xml(&[cnn], &programmes, None, false);
        assert!(xml.contains("channel id=\"5\""));
        assert!(xml.contains("<display-name>CNN</display-name>"));
        assert!(xml.contains("<display-name>5</display-name>"));
        assert!(xml.contains("<display-name>CNN.us</display-name>"));
        assert!(
            xml.find("<display-name>CNN</display-name>").unwrap()
                < xml.find("<display-name>5</display-name>").unwrap()
        );
        assert!(xml.contains("channel=\"5\""));
        assert!(xml.contains("News Hour"));
        assert!(xml.contains("20260815120000 +0000"));
        assert!(!xml.contains("http://provider"));
    }

    #[test]
    fn guide_xml_unique_id_per_tuner_row_even_when_tvg_id_is_shared() {
        let a = ch("2 BROKE GIRLS", "24.7.Dummy.us", 1, "");
        let b = ch("ARCHER", "24.7.Dummy.us", 5, "");
        let programmes = [EpgProgramme {
            tvg_id: "24.7.Dummy.us".into(),
            title: "Dummy Block".into(),
            description: None,
            start_utc: "2026-08-15T12:00:00Z".into(),
            stop_utc: "2026-08-15T13:00:00Z".into(),
        }];
        let xml = guide_xml(&[a, b], &programmes, None, false);
        assert!(xml.contains("channel id=\"1\""));
        assert!(xml.contains("channel id=\"5\""));
        assert!(!xml.contains("channel id=\"24.7.Dummy.us\""));
        assert!(xml.contains("<display-name>2 BROKE GIRLS</display-name>"));
        assert!(xml.contains("<display-name>ARCHER</display-name>"));
        assert_eq!(xml.matches("<programme ").count(), 2);
        assert!(!xml.contains("channel=\"24.7.Dummy.us\""));
    }
}
