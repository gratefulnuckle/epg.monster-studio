// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use gstreamer as gst;
use gstreamer::glib;
use gstreamer::glib::prelude::StaticType;
use gstreamer::prelude::*;
use gstreamer_video::prelude::*;

pub enum Note {
    Playing,
    Paused,
    Buffering(u8),
    Error(String),
    Eos,
}

pub struct Player {
    playbin: gst::Element,
    bus: gst::Bus,
    hwnd: usize,
}

impl Player {
    pub fn new(
        url: &str,
        ua: &str,
        headers: &BTreeMap<String, String>,
        hwnd: usize,
    ) -> Result<Self, String> {
        gst::init().map_err(|e| format!("gstreamer init: {e}"))?;
        crate::log::line("info", &format!("gstreamer {}", gst::version_string()));

        let playbin = gst::ElementFactory::make("playbin")
            .name("ghoul")
            .build()
            .map_err(|_| {
                "playbin missing — install GStreamer plugins-base (Linux: gstreamer1.0-plugins-base)"
                    .to_string()
            })?;

        if let Some(sink) = make_video_sink() {
            crate::log::line("info", &format!("video-sink {}", sink.name()));
            playbin.set_property("video-sink", &sink);
        } else {
            crate::log::line("warn", "no dedicated video sink — playbin autovideosink");
        }
        if let Some(sink) = make_audio_sink() {
            crate::log::line("info", &format!("audio-sink {}", sink.name()));
            playbin.set_property("audio-sink", &sink);
        }

        let ua = ua.to_string();
        let headers = headers.clone();
        playbin.connect("source-setup", false, move |values| {
            let Ok(source) = values[1].get::<gst::Element>() else {
                return None;
            };
            let factory = source
                .factory()
                .map(|f| f.name().to_string())
                .unwrap_or_else(|| source.name().to_string());
            crate::log::line("info", &format!("source-setup {factory}"));
            if source.find_property("user-agent").is_some() {
                source.set_property("user-agent", ua.as_str());
            }
            let http =
                factory.contains("soup") || factory.contains("http") || factory.contains("curl");
            if http && source.find_property("is-live").is_some() {
                source.set_property("is-live", true);
            }
            if http && source.find_property("iradio-mode").is_some() {
                source.set_property("iradio-mode", true);
            }
            if let Some(pspec) = source.find_property("timeout") {
                if pspec.value_type() == u32::static_type() {
                    source.set_property("timeout", 15u32);
                }
            }
            if source.find_property("extra-headers").is_some() && !headers.is_empty() {
                let mut extra = gst::Structure::new_empty("extra-headers");
                for (k, v) in &headers {
                    extra.set(k.as_str(), v.as_str());
                }
                source.set_property("extra-headers", extra);
            }
            None
        });

        if !url.trim().is_empty() {
            playbin.set_property("uri", url);
        }

        let bus = playbin.bus().ok_or("playbin has no bus")?;
        let hwnd_sync = hwnd;
        bus.set_sync_handler(move |_, msg| {
            if gstreamer_video::is_video_overlay_prepare_window_handle_message(msg) {
                if let Some(src) = msg.src() {
                    if let Ok(overlay) = src
                        .clone()
                        .dynamic_cast::<gstreamer_video::VideoOverlay>()
                    {
                        unsafe { overlay.set_window_handle(hwnd_sync) };
                    }
                }
                return gst::BusSyncReply::Drop;
            }
            gst::BusSyncReply::Pass
        });
        Ok(Self { playbin, bus, hwnd })
    }

    fn overlay(&self) -> Option<gstreamer_video::VideoOverlay> {
        self.playbin
            .clone()
            .dynamic_cast::<gstreamer_video::VideoOverlay>()
            .ok()
    }

    fn apply_handle(&self) {
        if let Some(overlay) = self.overlay() {
            unsafe { overlay.set_window_handle(self.hwnd) };
        }
    }

    /// Overlay render rectangle in the native window's client space.
    ///
    /// On Windows the overlay HWND is already a child placed in the stage —
    /// pass `0, 0, child_w, child_h`. Do not pass the parent stage origin
    /// (header offset): that insets the picture and leaves a white/empty strip.
    pub fn set_render_rect(&self, x: i32, y: i32, w: i32, h: i32) {
        if w <= 0 || h <= 0 {
            return;
        }
        if let Some(overlay) = self.overlay() {
            let _ = overlay.set_render_rectangle(x, y, w, h);
            overlay.expose();
        }
    }

    /// No-op on purpose: d3d11videosink crashes if we set_render_rectangle
    /// (including the -1,-1 "unset" sentinel) after it subclasses the HWND.
    pub fn fill_window(&self) {}

    pub fn video_size(&self) -> Option<(u32, u32)> {
        let sink = self
            .playbin
            .property_value("video-sink")
            .get::<gst::Element>()
            .ok()?;
        for pad in sink.pads() {
            let Some(caps) = pad.current_caps() else {
                continue;
            };
            let Some(st) = caps.structure(0) else {
                continue;
            };
            let w = st.get::<i32>("width").unwrap_or(0);
            let h = st.get::<i32>("height").unwrap_or(0);
            if w > 0 && h > 0 {
                return Some((w as u32, h as u32));
            }
        }
        None
    }

