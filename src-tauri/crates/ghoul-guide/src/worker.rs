// SPDX-License-Identifier: GPL-3.0-or-later

//! XMLTV now-playing on a worker thread. The UI only reads `Snapshot`.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::fetch::open_xml_reader;
use crate::tvg::tvg_keys;
use crate::xmltv::{hhmm, parse_programmes, unix_now, Prog, WINDOW_BACK_SECS, WINDOW_FWD_SECS};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowOn {
    pub title: String,
    pub start_unix: i64,
    pub stop_unix: i64,
}

impl NowOn {
    pub fn when_line(&self) -> String {
        format!("{}–{}", hhmm(self.start_unix), hhmm(self.stop_unix))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub loading: bool,
    pub err: Option<String>,
    pub progs: HashMap<String, Vec<Prog>>,
    pub now: HashMap<String, NowOn>,
    pub next: HashMap<String, NowOn>,
}

#[derive(Clone)]
pub struct Handle {
    inner: Arc<Mutex<Snapshot>>,
    tx: mpsc::Sender<Msg>,
}

enum Msg {
    SetXml(String),
    SetWanted(Vec<String>),
    Stop,
}

impl Handle {
    pub fn snapshot(&self) -> Snapshot {
        self.inner.lock().ok().map(|g| g.clone()).unwrap_or_default()
    }

    pub fn now_for(&self, tvg_id: &str) -> Option<NowOn> {
        let g = self.inner.lock().ok()?;
        for key in tvg_keys(tvg_id) {
            if let Some(n) = g.now.get(&key) {
                return Some(n.clone());
            }
        }
        None
    }

    pub fn next_for(&self, tvg_id: &str) -> Option<NowOn> {
        let g = self.inner.lock().ok()?;
        for key in tvg_keys(tvg_id) {
            if let Some(n) = g.next.get(&key) {
                return Some(n.clone());
            }
        }
        None
    }

    pub fn progs_for(&self, tvg_id: &str) -> Vec<Prog> {
        let Ok(g) = self.inner.lock() else {
            return Vec::new();
        };
        for key in tvg_keys(tvg_id) {
            if let Some(p) = g.progs.get(&key) {
                return p.clone();
            }
        }
        Vec::new()
    }

    pub fn set_xml(&self, src: &str) {
        let _ = self.tx.send(Msg::SetXml(src.to_string()));
    }

    pub fn set_wanted(&self, ids: Vec<String>) {
        let _ = self.tx.send(Msg::SetWanted(ids));
    }

