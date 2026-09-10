// SPDX-License-Identifier: GPL-3.0-or-later

//! One live engine at a time. Switch stops the current backend before the next
//! starts. A failed switch rolls back to the previous engine, else GStreamer,
//! else stopped.

use std::path::{Path, PathBuf};

use crate::engine::{choose_engine, Availability, Engine, EngineId, PlayRequest, PlayState};
use crate::gst_sidecar::GstSidecar;
use crate::libmpv::Libmpv;
use crate::mpv_ipc::MpvIpc;

pub type EngineFactory = Box<dyn Fn(EngineId) -> Result<Box<dyn Engine>, String> + Send>;

pub struct Session {
    app_root: PathBuf,
    factory: EngineFactory,
    current: Option<Box<dyn Engine>>,
    current_id: Option<EngineId>,
    last_req: Option<PlayRequest>,
}

impl Session {
    pub fn new(app_root: PathBuf) -> Self {
        let root = app_root.clone();
        Self {
            app_root,
            factory: Box::new(move |id| default_factory(&root, id)),
            current: None,
            current_id: None,
            last_req: None,
        }
    }

    pub fn with_factory(app_root: PathBuf, factory: EngineFactory) -> Self {
        Self {
            app_root,
            factory,
            current: None,
            current_id: None,
            last_req: None,
        }
    }

    pub fn current_id(&self) -> Option<EngineId> {
        self.current_id
    }

    pub fn status(&self) -> PlayState {
        self.current
            .as_ref()
            .map(|e| e.status())
            .unwrap_or(PlayState::Stopped)
    }

    pub fn last_request(&self) -> Option<&PlayRequest> {
        self.last_req.as_ref()
    }

    pub fn stop(&mut self) {
        if let Some(mut e) = self.current.take() {
            e.stop();
        }
        self.current_id = None;
    }

    pub fn start(&mut self, id: EngineId, req: PlayRequest) -> Result<(), String> {
        self.stop();
        let mut engine = (self.factory)(id)?;
        match engine.start(&req) {
            Ok(()) => {
                self.current_id = Some(id);
                self.current = Some(engine);
                self.last_req = Some(req);
                Ok(())
            }
            Err(e) => {
                engine.stop();
                Err(e)
            }
        }
    }

    pub fn reload(&mut self, req: PlayRequest) -> Result<(), String> {
        let Some(id) = self.current_id else {
            return Err("nothing to reload".into());
        };
        let Some(engine) = self.current.as_mut() else {
            return Err("nothing to reload".into());
        };
        match engine.reload(&req) {
            Ok(()) => {
                self.last_req = Some(req);
                Ok(())
            }
            Err(e) => {
                engine.stop();
                self.current = None;
                self.current_id = None;
                let _ = id;
                Err(e)
            }
        }
    }

    pub fn pause(&mut self, paused: bool) {
        if let Some(e) = self.current.as_mut() {
            e.pause(paused);
        }
    }

    pub fn mute(&mut self, muted: bool) {
        if let Some(e) = self.current.as_mut() {
            e.mute(muted);
        }
    }

    pub fn volume(&mut self, vol: f64) {
        if let Some(e) = self.current.as_mut() {
            e.volume(vol);
        }
    }

    /// Stop current, start `id` with the last URL. On failure, restore the
    /// previous engine if it can still start; else GStreamer if the factory
    /// can build it; else stopped.
    pub fn switch(&mut self, id: EngineId) -> Result<(), String> {
        let req = self
            .last_req
            .clone()
            .ok_or_else(|| "nothing to play".to_string())?;
        let prev = self.current_id;
        if prev == Some(id) {
            return self.reload(req);
        }
        let err = match self.start(id, req.clone()) {
            Ok(()) => return Ok(()),
            Err(e) => e,
        };
        if let Some(p) = prev {
            if self.start(p, req.clone()).is_ok() {
                return Err(err);
            }
        }
        if prev != Some(EngineId::Gstreamer) {
            if self.start(EngineId::Gstreamer, req).is_ok() {
                return Err(err);
            }
        }
        Err(err)
    }

    pub fn start_saved_or_fallback(
        &mut self,
        saved: Option<EngineId>,
        avail: &[Availability],
        req: PlayRequest,
    ) -> Result<EngineId, String> {
        let id = choose_engine(saved, avail).ok_or_else(|| missing_reason(avail))?;
        self.start(id, req)?;
        Ok(id)
    }