    pub fn play(&self) -> Result<(), String> {
        self.apply_handle();
        if let Err(e) = self.playbin.set_state(gst::State::Playing) {
            crate::log::line("error", &format!("gstreamer play: {e}"));
            return Err("Stream failed".into());
        }
        crate::log::line("info", "pipeline → Playing");
        Ok(())
    }

    pub fn open_uri(&self, uri: &str) -> Result<(), String> {
        let uri = uri.trim();
        if uri.is_empty() {
            return Err("empty URI".into());
        }
        crate::log::line("info", &format!("open {}", crate::log::redact_url(uri)));
        let _ = self.playbin.set_state(gst::State::Null);
        let _ = self.playbin.state(gst::ClockTime::from_seconds(2));
        self.playbin.set_property("uri", uri);
        self.apply_handle();
        self.play()
    }

    pub fn path_to_uri(path: &std::path::Path) -> Result<String, String> {
        let abs = match path.canonicalize() {
            Ok(p) => p,
            Err(_) if path.is_absolute() => path.to_path_buf(),
            Err(_) => std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf()),
        };
        let s = abs.to_string_lossy();
        // Windows canonicalize() yields \\?\C:\...; url::Url wants a normal path.
        let cleaned = s.strip_prefix(r"\\?\").unwrap_or(&s);
        url::Url::from_file_path(std::path::Path::new(cleaned))
            .map(|u| u.to_string())
            .map_err(|_| format!("cannot convert path to URI: {}", path.display()))
    }

    pub fn pause(&self) {
        let _ = self.playbin.set_state(gst::State::Paused);
    }

    pub fn toggle(&self) -> bool {
        if self.playbin.current_state() == gst::State::Playing {
            self.pause();
            false
        } else {
            let _ = self.play();
            true
        }
    }

    pub fn set_volume(&self, v: f64) {
        self.playbin.set_property("volume", v.clamp(0.0, 1.0));
    }

    pub fn set_mute(&self, mute: bool) {
        self.playbin.set_property("mute", mute);
    }

    pub fn position_ns(&self) -> Option<u64> {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map(|t| t.nseconds())
    }

    pub fn duration_ns(&self) -> Option<u64> {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(|t| t.nseconds())
    }

    pub fn position_secs(&self) -> f64 {
        self.position_ns()
            .map(|ns| ns as f64 / 1_000_000_000.0)
            .unwrap_or(0.0)
    }

    pub fn duration_secs(&self) -> f64 {
        self.duration_ns()
            .map(|ns| ns as f64 / 1_000_000_000.0)
            .unwrap_or(0.0)
    }

    pub fn seek_rel(&self, seconds: f64) {
        if !seconds.is_finite() {
            return;
        }
        let seconds = seconds.clamp(-1.0e7, 1.0e7);
        let pos = self.position_ns().unwrap_or(0) as i128;
        let delta = (seconds * 1_000_000_000.0).round() as i128;
        self.seek_to_ns(clamp_seek_ns(pos + delta, self.duration_ns()));
    }

    pub fn seek_frac(&self, frac: f64) {
        let Some(dur) = self.duration_ns() else {
            return;
        };
        self.seek_to_ns(frac_to_ns(frac, dur));
    }

    fn seek_to_ns(&self, ns: u64) {
        let ns = ns.min(gst::ClockTime::MAX.nseconds());
        let _ = self.playbin.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
            gst::ClockTime::from_nseconds(ns),
        );
    }

    pub fn poll(&self) -> Vec<Note> {
        let mut out = Vec::new();
        while let Some(msg) = self.bus.pop() {
            if gstreamer_video::is_video_overlay_prepare_window_handle_message(&msg) {
                self.apply_handle();
                continue;
            }
            match msg.view() {
                gst::MessageView::Error(e) => {
                    let dbg = e.debug().unwrap_or_default();
                    let text = format!("{} {dbg}", e.error());
                    crate::log::line("error", &text);
                    out.push(Note::Error(short_error(&text)));
                }
                gst::MessageView::Eos(_) => {
                    crate::log::line("info", "EOS");
                    out.push(Note::Eos);
                }
                gst::MessageView::Buffering(b) => {
                    let p = b.percent() as u8;
                    if p < 100 {
                        out.push(Note::Buffering(p));
                    } else {
                        out.push(Note::Playing);
                    }
                }
                gst::MessageView::StateChanged(s) => {
                    if s.current() == gst::State::Playing && s.pending() == gst::State::VoidPending
                    {
                        out.push(Note::Playing);
                    }
                    if s.current() == gst::State::Paused && s.pending() == gst::State::VoidPending {
                        out.push(Note::Paused);
                    }
                }
                gst::MessageView::Warning(w) => {
                    crate::log::line("warn", &format!("{} {:?}", w.error(), w.debug()));
                }
                _ => {}
            }
        }
        out
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.playbin.set_state(gst::State::Null);
    }
}

