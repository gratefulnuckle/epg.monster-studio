// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::time::Duration;

use crate::logo::PLAYER_UA;

/// Xtream Codes panel login → M3U via get.php (type=m3u_plus).

pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn normalize_base(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("Server URL is required.".into());
    }
    let s = if s.contains("://") {
        s.to_string()
    } else {
        format!("http://{s}")
    };
    let scheme = s
        .split_once("://")
        .map(|(sch, _)| sch)
        .unwrap_or("http");
    if scheme != "http" && scheme != "https" {
        return Err("Server URL must be http or https.".into());
    }
    let after = s.split_once("://").map(|(_, r)| r).unwrap_or(&s);
    let after = after.split(['?', '#']).next().unwrap_or(after);
    let hostport = after.split('/').next().unwrap_or(after).trim();
    if hostport.is_empty() {
        return Err("Server URL is missing a host.".into());
    }
    Ok(format!("{scheme}://{hostport}"))
}

pub fn playlist_url(
    server: &str,
    username: &str,
    password: &str,
    output: &str,
) -> Result<String, String> {
    let base = normalize_base(server)?;
    let user = username.trim();
    let pass = password.trim();
    if user.is_empty() || pass.is_empty() {
        return Err("Username and password are required.".into());
    }
    let output_l = output.trim().to_ascii_lowercase();
    let output = match output_l.as_str() {
        "" | "ts" | "mpegts" => "ts",
        "m3u8" | "hls" => "m3u8",
        other if other.chars().all(|c| c.is_ascii_alphanumeric()) => other,
        _ => "ts",
    };
    Ok(format!(
        "{base}/get.php?username={}&password={}&type=m3u_plus&output={}",
        percent_encode(user),
        percent_encode(pass),
        percent_encode(output)
    ))
}

pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn player_api_url(server: &str, username: &str, password: &str) -> Result<String, String> {
    let base = normalize_base(server)?;
    let user = username.trim();
    let pass = password.trim();
    if user.is_empty() || pass.is_empty() {
        return Err("Username and password are required.".into());
    }
    Ok(format!(
        "{base}/player_api.php?username={}&password={}",
        percent_encode(user),
        percent_encode(pass)
    ))
}

pub fn parse_login(location: &str) -> Option<(String, String, String)> {
    let loc = location.trim();
    if loc.is_empty() {
        return None;
    }
    let base = normalize_base(loc).ok()?;
    let q = loc.split_once('?')?.1;
    let mut user = String::new();
    let mut pass = String::new();
    for part in q.split('&') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let v = percent_decode(v);
        match k {
            "username" => user = v,
            "password" => pass = v,
            _ => {}
        }
    }
    if user.is_empty() || pass.is_empty() {
        return None;
    }
    Some((base, user, pass))
}

pub fn parse_exp_date(body: &str) -> Option<i64> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let exp = v.get("user_info").and_then(|u| u.get("exp_date"))?;
    match exp {
        serde_json::Value::Null => None,
        serde_json::Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().map(|u| u as i64))
            .filter(|i| *i > 0),
        serde_json::Value::String(s) => {
            let s = s.trim();
            if s.is_empty() || s.eq_ignore_ascii_case("null") || s == "0" {
                return None;
            }
            s.parse::<i64>().ok().filter(|i| *i > 0)
        }
        _ => None,
    }
}

pub fn expiry_label(exp: Option<i64>, now: i64) -> Option<String> {
    let exp = exp?;
    let secs = exp - now;
    if secs <= 0 {
        let days = (-secs) / 86_400;
        return Some(if days <= 0 {
            "expired".into()
        } else {
            format!("expired {days}d ago")
        });
    }
    let days = secs / 86_400;
    if days < 1 {
        let hours = (secs + 3_599) / 3_600;
        return Some(format!("expiry in {hours}h"));
    }
    Some(format!("expiry in {days}d"))
}

pub fn fetch_exp_date(
    server: &str,
    username: &str,
    password: &str,
    headers: &BTreeMap<String, String>,
) -> Option<i64> {
    let url = player_api_url(server, username, password).ok()?;
    let mut req = ureq::get(&url).timeout(Duration::from_secs(8));
    let mut has_ua = false;
    for (k, v) in headers {
        if k.eq_ignore_ascii_case("user-agent") {
            has_ua = true;
        }
        req = req.set(k, v);
    }
    if !has_ua {
        req = req.set("User-Agent", PLAYER_UA);
    }
    let body = req.call().ok()?.into_string().ok()?;
    parse_exp_date(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_url_encodes_password_and_strips_path() {
        let url = playlist_url("http://dns.example:8080/c/", "user", "p@ss word", "ts").unwrap();
        assert_eq!(
            url,
            "http://dns.example:8080/get.php?username=user&password=p%40ss%20word&type=m3u_plus&output=ts"
        );
    }

    #[test]
    fn normalize_base_adds_http_and_drops_get_php() {
        assert_eq!(
            normalize_base("dns.example:8080/get.php?username=x").unwrap(),
            "http://dns.example:8080"
        );
        assert_eq!(
            normalize_base("https://host:443/player_api.php").unwrap(),
            "https://host:443"
        );
    }

    #[test]
    fn rejects_empty_credentials() {
        let err = playlist_url("http://h", "  ", "p", "ts").unwrap_err();
        assert!(err.to_ascii_lowercase().contains("username"));
    }

    #[test]
    fn parse_exp_date_reads_unix_string() {
        let body = r#"{"user_info":{"exp_date":"1738368000","status":"Active"}}"#;
        assert_eq!(parse_exp_date(body), Some(1_738_368_000));
        assert_eq!(parse_exp_date(r#"{"user_info":{"exp_date":null}}"#), None);
        assert_eq!(parse_exp_date(r#"{"user_info":{"exp_date":"0"}}"#), None);
    }

    #[test]
    fn expiry_label_uses_days() {
        let now = 1_700_000_000;
        let in_29d = now + 29 * 86_400 + 3_600;
        assert_eq!(
            expiry_label(Some(in_29d), now).as_deref(),
            Some("expiry in 29d")
        );
        assert_eq!(
            expiry_label(Some(now - 3 * 86_400), now).as_deref(),
            Some("expired 3d ago")
        );
        assert_eq!(expiry_label(None, now), None);
    }

    #[test]
    fn parse_login_from_get_php() {
        let (base, user, pass) = parse_login(
            "http://dns.example:8080/get.php?username=user&password=p%40ss&type=m3u_plus&output=ts",
        )
        .unwrap();
        assert_eq!(base, "http://dns.example:8080");
        assert_eq!(user, "user");
        assert_eq!(pass, "p@ss");
    }
}
