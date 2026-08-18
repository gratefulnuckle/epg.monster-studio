// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use serde::Serialize;

use crate::info::{DISPLAY_NAME, VERSION};
use crate::models::ManagedChannel;

pub const SCHEMA_NAME: &str = "epg.monster.curation";
pub const SCHEMA_VERSION: i32 = 1;
pub const MAX_CHANNELS: i32 = 2500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurationChannel {
    pub tvg_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurationStudio {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurationDocument {
    pub schema: String,
    pub version: i32,
    pub replace: bool,
    pub rebuild: bool,
    pub studio: CurationStudio,
    pub channels: Vec<CurationChannel>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurationBuildResult {
    pub document: CurationDocument,
    pub managed_total: i32,
    pub included: i32,
    pub in_catalog: i32,
    pub skipped_no_tvg_id: i32,
    pub skipped_duplicate: i32,
    pub over_cap: i32,
    pub cap: i32,
}

pub fn build(
    channels: &[ManagedChannel],
    studio_version: &str,
    catalog_ids: Option<&HashSet<String>>,
    max_channels: Option<i32>,
) -> CurationBuildResult {
    let cap = max_channels.filter(|n| *n > 0).unwrap_or(MAX_CHANNELS);
    let mut ordered = channels.to_vec();
    ordered.sort_by(|a, b| {
        a.tuner_number
            .unwrap_or(i32::MAX)
            .cmp(&b.tuner_number.unwrap_or(i32::MAX))
            .then(a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
    });

    let mut skipped_empty = 0;
    let mut duplicate_rows = 0;
    let mut in_catalog = 0;
    let mut seen = HashSet::new();
    let mut rows = Vec::new();

    for ch in &ordered {
        let id = ch.tvg_id.as_deref().unwrap_or("").trim();
        if id.is_empty() {
            skipped_empty += 1;
            continue;
        }
        if !seen.insert(id.to_ascii_lowercase()) {
            duplicate_rows += 1;
        }
        if let Some(cat) = catalog_ids {
            if cat.iter().any(|c| c.eq_ignore_ascii_case(id)) {
                in_catalog += 1;
            }
        }
        rows.push(CurationChannel {
            tvg_id: id.to_string(),
            name: ch.name.clone(),
            logo: ch
                .tvg_logo
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            group: {
                let g = ch.group_title.trim();
                if g.is_empty() {
                    None
                } else {
                    Some(g.to_string())
                }
            },
            sort_order: ch.tuner_number.unwrap_or(ch.sort_order),
        });
    }

    let over = (rows.len() as i32 - cap).max(0);
    if rows.len() as i32 > cap {
        rows.truncate(cap as usize);
    }
    let included = rows.len() as i32;
    CurationBuildResult {
        document: CurationDocument {
            schema: SCHEMA_NAME.into(),
            version: SCHEMA_VERSION,
            replace: true,
            rebuild: true,
            studio: CurationStudio {
                name: DISPLAY_NAME.into(),
                version: if studio_version.trim().is_empty() {
                    VERSION.into()
                } else {
                    studio_version.into()
                },
            },
            channels: rows,
        },
        managed_total: channels.len() as i32,
        included,
        in_catalog,
        skipped_no_tvg_id: skipped_empty,
        skipped_duplicate: duplicate_rows,
        over_cap: over,
        cap,
    }
}

pub fn to_json(doc: &CurationDocument) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::StreamVariant;

    fn ch(name: &str, tvg: &str) -> ManagedChannel {
        ManagedChannel {
            id: name.into(),
            name: name.into(),
            group_title: "NEWS".into(),
            tvg_id: Some(tvg.into()),
            tvg_logo: None,
            notes: None,
            sort_order: 0,
            tvg_shift_hours: 0.0,
            in_tuner: false,
            tuner_number: None,
            variants: vec![],
            has_epg_match: false,
        }
    }

    #[test]
    fn build_skips_empty_tvg_id_and_never_includes_stream_urls() {
        let mut cnn = ch("CNN", "CNN.us2");
        cnn.tvg_logo = Some("https://logo.example/cnn.png".into());
        cnn.tuner_number = Some(5);
        cnn.variants.push(StreamVariant {
            id: "v".into(),
            managed_channel_id: "CNN".into(),
            url: "http://provider.example/secret".into(),
            label: None,
            source_entry_id: None,
            origin_name: None,
            origin_tvg_id: None,
            visibility: "visible".into(),
            priority: 0,
            last_audit_ok: None,
            last_audit_at: None,
        });
        let none = ch("NoId", "  ");
        let built = build(&[cnn, none], VERSION, None, None);
        assert_eq!(built.included, 1);
        assert_eq!(built.skipped_no_tvg_id, 1);
        assert_eq!(built.document.channels[0].tvg_id, "CNN.us2");
        assert_eq!(
            built.document.channels[0].logo.as_deref(),
            Some("https://logo.example/cnn.png")
        );
        let json = to_json(&built.document);
        assert!(json.contains("\"schema\": \"epg.monster.curation\""));
        assert!(json.contains("\"tvgId\": \"CNN.us2\""));
        assert!(!json.contains("provider.example"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn build_includes_unknown_tvg_ids_for_portal_missing_report() {
        let mut local = ch("Local", "Typo.Unknown.xyz");
        local.tuner_number = Some(1);
        let mut cnn = ch("CNN", "CNN.us2");
        cnn.tuner_number = Some(2);
        let catalog: HashSet<String> = ["CNN.us2".into()].into_iter().collect();
        let built = build(&[local, cnn], VERSION, Some(&catalog), Some(2500));
        assert_eq!(built.document.channels.len(), 2);
        assert!(built
            .document
            .channels
            .iter()
            .any(|c| c.tvg_id == "Typo.Unknown.xyz"));
        assert_eq!(built.in_catalog, 1);
    }

    #[test]
    fn build_sends_every_row_with_tvg_id_and_leaves_dups_for_the_api() {
        let mut a = ch("CNN East", "CNN.us2");
        a.tuner_number = Some(1);
        let mut b = ch("CNN West", "CNN.us2");
        b.tuner_number = Some(2);
        let built = build(&[a, b], VERSION, None, None);
        assert_eq!(built.included, 2);
        assert_eq!(built.skipped_duplicate, 1);
        assert_eq!(built.document.channels.len(), 2);
    }
}