fn make_audio_sink() -> Option<gst::Element> {
    // WASAPI shared mode is what Windows Volume Mixer lists. Do not probe
    // set_state(Ready) here — wasapi2sink fails that before playbin has a
    // clock, which used to force DirectSound (audio plays, no mixer row).
    let descs = if cfg!(windows) {
        &[
            "audioconvert ! audioresample ! wasapi2sink exclusive=false low-latency=false",
            "audioconvert ! audioresample ! wasapisink exclusive=false",
            "audioconvert ! audioresample ! directsoundsink",
            "audioconvert ! audioresample ! autoaudiosink",
        ][..]
    } else {
        &["audioconvert ! audioresample ! autoaudiosink"][..]
    };
    for desc in descs {
        match gst::parse::bin_from_description(desc, true) {
            Ok(bin) => {
                bin.set_property("name", "ghoul-asink");
                crate::log::line("info", &format!("audio pipeline {desc}"));
                return Some(bin.upcast());
            }
            Err(e) => crate::log::line("warn", &format!("audio pipeline `{desc}`: {e}")),
        }
    }
    None
}

fn make_video_sink() -> Option<gst::Element> {
    let names = if cfg!(windows) {
        // d3d11videosink subclasses our HWND and has been crashing the process
        // right after "pipeline → Playing" with no Rust log.
        &[
            "glimagesink",
            "d3dvideosink",
            "d3d11videosink",
            "autovideosink",
        ][..]
    } else if cfg!(target_os = "macos") {
        &["glimagesink", "osxvideosink", "autovideosink"][..]
    } else {
        &[
            "glimagesink",
            "gtk4paintablesink",
            "ximagesink",
            "autovideosink",
        ][..]
    };
    for name in names {
        if gst::ElementFactory::find(name).is_none() {
            continue;
        }
        let Ok(sink) = gst::ElementFactory::make(*name).name("ghoul-vsink").build() else {
            continue;
        };
        paint_sink_black(&sink);
        if sink.find_property("enable-navigation-events").is_some() {
            sink.set_property("enable-navigation-events", false);
        }
        return Some(sink);
    }
    None
}

fn paint_sink_black(sink: &gst::Element) {
    if sink.find_property("force-aspect-ratio").is_some() {
        sink.set_property("force-aspect-ratio", true);
    }
    if let Some(pspec) = sink.find_property("background-color") {
        set_prop_zero(sink, "background-color", pspec.value_type());
    }
    if let Some(pspec) = sink.find_property("fill-border") {
        let ty = pspec.value_type();
        if ty == bool::static_type() {
            sink.set_property("fill-border", true);
        } else {
            set_prop_zero(sink, "fill-border", ty);
        }
    }
}

fn set_prop_zero(el: &gst::Element, name: &str, ty: glib::Type) {
    if ty == u32::static_type() {
        el.set_property(name, 0u32);
    } else if ty == i32::static_type() {
        el.set_property(name, 0i32);
    } else if ty == u64::static_type() {
        el.set_property(name, 0u64);
    } else if ty == i64::static_type() {
        el.set_property(name, 0i64);
    }
}

fn clamp_seek_ns(pos: i128, duration: Option<u64>) -> u64 {
    let pos = pos.max(0);
    match duration {
        Some(d) => pos.min(d as i128).max(0) as u64,
        None => pos as u64,
    }
}

fn frac_to_ns(frac: f64, duration: u64) -> u64 {
    if duration == 0 || frac.is_nan() {
        return 0;
    }
    let f = frac.clamp(0.0, 1.0);
    let ns = (duration as f64 * f).round();
    if ns <= 0.0 {
        0
    } else if ns >= duration as f64 {
        duration
    } else {
        ns as u64
    }
}

fn short_error(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("not-linked") || lower.contains("no decoder") {
        return "Stream failed (codec)".into();
    }
    "Stream failed".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn path_to_uri_relative_starts_with_file() {
        let uri = Player::path_to_uri(Path::new("clip.mp4")).expect("uri");
        assert!(uri.starts_with("file:"), "got {uri}");
    }

    #[test]
    fn seek_frac_clamps() {
        const DUR: u64 = 1_000_000_000;
        assert_eq!(frac_to_ns(-0.5, DUR), 0);
        assert_eq!(frac_to_ns(0.0, DUR), 0);
        assert_eq!(frac_to_ns(0.25, DUR), 250_000_000);
        assert_eq!(frac_to_ns(1.0, DUR), DUR);
        assert_eq!(frac_to_ns(2.0, DUR), DUR);
        assert_eq!(frac_to_ns(f64::NAN, DUR), 0);
        assert_eq!(frac_to_ns(f64::INFINITY, DUR), DUR);
        assert_eq!(frac_to_ns(f64::NEG_INFINITY, DUR), 0);
        assert_eq!(frac_to_ns(0.5, 0), 0);
        assert_eq!(clamp_seek_ns(-1_000, Some(5_000)), 0);
        assert_eq!(clamp_seek_ns(9_000, Some(5_000)), 5_000);
        assert_eq!(clamp_seek_ns(100, Some(5_000)), 100);
        assert_eq!(clamp_seek_ns(-10, None), 0);
    }
}
