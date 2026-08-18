// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct ChannelEntry {
    pub id: String,
    pub source_id: String,
    pub group_title: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvg_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvg_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tvg_logo: Option<String>,
    pub tvg_shift_hours: f64,
    pub url: String,
    pub attrs_json: String,
    pub line_no: i32,
}

impl Default for ChannelEntry {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            source_id: String::new(),
            group_title: "Ungrouped".into(),
            name: String::new(),
            tvg_id: None,
            tvg_name: None,
            tvg_logo: None,
            tvg_shift_hours: 0.0,
            url: String::new(),
            attrs_json: "{}".into(),
            line_no: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub struct PlaylistSource {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub location: String,
    pub headers_json: String,
    pub channel_count: i32,
}
