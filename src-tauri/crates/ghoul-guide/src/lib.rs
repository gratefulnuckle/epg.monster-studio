// SPDX-License-Identifier: GPL-3.0-or-later

//! G-houl guide: M3U parse, XMLTV worker, `tvg_keys`, snapshot.

pub mod fetch;
pub mod playlist;
pub mod tvg;
pub mod worker;
pub mod xmltv;

pub use fetch::{load_playlist_text, looks_like_http, redact_url};
pub use playlist::{
    categories, in_category, parse_m3u, search, wrap_index, Category, Channel, ALL_CHANNELS, OTHER,
};
pub use tvg::tvg_keys;
pub use worker::{start, Handle, NowOn, Snapshot};
pub use xmltv::{hhmm, parse_programmes, unix_now, xmltv_to_unix, Prog, WINDOW_BACK_SECS, WINDOW_FWD_SECS};