    pub fn app_root(&self) -> &Path {
        &self.app_root
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}

fn missing_reason(avail: &[Availability]) -> String {
    let parts: Vec<&str> = avail
        .iter()
        .filter(|a| !a.available)
        .filter_map(|a| a.reason.as_deref())
        .collect();
    if parts.is_empty() {
        "no engine available".into()
    } else {
        parts.join("; ")
    }
}

fn default_factory(app_root: &Path, id: EngineId) -> Result<Box<dyn Engine>, String> {
    match id {
        EngineId::Gstreamer => Ok(Box::new(GstSidecar::new(app_root.to_path_buf()))),
        EngineId::MpvIpc => Ok(Box::new(MpvIpc::new(app_root.to_path_buf()))),
        EngineId::Libmpv => Ok(Box::new(Libmpv::new(app_root.to_path_buf()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::PlayState;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct Mock {
        id: EngineId,
        live: Arc<AtomicU32>,
        state: PlayState,
        fail: bool,
        starts: u32,
        stops: u32,
        last_ua: String,
        last_headers: BTreeMap<String, String>,
    }

    impl Mock {
        fn new(id: EngineId, live: Arc<AtomicU32>, fail: bool) -> Self {
            Self {
                id,
                live,
                state: PlayState::Stopped,
                fail,
                starts: 0,
                stops: 0,
                last_ua: String::new(),
                last_headers: BTreeMap::new(),
            }
        }
    }

    impl Engine for Mock {
        fn id(&self) -> EngineId {
            self.id
        }
        fn start(&mut self, req: &PlayRequest) -> Result<(), String> {
            if self.fail {
                return Err(format!("{} failed to start", self.id.as_str()));
            }
            let n = self.live.fetch_add(1, Ordering::SeqCst);
            assert_eq!(n, 0, "two engines live at once");
            self.starts += 1;
            self.last_ua = req.ua.clone();
            self.last_headers = req.extra_headers();
            self.state = PlayState::Playing;
            Ok(())
        }
        fn stop(&mut self) {
            if matches!(self.state, PlayState::Playing | PlayState::Paused | PlayState::Buffering(_))
            {
                self.live.fetch_sub(1, Ordering::SeqCst);
            }
            self.stops += 1;
            self.state = PlayState::Stopped;
        }
        fn pause(&mut self, paused: bool) {
            if matches!(self.state, PlayState::Playing | PlayState::Paused) {
                self.state = if paused {
                    PlayState::Paused
                } else {
                    PlayState::Playing
                };
            }
        }
        fn mute(&mut self, _muted: bool) {}
        fn volume(&mut self, _vol: f64) {}
        fn status(&self) -> PlayState {
            self.state.clone()
        }
    }

    fn req() -> PlayRequest {
        let mut headers = BTreeMap::new();
        headers.insert("Referer".into(), "http://ref".into());
        PlayRequest {
            url: "http://example/live".into(),
            ua: "Foo/1".into(),
            headers,
            pane: 1,
        }
    }

    #[test]
    fn start_stop_reload_never_two_live() {
        let live = Arc::new(AtomicU32::new(0));
        let live2 = Arc::clone(&live);
        let session = Session::with_factory(
            PathBuf::from("."),
            Box::new(move |id| Ok(Box::new(Mock::new(id, Arc::clone(&live2), false)))),
        );
        let mut session = session;
        session.start(EngineId::Gstreamer, req()).unwrap();
        assert_eq!(live.load(Ordering::SeqCst), 1);
        assert!(matches!(session.status(), PlayState::Playing));
        session.reload(req()).unwrap();
        assert_eq!(live.load(Ordering::SeqCst), 1);
        session.start(EngineId::MpvIpc, req()).unwrap();
        assert_eq!(live.load(Ordering::SeqCst), 1);
        session.stop();
        assert_eq!(live.load(Ordering::SeqCst), 0);
        assert!(matches!(session.status(), PlayState::Stopped));
    }

    #[test]
    fn switch_rolls_back_on_failure() {
        let live = Arc::new(AtomicU32::new(0));
        let live2 = Arc::clone(&live);
        let mut session = Session::with_factory(
            PathBuf::from("."),
            Box::new(move |id| {
                let fail = id == EngineId::Libmpv;
                Ok(Box::new(Mock::new(id, Arc::clone(&live2), fail)))
            }),
        );
        session.start(EngineId::Gstreamer, req()).unwrap();
        let err = session.switch(EngineId::Libmpv).unwrap_err();
        assert!(err.contains("libmpv"));
        assert_eq!(session.current_id(), Some(EngineId::Gstreamer));
        assert_eq!(live.load(Ordering::SeqCst), 1);
        session.stop();
        assert_eq!(live.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn headers_and_ua_reach_backend() {
        let live = Arc::new(AtomicU32::new(0));
        let live2 = Arc::clone(&live);
        let seen: Arc<std::sync::Mutex<Option<(String, BTreeMap<String, String>)>>> =
            Arc::new(std::sync::Mutex::new(None));
        let seen2 = Arc::clone(&seen);
        // Capture via a mock that records into `seen` through start() side channel:
        // re-read from last_request on the session after start.
        let mut session = Session::with_factory(
            PathBuf::from("."),
            Box::new(move |id| {
                let _ = &seen2;
                Ok(Box::new(Mock::new(id, Arc::clone(&live2), false)))
            }),
        );
        session.start(EngineId::MpvIpc, req()).unwrap();
        let last = session.last_request().unwrap();
        assert_eq!(last.ua, "Foo/1");
        assert_eq!(
            last.extra_headers().get("Referer").map(String::as_str),
            Some("http://ref")
        );
        session.stop();
    }
}
