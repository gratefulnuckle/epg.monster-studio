// SPDX-License-Identifier: GPL-3.0-or-later

pub mod audit;
pub mod curation;
pub mod hdhr;
pub mod epg;
pub mod export;
pub mod logo;
pub mod lineup;
pub mod members;
pub mod issue;
pub mod crash;
pub mod info;
pub mod models;
pub mod parser;
pub mod paths;
pub mod player;
pub mod settings;
pub mod store;
pub mod tools;
pub mod bootstrap;

pub use info::{
    display_version, github_open_studio_issues, latest_github_tag, DISPLAY_NAME, PRODUCT_ID,
    USER_AGENT, VERSION,
};
pub use models::{ChannelEntry, EpgSuggestion, ManagedChannel, NowPlaying, StreamVariant};
pub use export::{export_all, export_visible_only};
pub use parser::parse_m3u;
pub use settings::AppSettings;
pub use store::SqliteStore;
