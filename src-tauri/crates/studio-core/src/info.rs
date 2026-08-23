// SPDX-License-Identifier: GPL-3.0-or-later

pub const VERSION: &str = "v2.0.2";
pub const EDITION: &str = "2026";
pub const USER_AGENT: &str = "epg.monster-studio/v2.0.2";
pub const DISPLAY_NAME: &str = "epg.monster studio";
pub const PRODUCT_ID: &str = "epg.monster-studio";
pub const GITHUB_REPO: &str = "gratefulnuckle/epg.monster-studio";
pub const GITHUB_RELEASES_LATEST: &str =
    "https://github.com/gratefulnuckle/epg.monster-studio/releases/latest";

/// Splash / About line. CI sets STUDIO_BUILD + STUDIO_SHA; local builds are `(dev)`.
pub fn display_version() -> String {
    let build = option_env!("STUDIO_BUILD");
    let sha = option_env!("STUDIO_SHA");
    match (build, sha) {
        (Some(n), Some(s)) => format!("{EDITION} edition · {VERSION} (build {n} · {s})"),
        (Some(n), None) => format!("{EDITION} edition · {VERSION} (build {n})"),
        (None, Some(s)) => format!("{EDITION} edition · {VERSION} (dev · {s})"),
        (None, None) => format!("{EDITION} edition · {VERSION} (dev)"),
    }
}

#[derive(Debug, Clone)]
pub struct GithubRelease {
    pub tag: String,
    pub html_url: String,
    pub body: Option<String>,
}

fn github_status_message(status: u16) -> String {
    match status {
        404 => format!("No GitHub release yet for {GITHUB_REPO}."),
        403 => "GitHub rate-limited the update check. Try again later.".into(),
        n => format!("GitHub latest release returned HTTP {n}."),
    }
}

/// Latest GitHub release for this v2 repo. Does not log URLs or keys.
pub fn latest_github_release() -> Result<GithubRelease, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let resp = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call();
    let body = match resp {
        Ok(r) => r.into_string().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(code, _)) => return Err(github_status_message(code)),
        Err(e) => return Err(e.to_string()),
    };
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no release tag".to_string())?;
    let html_url = v
        .get("html_url")
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| GITHUB_RELEASES_LATEST.to_string());
    let body = v
        .get("body")
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(GithubRelease {
        tag,
        html_url,
        body,
    })
}

/// Latest GitHub release tag. Does not log URLs or keys.
pub fn latest_github_tag() -> Result<String, String> {
    latest_github_release().map(|r| r.tag)
}

pub fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches('v');
    let mut nums = s.split(|c: char| !c.is_ascii_digit());
    let major = nums.next()?.parse().ok()?;
    let minor = nums.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let patch = nums.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

pub fn remote_is_newer(remote: &str, local: &str) -> bool {
    match (parse_semver(remote), parse_semver(local)) {
        (Some(r), Some(l)) => r > l,
        _ => {
            remote.trim().trim_start_matches('v') != local.trim().trim_start_matches('v')
        }
    }
}

/// Open studio issues from the public board (project 2 / `product:studio`).
pub fn github_open_studio_issues() -> Result<(u32, Option<String>), String> {
    let url = "https://api.github.com/search/issues?q=repo:gratefulnuckle/all-monster-issues+state:open+label:product:studio";
    let body = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let n = v.get("total_count").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
    let title = v
        .get("items")
        .and_then(|i| i.as_array())
        .and_then(|a| a.first())
        .and_then(|it| it.get("title"))
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Ok((n, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_strips_v_prefix() {
        assert_eq!(parse_semver("v2.0.0"), Some((2, 0, 0)));
        assert_eq!(parse_semver("2.1.3"), Some((2, 1, 3)));
        assert_eq!(parse_semver("v2.0.1-beta"), Some((2, 0, 1)));
    }

    #[test]
    fn remote_is_newer_compares_triples() {
        assert!(!remote_is_newer("v2.0.0", "v2.0.0"));
        assert!(!remote_is_newer("2.0.0", VERSION));
        assert!(remote_is_newer("v2.0.1", "v2.0.0"));
        assert!(remote_is_newer("v2.1.0", "v2.0.9"));
        assert!(!remote_is_newer("v1.9.9", "v2.0.0"));
    }
}