    /// Cancel an in-flight parse. Dropping the last `Handle` also disconnects
    /// the worker; this is the explicit teardown path.
    pub fn stop(&self) {
        let _ = self.tx.send(Msg::Stop);
    }
}

pub fn start() -> Handle {
    let inner = Arc::new(Mutex::new(Snapshot::default()));
    let (tx, rx) = mpsc::channel::<Msg>();
    let slot = Arc::clone(&inner);
    let _ = std::thread::Builder::new()
        .name("ghoul-epg".into())
        .spawn(move || worker(rx, slot));
    Handle { inner, tx }
}

fn worker(rx: mpsc::Receiver<Msg>, slot: Arc<Mutex<Snapshot>>) {
    let mut xml_src = String::new();
    let mut wanted: HashSet<String> = HashSet::new();
    let mut cache: HashMap<String, Vec<Prog>> = HashMap::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Msg::SetXml(s)) => {
                xml_src = s;
                reload(&xml_src, &wanted, &mut cache, &slot);
            }
            Ok(Msg::SetWanted(ids)) => {
                wanted = ids.into_iter().flat_map(|s| tvg_keys(&s)).collect();
                if !xml_src.is_empty() {
                    reload(&xml_src, &wanted, &mut cache, &slot);
                } else {
                    publish_from_cache(&cache, &slot);
                }
            }
            Ok(Msg::Stop) => return,
            Err(RecvTimeoutError::Timeout) => {
                if !cache.is_empty() {
                    publish_from_cache(&cache, &slot);
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn reload(
    src: &str,
    wanted: &HashSet<String>,
    cache: &mut HashMap<String, Vec<Prog>>,
    slot: &Arc<Mutex<Snapshot>>,
) {
    if src.trim().is_empty() || wanted.is_empty() {
        return;
    }
    if let Ok(mut g) = slot.lock() {
        g.loading = true;
        g.err = None;
    }
    let now = unix_now();
    match load_xml_windowed(src, wanted, now, |partial| {
        *cache = partial.clone();
        publish_from_cache(cache, slot);
        if let Ok(mut g) = slot.lock() {
            g.loading = true;
        }
    }) {
        Ok(map) => {
            *cache = map;
            publish_from_cache(cache, slot);
        }
        Err(_) => {
            if let Ok(mut g) = slot.lock() {
                g.loading = false;
                g.err = Some("EPG failed".into());
            }
        }
    }
}

fn publish_from_cache(cache: &HashMap<String, Vec<Prog>>, slot: &Arc<Mutex<Snapshot>>) {
    let now = unix_now();
    let mut snap = Snapshot {
        loading: false,
        err: None,
        progs: cache.clone(),
        now: HashMap::new(),
        next: HashMap::new(),
    };
    for (id, progs) in cache {
        let (n, nx) = pick_now_next(progs, now);
        if let Some(n) = n {
            snap.now.insert(id.clone(), n);
        }
        if let Some(nx) = nx {
            snap.next.insert(id.clone(), nx);
        }
    }
    if let Ok(mut g) = slot.lock() {
        *g = snap;
    }
}

pub fn pick_now_next(progs: &[Prog], now: i64) -> (Option<NowOn>, Option<NowOn>) {
    let mut now_on = None;
    let mut next = None;
    for p in progs {
        if p.start <= now && now < p.stop {
            now_on = Some(NowOn {
                title: p.title.clone(),
                start_unix: p.start,
                stop_unix: p.stop,
            });
        } else if p.start >= now {
            let cand = NowOn {
                title: p.title.clone(),
                start_unix: p.start,
                stop_unix: p.stop,
            };
            if next
                .as_ref()
                .map(|n: &NowOn| cand.start_unix < n.start_unix)
                .unwrap_or(true)
            {
                next = Some(cand);
            }
        }
    }
    (now_on, next)
}

fn load_xml_windowed(
    src: &str,
    wanted: &HashSet<String>,
    now: i64,
    tick: impl FnMut(&HashMap<String, Vec<Prog>>),
) -> Result<HashMap<String, Vec<Prog>>, String> {
    let lo = now - WINDOW_BACK_SECS;
    let hi = now + WINDOW_FWD_SECS;
    let reader = open_xml_reader(src)?;
    parse_programmes(reader, wanted, lo, hi, tick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xmltv::xmltv_to_unix;
    use std::time::Duration;

    fn wait_snap(h: &Handle, pred: impl Fn(&Snapshot) -> bool) -> Snapshot {
        for _ in 0..200 {
            let s = h.snapshot();
            if pred(&s) {
                return s;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("timeout waiting for snapshot: {:?}", h.snapshot().err);
    }

    #[test]
    fn pick_now_and_next() {
        let now = xmltv_to_unix("20240101123000 +0000").unwrap();
        let progs = vec![
            Prog {
                start: xmltv_to_unix("20240101120000 +0000").unwrap(),
                stop: xmltv_to_unix("20240101130000 +0000").unwrap(),
                title: "The News".into(),
            },
            Prog {
                start: xmltv_to_unix("20240101130000 +0000").unwrap(),
                stop: xmltv_to_unix("20240101140000 +0000").unwrap(),
                title: "After".into(),
            },
        ];
        let (n, nx) = pick_now_next(&progs, now);
        let n = n.expect("now");
        let nx = nx.expect("next");
        assert_eq!(n.title, "The News");
        assert_eq!(nx.title, "After");
        assert_eq!(n.when_line(), "12:00–13:00");
    }

    #[test]
    fn snapshot_serializes_camel_case() {
        let mut snap = Snapshot {
            loading: false,
            err: None,
            ..Snapshot::default()
        };
        snap.progs.insert(
            "cnn.us".into(),
            vec![Prog {
                start: 1,
                stop: 2,
                title: "X".into(),
            }],
        );
        let json = serde_json::to_string(&snap).unwrap();
        assert!(json.contains("\"progs\""));
        assert!(json.contains("\"start\""));
        assert!(!json.contains("startUnix"));
    }

    #[test]
    fn worker_publishes_progs_and_now() {
        let xml = r#"<?xml version="1.0"?>
<tv>
<programme start="20240101120000 +0000" stop="20990101130000 +0000" channel="CNN.us">
<title>Live Window</title>
</programme>
</tv>"#;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("iptv.xml");
        std::fs::write(&path, xml).unwrap();
        let h = start();
        h.set_wanted(vec!["CNN.us".into()]);
        h.set_xml(path.to_str().unwrap());
        let snap = wait_snap(&h, |s| !s.loading && s.progs.contains_key("cnn.us"));
        assert!(snap.err.is_none());
        assert_eq!(snap.progs["cnn.us"][0].title, "Live Window");
        assert_eq!(snap.now["cnn.us"].title, "Live Window");
        h.stop();
    }

    #[test]
    fn worker_stop_does_not_panic() {
        let h = start();
        h.stop();
        std::thread::sleep(Duration::from_millis(30));
    }
}
