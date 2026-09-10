// SPDX-License-Identifier: GPL-3.0-or-later

//! M3U parsing for the Live TV lineup.
//!
//! One parse path for both hosts: studio writes its curated snapshot with the
//! same `#EXTVLCOPT:` / `#EXTHTTP:` tags an operator playlist would carry, so
//! `ghoul.exe` and the studio embed read identical bytes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// First entry of the category list, always present.
pub const ALL_CHANNELS: &str = "All channels";
/// Category for entries with no `group-title`.
pub const OTHER: &str = "Other";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub name: String,
    pub group: String,
    pub tvg_id: String,
    pub tvg_logo: String,
    pub url: String,
    /// Per-entry User-Agent. Empty means "fall back to the preset dropdown".
    pub ua: String,
    /// Extra request headers. Never contains `User-Agent`.
    pub headers: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub title: String,
    pub count: usize,
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

fn parse_attrs(s: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'-' || b[i] == b'_') {
            i += 1;
        }
        if i == start {
            i += 1;
            continue;
        }
        let key = s[start..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'"' {
            continue;
        }
        i += 1;
        let vs = i;
        while i < b.len() && b[i] != b'"' {
            i += 1;
        }
        let val = unescape(&s[vs..i.min(s.len())]);
        if i < b.len() {
            i += 1;
        }
        out.insert(key, val);
    }
    out
}

/// VLC option names that map onto a real request header.
fn vlcopt_header(key: &str) -> Option<&'static str> {
    match key {
        "http-referrer" | "http-referer" => Some("Referer"),
        "http-cookie" | "http-cookies" => Some("Cookie"),
        _ => None,
    }
}

#[derive(Default)]
struct Pending {
    extinf: Option<String>,
    ua: String,
    headers: BTreeMap<String, String>,
}

impl Pending {
    fn take_directive(&mut self, line: &str) -> bool {
        if let Some(rest) = strip_ci(line, "#EXTVLCOPT:") {
            let Some((key, value)) = rest.split_once('=') else {
                return true;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            if key == "http-user-agent" {
                self.ua = value.to_string();
            } else if let Some(name) = vlcopt_header(&key) {
                if !value.is_empty() {
                    self.headers.insert(name.to_string(), value.to_string());
                }
            }
            return true;
        }
        if let Some(rest) = strip_ci(line, "#EXTHTTP:") {
            for (k, v) in parse_json_headers(rest.trim()) {
                if k.eq_ignore_ascii_case("user-agent") {
                    if self.ua.is_empty() {
                        self.ua = v;
                    }
                } else {
                    self.headers.insert(k, v);
                }
            }
            return true;
        }
        // Bare `http-user-agent=…` continuation, seen in hand-rolled playlists.
        if let Some(rest) = strip_ci(line, "http-user-agent=") {
            self.ua = rest.trim().to_string();
            return true;
        }
        false
    }
}

fn strip_ci<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    if line.len() >= prefix.len() && line[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&line[prefix.len()..])
    } else {
        None
    }
}

/// Minimal flat `{"Key":"Value"}` reader. Not a general JSON parser: the tag is
/// always one flat object of string pairs, written by us or by another player.
fn parse_json_headers(raw: &str) -> Vec<(String, String)> {
    let mut chars = raw.chars().peekable();
    let mut strings: Vec<String> = Vec::new();
    while let Some(c) = chars.next() {
        if c != '"' {
            continue;
        }
        let mut buf = String::new();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => buf.push('\n'),
                    Some('t') => buf.push('\t'),
                    Some('r') => buf.push('\r'),
                    Some(other) => buf.push(other),
                    None => break,
                },
                other => buf.push(other),
            }
        }
        strings.push(buf);
    }
    strings
        .chunks_exact(2)
        .filter(|pair| !pair[0].trim().is_empty())
        .map(|pair| (pair[0].trim().to_string(), pair[1].clone()))
        .collect()
}

