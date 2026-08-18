// SPDX-License-Identifier: GPL-3.0-or-later

pub const VERSION: &str = "v2.0.0";
pub const USER_AGENT: &str = "epg.monster-studio/v2.0.0";
pub const DISPLAY_NAME: &str = "epg.monster studio";
pub const PRODUCT_ID: &str = "epg.monster-studio";

/// Splash / About line. CI sets STUDIO_BUILD + STUDIO_SHA; local builds are `(dev)`.
pub fn display_version() -> String {
    let build = option_env!("STUDIO_BUILD");
    let sha = option_env!("STUDIO_SHA");
    match (build, sha) {
        (Some(n), Some(s)) => format!("v2.0.0 (build {n} · {s})"),
        (Some(n), None) => format!("v2.0.0 (build {n})"),
        (None, Some(s)) => format!("v2.0.0 (dev · {s})"),
        (None, None) => "v2.0.0 (dev)".into(),
    }
}

/// Latest GitHub release tag. Does not log URLs or keys.
pub fn latest_github_tag() -> Result<String, String> {
    let body = ureq::get(
        "https://api.github.com/repos/gratefulnuckle/epg.monster-studio-tauri/releases/latest",
    )
    .set("User-Agent", USER_AGENT)
    .set("Accept", "application/vnd.github+json")
    .call()
    .map_err(|e| e.to_string())?
    .into_string()
    .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    v.get("tag_name")
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "no release tag".into())
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
