// SPDX-License-Identifier: GPL-3.0-or-later

//! G-houl playback: one live engine, UA presets, availability.

pub mod engine;
pub mod gst_sidecar;
pub mod libmpv;
pub mod mpv_ipc;
pub mod paths;
pub mod session;
pub mod ua;

pub use engine::{
    availability, choose_engine, Availability, Engine, EngineId, PlayRequest, PlayState,
};
pub use session::Session;
pub use ua::{preset_by_id, resolve_ua, UaPreset, PRESETS, TIVIMATE};