pub fn parse_m3u(content: &str) -> Vec<Channel> {
    let mut out = Vec::new();
    let mut pending = Pending::default();
    for raw in content.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if strip_ci(trimmed, "#EXTINF").is_some() {
            // A new #EXTINF abandons any directive that never reached a URL.
            pending = Pending {
                extinf: Some(trimmed.to_string()),
                ..Pending::default()
            };
            continue;
        }
        if pending.take_directive(trimmed) {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        let taken = std::mem::take(&mut pending);
        out.push(match taken.extinf {
            Some(extinf) => parse_extinf(&extinf, trimmed, taken.ua, taken.headers),
            None => Channel {
                name: trimmed.to_string(),
                group: OTHER.into(),
                url: trimmed.to_string(),
                ua: taken.ua,
                headers: taken.headers,
                ..Channel::default()
            },
        });
    }
    out
}

fn parse_extinf(
    extinf: &str,
    url: &str,
    ua: String,
    mut headers: BTreeMap<String, String>,
) -> Channel {
    let comma = extinf.rfind(',');
    let (attr_region_raw, name_raw) = match comma {
        Some(i) => (&extinf[..i], extinf[i + 1..].trim()),
        None => (extinf, "Unknown"),
    };
    let mut attr_region = if let Some(colon) = attr_region_raw.find(':') {
        attr_region_raw[colon + 1..].trim().to_string()
    } else {
        attr_region_raw.trim().to_string()
    };
    if let Some(space) = attr_region.find(' ') {
        if attr_region[..space].parse::<f64>().is_ok() {
            attr_region = attr_region[space..].trim().to_string();
        }
    } else if attr_region.parse::<f64>().is_ok() {
        attr_region.clear();
    }
    let attrs = parse_attrs(&attr_region);
    let get = |k: &str| {
        attrs
            .get(k)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    headers.retain(|k, v| !k.eq_ignore_ascii_case("user-agent") && !v.trim().is_empty());
    let name = unescape(name_raw);
    Channel {
        name: if name.is_empty() {
            url.trim().to_string()
        } else {
            name
        },
        group: get("group-title").unwrap_or_else(|| OTHER.into()),
        tvg_id: get("tvg-id").unwrap_or_default(),
        tvg_logo: get("tvg-logo").unwrap_or_default(),
        url: url.trim().to_string(),
        ua,
        headers,
    }
}

/// `All channels` first, then each distinct `group-title` in first-seen order.
pub fn categories(channels: &[Channel]) -> Vec<Category> {
    let mut out = vec![Category {
        title: ALL_CHANNELS.into(),
        count: channels.len(),
    }];
    for ch in channels {
        let title = if ch.group.trim().is_empty() {
            OTHER
        } else {
            ch.group.trim()
        };
        match out.iter_mut().find(|c| c.title.eq_ignore_ascii_case(title)) {
            Some(existing) => existing.count += 1,
            None => out.push(Category {
                title: title.to_string(),
                count: 1,
            }),
        }
    }
    out
}

/// Indices of the channels shown for a category. `All channels` keeps order.
pub fn in_category(channels: &[Channel], category: &str) -> Vec<usize> {
    if category.trim().is_empty() || category.eq_ignore_ascii_case(ALL_CHANNELS) {
        return (0..channels.len()).collect();
    }
    channels
        .iter()
        .enumerate()
        .filter(|(_, ch)| {
            let group = if ch.group.trim().is_empty() {
                OTHER
            } else {
                ch.group.trim()
            };
            group.eq_ignore_ascii_case(category.trim())
        })
        .map(|(i, _)| i)
        .collect()
}

/// Channel up/down. Wraps at both ends; an empty lineup stays at 0.
pub fn wrap_index(len: usize, current: usize, delta: i64) -> usize {
    if len == 0 {
        return 0;
    }
    let len_i = len as i64;
    let cur = (current.min(len - 1)) as i64;
    (((cur + delta) % len_i + len_i) % len_i) as usize
}

/// Case-insensitive substring match on the channel name.
pub fn search(channels: &[Channel], needle: &str) -> Vec<usize> {
    let needle = needle.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return (0..channels.len()).collect();
    }
    channels
        .iter()
        .enumerate()
        .filter(|(_, ch)| ch.name.to_ascii_lowercase().contains(&needle))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tvg_group_and_logo() {
        let m3u = "#EXTM3U\n\
#EXTINF:-1 tvg-id=\"CNN.us\" tvg-logo=\"http://logo/cnn.png\" group-title=\"News\",CNN\n\
http://example.com/cnn\n\
#EXTINF:-1 group-title=\"Sports\",ESPN\n\
http://example.com/espn\n";
        let ch = parse_m3u(m3u);
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].tvg_id, "CNN.us");
        assert_eq!(ch[0].tvg_logo, "http://logo/cnn.png");
        assert_eq!(ch[0].group, "News");
        assert!(ch[1].tvg_id.is_empty());
    }

    #[test]
    fn unescapes_amp() {
        let m3u = "#EXTM3U\n#EXTINF:-1 tvg-id=\"A&amp;E.us\" group-title=\"US Locals &amp; Regional\",Crime &amp; Investigation\nhttp://x/ae\n";
        let ch = parse_m3u(m3u);
        assert_eq!(ch[0].tvg_id, "A&E.us");
        assert_eq!(ch[0].group, "US Locals & Regional");
        assert_eq!(ch[0].name, "Crime & Investigation");
    }

    #[test]
    fn reads_per_entry_ua_and_headers() {
        let m3u = "#EXTM3U\n\
#EXTINF:-1 group-title=\"News\",CNN\n\
#EXTVLCOPT:http-user-agent=Foo/1\n\
#EXTVLCOPT:http-referrer=http://ref\n\
http://example.com/cnn\n";
        let ch = parse_m3u(m3u);
        assert_eq!(ch[0].ua, "Foo/1");
        assert_eq!(ch[0].headers.get("Referer").map(String::as_str), Some("http://ref"));
        assert!(!ch[0].headers.contains_key("User-Agent"));
    }

    #[test]
    fn reads_exthttp_header_object() {
        let m3u = "#EXTM3U\n\
#EXTINF:-1,Chan\n\
#EXTHTTP:{\"Referer\":\"http://ref\",\"X-Token\":\"abc\"}\n\
http://example.com/one\n";
        let ch = parse_m3u(m3u);
        assert_eq!(ch[0].headers.get("Referer").map(String::as_str), Some("http://ref"));
        assert_eq!(ch[0].headers.get("X-Token").map(String::as_str), Some("abc"));
    }

    #[test]
    fn directives_do_not_leak_to_the_next_entry() {
        let m3u = "#EXTM3U\n\
#EXTINF:-1,One\n\
#EXTVLCOPT:http-user-agent=Foo/1\n\
http://example.com/one\n\
#EXTINF:-1,Two\n\
http://example.com/two\n";
        let ch = parse_m3u(m3u);
        assert_eq!(ch[0].ua, "Foo/1");
        assert!(ch[1].ua.is_empty(), "second entry inherited a UA");
    }

    #[test]
    fn empty_group_becomes_other_and_all_channels_is_first() {
        let m3u = "#EXTM3U\n#EXTINF:-1,Nogroup\nhttp://x/1\n#EXTINF:-1 group-title=\"News\",CNN\nhttp://x/2\n";
        let ch = parse_m3u(m3u);
        let cats = categories(&ch);
        assert_eq!(cats[0].title, ALL_CHANNELS);
        assert_eq!(cats[0].count, 2);
        assert!(cats.iter().any(|c| c.title == OTHER && c.count == 1));
        assert_eq!(in_category(&ch, OTHER), vec![0]);
        assert_eq!(in_category(&ch, ALL_CHANNELS), vec![0, 1]);
    }

    #[test]
    fn wrap_zap_wraps_both_ends() {
        assert_eq!(wrap_index(3, 2, 1), 0);
        assert_eq!(wrap_index(3, 0, -1), 2);
        assert_eq!(wrap_index(3, 1, 1), 2);
        assert_eq!(wrap_index(3, 0, -4), 2);
        assert_eq!(wrap_index(0, 0, 1), 0);
        assert_eq!(wrap_index(2, 9, 1), 0);
    }

    #[test]
    fn search_is_case_insensitive_substring() {
        let m3u = "#EXTM3U\n#EXTINF:-1,CNN HD\nhttp://x/1\n#EXTINF:-1,ESPN\nhttp://x/2\n";
        let ch = parse_m3u(m3u);
        assert_eq!(search(&ch, "cnn"), vec![0]);
        assert_eq!(search(&ch, "n"), vec![0, 1]);
        assert!(search(&ch, "nothing").is_empty());
        assert_eq!(search(&ch, "  "), vec![0, 1]);
    }
}
