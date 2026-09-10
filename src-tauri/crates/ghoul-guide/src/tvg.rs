// SPDX-License-Identifier: GPL-3.0-or-later

//! Playlist `tvg-id` aliases, lowercased, for keying the guide snapshot.
//!
//! Not the same helper as `studio_core::epg::tvg_lookup_ids`, which preserves
//! case and also tries space/dot variants for catalog matching. These keys are
//! the map key inside the worker, so they must be lowercase and stable.

/// Aliases so `KAUT-DT.us_locals1.us (src05)` matches XMLTV `KAUT-DT.us`.
pub fn tvg_keys(id: &str) -> Vec<String> {
    let raw = id.trim();
    if raw.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: &str| {
        let k = s.trim().to_ascii_lowercase();
        if !k.is_empty() && !out.iter().any(|x| x == &k) {
            out.push(k);
        }
    };
    push(raw);
    if let Some((head, _)) = raw.split_once(" (") {
        push(head);
    }
    let base = raw.split(" (").next().unwrap_or(raw);
    let lower = base.to_ascii_lowercase();
    if let Some(idx) = lower.find(".us_locals") {
        let stem = &base[..idx];
        push(&format!("{stem}.us"));
        push(stem);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_locals_and_src() {
        let keys = tvg_keys("KAUT-DT.us_locals1.us (src05)");
        assert!(keys.iter().any(|k| k == "kaut-dt.us"));
        assert!(keys.iter().any(|k| k == "kaut-dt.us_locals1.us"));
        assert!(keys.iter().any(|k| k == "kaut-dt"));
    }

    #[test]
    fn case_insensitive_and_deduped() {
        let keys = tvg_keys("CNN.us");
        assert_eq!(keys, vec!["cnn.us".to_string()]);
    }

    #[test]
    fn empty_id_has_no_keys() {
        assert!(tvg_keys("   ").is_empty());
    }
}
