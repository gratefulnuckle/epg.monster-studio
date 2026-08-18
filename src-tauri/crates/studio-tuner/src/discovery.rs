// SPDX-License-Identifier: GPL-3.0-or-later

use studio_core::settings::TunerServerProfile;

pub const HDHR_PORT: u16 = 65001;
pub const SSDP_PORT: u16 = 1900;
pub const DISCOVER_REQ: u16 = 0x0002;
pub const DISCOVER_RPY: u16 = 0x0003;
pub const TAG_DEVICE_TYPE: u8 = 0x01;
pub const TAG_DEVICE_ID: u8 = 0x02;
pub const TAG_TUNER_COUNT: u8 = 0x10;
pub const TAG_LINEUP_URL: u8 = 0x27;
pub const TAG_BASE_URL: u8 = 0x2A;
pub const DEVICE_TYPE_TUNER: u32 = 1;

pub fn parse_device_id(hex: &str) -> u32 {
    let mut s = hex.trim();
    if s.len() >= 2 && s[..2].eq_ignore_ascii_case("0x") {
        s = &s[2..];
    }
    u32::from_str_radix(s, 16).unwrap_or(0)
}

pub fn build_discover_reply(profile: &TunerServerProfile, base_url: &str) -> Vec<u8> {
    let mut p = profile.clone();
    p.ensure_identity();
    let root = if base_url.trim().is_empty() {
        p.base_url()
    } else {
        base_url.trim_end_matches('/').to_string()
    };
    let mut payload = Vec::new();
    append_u32(&mut payload, TAG_DEVICE_TYPE, DEVICE_TYPE_TUNER);
    append_u32(&mut payload, TAG_DEVICE_ID, parse_device_id(&p.device_id));
    payload.push(TAG_TUNER_COUNT);
    payload.push(1);
    payload.push(p.tuner_count.clamp(1, 16) as u8);
    append_string(&mut payload, TAG_BASE_URL, &root);
    append_string(&mut payload, TAG_LINEUP_URL, &format!("{root}/lineup.json"));
    let mut body = Vec::with_capacity(8 + payload.len());
    body.push((DISCOVER_RPY >> 8) as u8);
    body.push((DISCOVER_RPY & 0xFF) as u8);
    body.push((payload.len() >> 8) as u8);
    body.push((payload.len() & 0xFF) as u8);
    body.extend_from_slice(&payload);
    let crc = crc32_mpeg(&body);
    body.push((crc >> 24) as u8);
    body.push((crc >> 16) as u8);
    body.push((crc >> 8) as u8);
    body.push(crc as u8);
    body
}

pub fn is_discover_request(buf: &[u8]) -> bool {
    buf.len() >= 4 && u16::from_be_bytes([buf[0], buf[1]]) == DISCOVER_REQ
}

pub fn parse_tags(packet: &[u8]) -> std::collections::HashMap<u8, Vec<u8>> {
    let mut tags = std::collections::HashMap::new();
    if packet.len() < 8 {
        return tags;
    }
    let len = ((packet[2] as usize) << 8) | packet[3] as usize;
    let end = (packet.len() - 4).min(4 + len);
    let mut i = 4;
    while i + 2 <= end {
        let tag = packet[i];
        let n = packet[i + 1] as usize;
        i += 2;
        if i + n > end {
            break;
        }
        tags.insert(tag, packet[i..i + n].to_vec());
        i += n;
    }
    tags
}

pub fn crc32_mpeg(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &b in data {
        crc ^= (b as u32) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    crc
}

pub fn ssdp_search_response(location: &str, device_id: &str, server: &str) -> String {
    let id = if device_id.trim().is_empty() {
        "epgmonster"
    } else {
        device_id
    };
    format!(
        "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nEXT:\r\nLOCATION: {location}\r\nSERVER: {server} UPnP/1.0\r\nST: upnp:rootdevice\r\nUSN: uuid:{id}::upnp:rootdevice\r\n\r\n"
    )
}

pub fn ssdp_notify(location: &str, device_id: &str, server: &str) -> String {
    let id = if device_id.trim().is_empty() {
        "epgmonster"
    } else {
        device_id
    };
    format!(
        "NOTIFY * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nCACHE-CONTROL: max-age=1800\r\nLOCATION: {location}\r\nNT: upnp:rootdevice\r\nNTS: ssdp:alive\r\nSERVER: {server} UPnP/1.0\r\nUSN: uuid:{id}::upnp:rootdevice\r\n\r\n"
    )
}

pub fn is_ssdp_search(text: &str) -> bool {
    text.len() >= 8 && text[..8].eq_ignore_ascii_case("M-SEARCH")
}

fn append_u32(payload: &mut Vec<u8>, tag: u8, value: u32) {
    payload.push(tag);
    payload.push(4);
    payload.extend_from_slice(&value.to_be_bytes());
}

fn append_string(payload: &mut Vec<u8>, tag: u8, value: &str) {
    let mut bytes = value.as_bytes().to_vec();
    if bytes.len() > 255 {
        bytes.truncate(255);
    }
    payload.push(tag);
    payload.push(bytes.len() as u8);
    payload.extend_from_slice(&bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use studio_core::info::USER_AGENT;
    use studio_core::settings::TunerServerProfile;

    #[test]
    fn hdhr_reply_roundtrips_device_and_urls() {
        let profile = TunerServerProfile {
            kind: "Plex".into(),
            enabled: true,
            running: false,
            friendly_name: "t".into(),
            device_id: "AABBCCDD".into(),
            tuner_count: 2,
            bind_address: "127.0.0.1".into(),
            port: 8080,
            allow_lan: false,
            remux_enabled: true,
            downspiral_enabled: false,
        };
        let pkt = build_discover_reply(&profile, "http://192.168.1.10:8080");
        assert_eq!(u16::from_be_bytes([pkt[0], pkt[1]]), DISCOVER_RPY);
        let tags = parse_tags(&pkt);
        assert!(tags.contains_key(&TAG_DEVICE_ID));
        let id = &tags[&TAG_DEVICE_ID];
        assert_eq!(
            u32::from_be_bytes([id[0], id[1], id[2], id[3]]),
            0xAABBCCDD
        );
        assert!(String::from_utf8_lossy(&tags[&TAG_BASE_URL]).contains("192.168.1.10:8080"));
        assert!(String::from_utf8_lossy(&tags[&TAG_LINEUP_URL]).contains("/lineup.json"));
        let body_len = pkt.len() - 4;
        let crc = crc32_mpeg(&pkt[..body_len]);
        let got = u32::from_be_bytes([
            pkt[body_len],
            pkt[body_len + 1],
            pkt[body_len + 2],
            pkt[body_len + 3],
        ]);
        assert_eq!(crc, got);
    }

    #[test]
    fn ssdp_search_and_notify() {
        assert!(is_ssdp_search("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n"));
        assert!(!is_ssdp_search("NOTIFY * HTTP/1.1"));
        let r = ssdp_search_response("http://127.0.0.1:8080/discover.json", "DEADBEEF", USER_AGENT);
        assert!(r.contains("LOCATION: http://127.0.0.1:8080/discover.json"));
        let n = ssdp_notify("http://192.168.1.10:8080/discover.json", "AABBCCDD", USER_AGENT);
        assert!(n.starts_with("NOTIFY * HTTP/1.1"));
        assert!(n.contains("ssdp:alive"));
    }
}
