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
#[serde(rename_all = "camelCase")]
pub struct StreamVariant {
    pub id: String,
    pub managed_channel_id: String,
    pub url: String,
    pub label: Option<String>,
    pub source_entry_id: Option<String>,
    pub origin_name: Option<String>,
    pub origin_tvg_id: Option<String>,
    pub visibility: String,
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_audit_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_audit_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedChannel {
    pub id: String,
    pub name: String,
    pub group_title: String,
    pub tvg_id: Option<String>,
    pub tvg_logo: Option<String>,
    pub notes: Option<String>,
    pub sort_order: i32,
    pub tvg_shift_hours: f64,
    pub in_tuner: bool,
    pub tuner_number: Option<i32>,
    pub variants: Vec<StreamVariant>,
    pub has_epg_match: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EpgSuggestion {
    pub tvg_id: String,
    pub name: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EpgAuditRow {
    pub managed_channel_id: String,
    pub channel_name: String,
    pub group_title: String,
    pub current_tvg_id: Option<String>,
    /// matched | missing | unknown
    pub status: String,
    pub suggested_tvg_id: Option<String>,
    pub suggested_name: Option<String>,
    pub suggested_logo: Option<String>,
    pub score: f64,
    pub second_score: f64,
    pub match_kind: Option<String>,
}

impl EpgAuditRow {
    pub fn has_suggestion(&self) -> bool {
        self.suggested_tvg_id.as_deref().is_some_and(|s| !s.is_empty())
    }

    pub fn is_unique_suggestion(&self) -> bool {
        self.score >= 0.98 || self.score - self.second_score >= 0.10
    }

    pub fn status_label(&self) -> &'static str {
        match self.status.as_str() {
            "matched" => "Matched",
            "unknown" => "Unknown ID",
            _ => "Missing ID",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub tvg_id: String,
    pub name: String,
    pub logo: Option<String>,
    pub section: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub start_local: String,
    pub stop_local: String,
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
