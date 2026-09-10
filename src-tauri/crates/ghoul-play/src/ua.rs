// SPDX-License-Identifier: GPL-3.0-or-later

//! User-Agent presets, independent of the playback engine.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UaPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub value: &'static str,
}

pub const TIVIMATE: UaPreset = UaPreset {
    id: "tivimate",
    label: "TiviMate 4.6.0",
    value: "TiviMate/4.6.0 (Linux; Android 11)",
};

pub const PRESETS: &[UaPreset] = &[
    TIVIMATE,
    UaPreset {
        id: "vlc",
        label: "VLC",
        value: "VLC/3.0.21 LibVLC/3.0.21",
    },
    UaPreset {
        id: "gse",
        label: "GSE Smart IPTV",
        value: "GSE SMART IPTV/2.9.9",
    },
    UaPreset {
        id: "smarters",
        label: "IPTV Smarters",
        value: "IPTVSmartersPlayer",
    },
    UaPreset {
        id: "browser",
        label: "Browser",
        value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    },
    UaPreset {
        id: "custom",
        label: "Custom",
        value: "",
    },
];

pub fn preset_by_id(id: &str) -> Option<&'static UaPreset> {
    PRESETS.iter().find(|p| p.id.eq_ignore_ascii_case(id.trim()))
}

/// Row / M3U UA wins when non-empty. Else the preset dropdown. An empty
/// Custom string is `tivimate`, never a blank User-Agent.
pub fn resolve_ua(row_ua: &str, preset_id: &str, custom: &str) -> String {
    let row = row_ua.trim();
    if !row.is_empty() {
        return row.to_string();
    }
    let id = preset_id.trim();
    if id.eq_ignore_ascii_case("custom") {
        let c = custom.trim();
        if !c.is_empty() {
            return c.to_string();
        }
        return TIVIMATE.value.to_string();
    }
    preset_by_id(id)
        .map(|p| p.value)
        .filter(|v| !v.is_empty())
        .unwrap_or(TIVIMATE.value)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_ua_wins_over_preset() {
        assert_eq!(
            resolve_ua("Foo/1", "vlc", ""),
            "Foo/1"
        );
    }

    #[test]
    fn preset_table_ids() {
        let ids: Vec<_> = PRESETS.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            vec!["tivimate", "vlc", "gse", "smarters", "browser", "custom"]
        );
    }

    #[test]
    fn empty_custom_is_tivimate() {
        assert_eq!(resolve_ua("", "custom", ""), TIVIMATE.value);
        assert_eq!(resolve_ua("", "custom", "   "), TIVIMATE.value);
        assert_eq!(resolve_ua("", "custom", "Mine/9"), "Mine/9");
    }

    #[test]
    fn unknown_or_empty_preset_is_tivimate() {
        assert_eq!(resolve_ua("", "", ""), TIVIMATE.value);
        assert_eq!(resolve_ua("", "nope", ""), TIVIMATE.value);
        assert_eq!(resolve_ua("", "vlc", ""), "VLC/3.0.21 LibVLC/3.0.21");
    }

    #[test]
    fn whitespace_row_falls_through() {
        assert_eq!(resolve_ua("  ", "gse", ""), "GSE SMART IPTV/2.9.9");
    }
}
