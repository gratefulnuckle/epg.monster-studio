// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::models::{ManagedChannel, StreamVariant};
use crate::paths::{audit_process_db_path, offline_slates_directory};
use crate::settings::AppSettings;
use crate::store::{SqliteStore, StoreError};

pub const HASH_SIZE: usize = 16;
pub const HASH_BYTES: usize = HASH_SIZE * HASH_SIZE;
pub const DIGEST_BYTES: usize = HASH_BYTES / 8;
pub const MATCH_DISTANCE: i32 = 24;

pub fn classify_error(error: &str) -> String {
    if error.trim().is_empty() {
        return String::new();
    }
    let e = error.to_ascii_lowercase();
    if e.contains("offline slate") {
        return "offline-slate".into();
    }
    if e.contains("404") {
        return "http-404".into();
    }
    if e.contains("403") {
        return "http-403".into();
    }
    if e.contains("401") {
        return "http-401".into();
    }
    if e.contains("-138") {
        return "connect-timeout".into();
    }
    if e.contains("i/o error") || e.contains("io error") {
        return "io-error".into();
    }
    if e.trim() == "timeout" || e.contains("timed out") {
        return "probe-timeout".into();
    }
    if e.contains("-1145393733") || e.contains("terminating thread") {
        return "decode-abort".into();
    }
    if e.contains("canceled") || e.contains("cancelled") {
        return "cancelled".into();
    }
    if e.contains("invalid data") {
        return "invalid-data".into();
    }
    if e.contains("end of file") || e.contains("eof") {
        return "eof".into();
    }
    if e.contains("server returned") {
        return "http-other".into();
    }
    "other".into()
}

pub fn display_name(cls: &str) -> &'static str {
    match cls {
        "offline-slate" => "Offline slate",
        "http-404" => "HTTP 404",
        "http-403" => "HTTP 403",
        "http-401" => "HTTP 401",
        "http-other" => "HTTP error",
        "connect-timeout" => "Connect timeout",
        "io-error" => "I/O error",
        "probe-timeout" => "Probe timeout",
        "decode-abort" => "Decode abort",
        "cancelled" => "Cancelled",
        "invalid-data" => "Invalid data",
        "eof" => "EOF",
        "" => "OK",
        _ => "Other",
    }
}

pub fn format_hms(seconds: i64) -> String {
    let sec = seconds.max(0);
    let h = sec / 3600;
    let m = (sec % 3600) / 60;
    let s = sec % 60;
    format!("{h}h {m}m {s}s")
}

pub fn compare_estimate(elapsed_secs: i64, first_eta_secs: i64) -> String {
    let elapsed = elapsed_secs.max(0);
    let first = first_eta_secs.max(0);
    let faster = first >= elapsed;
    let abs = if faster { first - elapsed } else { elapsed - first };
    format!(
        "Elapsed {}  ·  first estimate {}  ·  {} {}",
        format_hms(elapsed),
        format_hms(first),
        format_hms(abs),
        if faster { "faster" } else { "slower" }
    )
}

pub fn hash_gray_16x16(gray: &[u8]) -> Result<Vec<u8>, String> {
    if gray.len() != HASH_BYTES {
        return Err(format!("Expected {HASH_BYTES} gray pixels."));
    }
    let sum: u32 = gray.iter().map(|b| *b as u32).sum();
    let mean = sum as f64 / gray.len() as f64;
    let mut bits = vec![0u8; DIGEST_BYTES];
    for (i, px) in gray.iter().enumerate() {
        if (*px as f64) > mean {
            bits[i / 8] |= 1 << (7 - (i % 8));
        }
    }
    Ok(bits)
}

pub fn hamming(a: &[u8], b: &[u8]) -> i32 {
    let n = a.len().min(b.len());
    let mut d = 0i32;
    for i in 0..n {
        d += (a[i] ^ b[i]).count_ones() as i32;
    }
    d += 8 * (a.len() as i32 - b.len() as i32).abs();
    d
}

pub fn black_total_seconds(stderr: &str) -> f64 {
    let mut sum = 0.0;
    for part in stderr.split("black_duration:") {
        if part.len() == stderr.len() {
            continue;
        }
        let num: String = part
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if let Ok(v) = num.parse::<f64>() {
            sum += v;
        }
    }
    sum
}

pub fn is_mostly_black(stderr: &str, sample_seconds: f64, ratio: f64) -> bool {
    sample_seconds > 0.0 && black_total_seconds(stderr) >= sample_seconds * ratio
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditJob {
    pub id: String,
    pub state: String,
    pub scope: String,
    pub auto_swap: bool,
    pub total: i32,
    pub current_index: i32,
    pub ok_count: i32,
    pub fail_count: i32,
    pub started_at: String,
    pub updated_at: String,
    pub finished_at: Option<String>,
    pub pid: u32,
    pub grades_json: String,
    pub first_eta_seconds: i32,
    pub elapsed_ms: i64,
}

impl AuditJob {
    pub fn has_remaining(&self) -> bool {
        matches!(self.state.as_str(), "running" | "paused") && self.current_index < self.total
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQueueItem {
    pub seq: i32,
    pub variant_id: String,
    pub channel_id: String,
    pub channel_name: String,
    pub group_title: String,
    pub visibility: String,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditFeedRow {
    pub seq: i32,
    pub is_header: bool,
    pub title: String,
    pub subtitle: String,
    pub detail: String,
    pub grade: String,
    pub status_label: String,
    pub latency_label: String,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuditResult {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub ok: bool,
    pub error: Option<String>,
    pub latency_ms: Option<i32>,
    pub engine: String,
    pub probed_at: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub fps: Option<f64>,
    pub aspect_ratio: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub grade: String,
    pub job_id: Option<String>,
    pub channel_id: Option<String>,
    pub channel_name: Option<String>,
    pub group_title: Option<String>,
    pub tvg_id: Option<String>,
    pub error_class: Option<String>,
}

impl AuditResult {
    pub fn detail_line(&self) -> String {
        let mut parts = Vec::new();
        if let (Some(w), Some(h)) = (self.width, self.height) {
            if w > 0 && h > 0 {
                parts.push(format!("{w}×{h}"));
            }
        }
        if let Some(ar) = &self.aspect_ratio {
            if !ar.is_empty() {
                parts.push(ar.clone());
            }
        }
        if let Some(fps) = self.fps {
            if fps > 0.0 {
                parts.push(format!("{fps} fps"));
            }
        }
        if let Some(v) = &self.video_codec {
            if !v.is_empty() {
                parts.push(v.clone());
            }
        }
        if let Some(a) = &self.audio_codec {
            if !a.is_empty() {
                parts.push(a.clone());
            }
        }
        if let Some(ms) = self.latency_ms {
            parts.push(format!("{ms} ms"));
        }
        if parts.is_empty() {
            self.error.clone().unwrap_or_else(|| "No stream details".into())
        } else {
            parts.join(" · ")
        }
    }
}

pub fn finalize_grade(mut r: AuditResult) -> AuditResult {
    if !r.ok {
        r.grade = "F".into();
        return r;
    }
    let latency = r.latency_ms.unwrap_or(99999);
    let height = r.height.unwrap_or(0);
    let fps = r.fps.unwrap_or(0.0);
    let has_video = height > 0 || r.width.unwrap_or(0) > 0;
    if !has_video {
        r.grade = if latency <= 5000 { "B" } else { "C" }.into();
        return r;
    }
    let mut score = 0;
    score += if height >= 1080 {
        40
    } else if height >= 720 {
        30
    } else if height >= 480 {
        18
    } else {
        8
    };
    score += if fps >= 50.0 {
        25
    } else if fps >= 24.0 {
        20
    } else if fps > 0.0 {
        10
    } else {
        5
    };
    score += if latency <= 3500 {
        25
    } else if latency <= 6000 {
        18
    } else if latency <= 10000 {
        10
    } else {
        4
    };
    if r.video_codec.as_deref().is_some_and(|s| !s.is_empty()) {
        score += 5;
    }
    if r.audio_codec.as_deref().is_some_and(|s| !s.is_empty()) {
        score += 5;
    }
    r.grade = match score {
        85.. => "A",
        70.. => "B",
        55.. => "C",
        _ => "D",
    }
    .into();
    r
}

pub fn parse_grades(json: &str) -> HashMap<String, i32> {
    serde_json::from_str(json).unwrap_or_default()
}

pub fn serialize_grades(grades: &HashMap<String, i32>) -> String {
    serde_json::to_string(grades).unwrap_or_else(|_| "{}".into())
}

pub struct ProcessStore {
    conn: Connection,
    pub path: PathBuf,
}

impl ProcessStore {
    pub fn open(path: Option<&Path>) -> Result<Self, StoreError> {
        let path = path
            .map(Path::to_path_buf)
            .unwrap_or_else(audit_process_db_path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(&path)?;
        let store = Self { conn, path };
        store.init()?;
        Ok(store)
    }

    fn init(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS job (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL,
                scope TEXT NOT NULL,
                auto_swap INTEGER NOT NULL,
                total INTEGER NOT NULL,
                current_index INTEGER NOT NULL,
                ok_count INTEGER NOT NULL,
                fail_count INTEGER NOT NULL,
                started_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                finished_at TEXT NULL,
                pid INTEGER NOT NULL,
                grades_json TEXT NOT NULL DEFAULT '{}'
            );
            CREATE TABLE IF NOT EXISTS queue (
                seq INTEGER PRIMARY KEY,
                variant_id TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                channel_name TEXT NOT NULL,
                group_title TEXT NOT NULL,
                visibility TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS feed (
                seq INTEGER PRIMARY KEY,
                is_header INTEGER NOT NULL,
                title TEXT NOT NULL,
                subtitle TEXT NOT NULL,
                detail TEXT NOT NULL,
                grade TEXT NOT NULL,
                status_label TEXT NOT NULL,
                latency_label TEXT NOT NULL,
                ok INTEGER NOT NULL
            );
            "#,
        )?;
        let _ = self
            .conn
            .execute("ALTER TABLE job ADD COLUMN first_eta_seconds INTEGER NOT NULL DEFAULT 0", []);
        let _ = self
            .conn
            .execute("ALTER TABLE job ADD COLUMN elapsed_ms INTEGER NOT NULL DEFAULT 0", []);
        Ok(())
    }

    pub fn load_job(&self) -> Result<Option<AuditJob>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, state, scope, auto_swap, total, current_index, ok_count, fail_count,
                    started_at, updated_at, finished_at, pid, grades_json,
                    first_eta_seconds, elapsed_ms
             FROM job ORDER BY updated_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query([])?;
        let Some(r) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(AuditJob {
            id: r.get(0)?,
            state: r.get(1)?,
            scope: r.get(2)?,
            auto_swap: r.get::<_, i64>(3)? != 0,
            total: r.get::<_, i64>(4)? as i32,
            current_index: r.get::<_, i64>(5)? as i32,
            ok_count: r.get::<_, i64>(6)? as i32,
            fail_count: r.get::<_, i64>(7)? as i32,
            started_at: r.get(8)?,
            updated_at: r.get(9)?,
            finished_at: r.get(10)?,
            pid: r.get::<_, i64>(11)? as u32,
            grades_json: r.get::<_, Option<String>>(12)?.unwrap_or_else(|| "{}".into()),
            first_eta_seconds: r.get::<_, i64>(13).unwrap_or(0) as i32,
            elapsed_ms: r.get::<_, i64>(14).unwrap_or(0),
        }))
    }

    pub fn replace_job(&self, job: &AuditJob, queue: &[AuditQueueItem]) -> Result<(), StoreError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch("DELETE FROM job; DELETE FROM queue; DELETE FROM feed;")?;
        tx.execute(
            "INSERT INTO job(id,state,scope,auto_swap,total,current_index,ok_count,fail_count,
                             started_at,updated_at,finished_at,pid,grades_json,first_eta_seconds,elapsed_ms)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                job.id,
                job.state,
                job.scope,
                job.auto_swap as i32,
                job.total,
                job.current_index,
                job.ok_count,
                job.fail_count,
                job.started_at,
                job.updated_at,
                job.finished_at,
                job.pid as i64,
                job.grades_json,
                job.first_eta_seconds,
                job.elapsed_ms
            ],
        )?;
        for q in queue {
            tx.execute(
                "INSERT INTO queue(seq,variant_id,channel_id,channel_name,group_title,visibility,done)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    q.seq,
                    q.variant_id,
                    q.channel_id,
                    q.channel_name,
                    q.group_title,
                    q.visibility,
                    q.done as i32
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn update_job(&self, job: &AuditJob) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE job SET state=?1, scope=?2, auto_swap=?3, total=?4,
                 current_index=?5, ok_count=?6, fail_count=?7, started_at=?8,
                 updated_at=?9, finished_at=?10, pid=?11, grades_json=?12,
                 first_eta_seconds=?13, elapsed_ms=?14
             WHERE id=?15",
            params![
                job.state,
                job.scope,
                job.auto_swap as i32,
                job.total,
                job.current_index,
                job.ok_count,
                job.fail_count,
                job.started_at,
                now_iso(),
                job.finished_at,
                job.pid as i64,
                job.grades_json,
                job.first_eta_seconds,
                job.elapsed_ms,
                job.id
            ],
        )?;
        Ok(())
    }

    pub fn mark_queue_done(&self, seq: i32) -> Result<(), StoreError> {
        self.conn
            .execute("UPDATE queue SET done=1 WHERE seq=?1", params![seq])?;
        Ok(())
    }

    pub fn load_queue(&self) -> Result<Vec<AuditQueueItem>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, variant_id, channel_id, channel_name, group_title, visibility, done
             FROM queue ORDER BY seq",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AuditQueueItem {
                seq: r.get::<_, i64>(0)? as i32,
                variant_id: r.get(1)?,
                channel_id: r.get(2)?,
                channel_name: r.get(3)?,
                group_title: r.get(4)?,
                visibility: r.get(5)?,
                done: r.get::<_, i64>(6)? != 0,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn append_feed(&self, row: &AuditFeedRow) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO feed(seq,is_header,title,subtitle,detail,grade,status_label,latency_label,ok)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                row.seq,
                row.is_header as i32,
                row.title,
                row.subtitle,
                row.detail,
                row.grade,
                row.status_label,
                row.latency_label,
                row.ok as i32
            ],
        )?;
        Ok(())
    }

    pub fn load_feed(&self) -> Result<Vec<AuditFeedRow>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq,is_header,title,subtitle,detail,grade,status_label,latency_label,ok
             FROM feed ORDER BY seq",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(AuditFeedRow {
                seq: r.get::<_, i64>(0)? as i32,
                is_header: r.get::<_, i64>(1)? != 0,
                title: r.get(2)?,
                subtitle: r.get(3)?,
                detail: r.get(4)?,
                grade: r.get(5)?,
                status_label: r.get(6)?,
                latency_label: r.get(7)?,
                ok: r.get::<_, i64>(8)? != 0,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn clear(&self) -> Result<(), StoreError> {
        self.conn
            .execute_batch("DELETE FROM job; DELETE FROM queue; DELETE FROM feed;")?;
        Ok(())
    }
}

pub fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

pub const WEEK_DAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

pub fn today_name() -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    now.weekday().to_string()
}

pub fn day_key() -> String {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let now = time::OffsetDateTime::now_utc().to_offset(offset);
    format!("{:04}-{:02}-{:02}", now.year(), now.month() as u8, now.day())
}

pub fn already_ran_today(last_run: &str) -> bool {
    last_run.trim().eq_ignore_ascii_case(&day_key())
}

pub fn parse_weekly(json: &str) -> HashMap<String, Vec<String>> {
    let mut plan = HashMap::new();
    for d in WEEK_DAYS {
        plan.insert(d.to_string(), Vec::new());
    }
    if json.trim().is_empty() {
        return plan;
    }
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(json) else {
        return plan;
    };
    let Some(obj) = doc.as_object() else {
        return plan;
    };
    for (k, v) in obj {
        let Some(arr) = v.as_array() else { continue };
        let mut list = Vec::new();
        for el in arr {
            if let Some(s) = el.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                if !list.iter().any(|x: &String| x.eq_ignore_ascii_case(s)) {
                    list.push(s.to_string());
                }
            }
        }
        plan.insert(k.clone(), list);
    }
    plan
}

pub fn serialize_weekly(plan: &HashMap<String, Vec<String>>) -> String {
    let mut ordered = serde_json::Map::new();
    for d in WEEK_DAYS {
        let list = groups_for(plan, d);
        ordered.insert(d.to_string(), serde_json::json!(list));
    }
    serde_json::to_string(&ordered).unwrap_or_else(|_| "{}".into())
}

pub fn groups_for<'a>(plan: &'a HashMap<String, Vec<String>>, day: &str) -> Vec<String> {
    plan.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(day))
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSnapshot {
    pub job: Option<AuditJob>,
    pub feed: Vec<AuditFeedRow>,
    pub queue: Vec<AuditQueueItem>,
    pub grade_counts: HashMap<String, i32>,
    pub interrupted_on_launch: bool,
}

pub fn snapshot(process: &ProcessStore) -> Result<AuditSnapshot, StoreError> {
    let job = process.load_job()?;
    let mut interrupted = false;
    let mut job = job;
    if let Some(j) = job.as_mut() {
        if j.state == "running" && j.pid != std::process::id() && j.has_remaining() {
            j.state = "paused".into();
            process.update_job(j)?;
            interrupted = true;
        }
    }
    let grades = job
        .as_ref()
        .map(|j| parse_grades(&j.grades_json))
        .unwrap_or_default();
    Ok(AuditSnapshot {
        job,
        feed: process.load_feed()?,
        queue: process.load_queue()?,
        grade_counts: grades,
        interrupted_on_launch: interrupted,
    })
}

pub fn begin_job(
    store: &SqliteStore,
    process: &ProcessStore,
    settings: &AppSettings,
    auto_swap: bool,
    visible_only: bool,
    channel_ids: Option<&[String]>,
) -> Result<AuditJob, StoreError> {
    let channels = store.list_managed(None)?;
    let by_id: HashMap<String, ManagedChannel> =
        channels.iter().cloned().map(|c| (c.id.clone(), c)).collect();
    let mut variants = store.list_all_variants()?;
    if let Some(ids) = channel_ids {
        let set: std::collections::HashSet<_> = ids.iter().cloned().collect();
        variants.retain(|v| set.contains(&v.managed_channel_id));
    }
    if visible_only {
        variants.retain(|v| v.visibility.eq_ignore_ascii_case("visible"));
    }
    variants.sort_by(|a, b| {
        let ga = by_id
            .get(&a.managed_channel_id)
            .map(|c| c.group_title.to_ascii_lowercase())
            .unwrap_or_else(|| "\u{ffff}".into());
        let gb = by_id
            .get(&b.managed_channel_id)
            .map(|c| c.group_title.to_ascii_lowercase())
            .unwrap_or_else(|| "\u{ffff}".into());
        ga.cmp(&gb)
            .then_with(|| {
                let na = by_id
                    .get(&a.managed_channel_id)
                    .map(|c| c.name.to_ascii_lowercase())
                    .unwrap_or_default();
                let nb = by_id
                    .get(&b.managed_channel_id)
                    .map(|c| c.name.to_ascii_lowercase())
                    .unwrap_or_default();
                na.cmp(&nb)
            })
            .then(a.priority.cmp(&b.priority))
    });
    let mut queue = Vec::new();
    for (n, v) in variants.iter().enumerate() {
        let ch = by_id.get(&v.managed_channel_id);
        queue.push(AuditQueueItem {
            seq: n as i32,
            variant_id: v.id.clone(),
            channel_id: v.managed_channel_id.clone(),
            channel_name: ch
                .map(|c| c.name.clone())
                .filter(|s| !s.is_empty())
                .or_else(|| v.label.clone())
                .unwrap_or_else(|| "Unknown".into()),
            group_title: ch
                .map(|c| c.group_title.clone())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Ungrouped".into()),
            visibility: v.visibility.clone(),
            done: false,
        });
    }
    let delay = settings.audit_delay_ms.max(0) as i64;
    let job = AuditJob {
        id: uuid::Uuid::new_v4().simple().to_string(),
        state: "running".into(),
        scope: if visible_only { "visible" } else { "all" }.into(),
        auto_swap,
        total: queue.len() as i32,
        current_index: 0,
        ok_count: 0,
        fail_count: 0,
        started_at: now_iso(),
        updated_at: now_iso(),
        finished_at: None,
        pid: std::process::id(),
        grades_json: "{}".into(),
        first_eta_seconds: ((delay + 3500) * queue.len().max(1) as i64 / 1000).max(1) as i32,
        elapsed_ms: 0,
    };
    process.replace_job(&job, &queue)?;
    Ok(job)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditStep {
    pub job: AuditJob,
    pub feed: Vec<AuditFeedRow>,
    pub done: bool,
}

pub fn next_step(
    store: &SqliteStore,
    process: &ProcessStore,
    settings: &AppSettings,
) -> Result<AuditStep, StoreError> {
    let mut job = process
        .load_job()?
        .ok_or_else(|| StoreError::Io(std::io::Error::other("No saved Stream Audit")))?;
    if job.state != "running" {
        return Err(StoreError::Io(std::io::Error::other(
            if job.has_remaining() {
                "Stream Audit is paused"
            } else {
                "Nothing to resume."
            },
        )));
    }
    let queue = process.load_queue()?;
    let Some(item) = queue.iter().find(|q| !q.done).cloned() else {
        job.state = "completed".into();
        job.finished_at = Some(now_iso());
        job.current_index = job.total;
        process.update_job(&job)?;
        return Ok(AuditStep {
            job,
            feed: Vec::new(),
            done: true,
        });
    };

    let ffmpeg = settings.ffmpeg_path.clone();
    let ffprobe = settings.ffprobe_path.clone();
    ensure_slate_templates(&ffmpeg);
    let timeout = settings.audit_timeout_ms.max(1000);
    let delay = settings.audit_delay_ms.max(0) as u64;
    let mut added = Vec::new();
    let feed = process.load_feed()?;
    let last_group = feed.iter().rev().find(|f| f.is_header).map(|f| f.title.clone());
    let mut feed_seq = feed.iter().map(|f| f.seq).max().unwrap_or(-1) + 1;

    if last_group
        .as_deref()
        .map(|g| !g.eq_ignore_ascii_case(&item.group_title))
        .unwrap_or(true)
    {
        let row = AuditFeedRow {
            seq: feed_seq,
            is_header: true,
            title: item.group_title.clone(),
            subtitle: "starting group".into(),
            detail: String::new(),
            grade: String::new(),
            status_label: String::new(),
            latency_label: String::new(),
            ok: false,
        };
        feed_seq += 1;
        process.append_feed(&row)?;
        added.push(row);
    }

    let Some(variant) = store.get_variant(&item.variant_id)? else {
        process.mark_queue_done(item.seq)?;
        job.current_index = item.seq + 1;
        process.update_job(&job)?;
        return Ok(AuditStep {
            job,
            feed: added,
            done: false,
        });
    };

    let stream_role = if variant.visibility.eq_ignore_ascii_case("visible") {
        "primary"
    } else {
        "backup"
    };
    let ch = store.get_managed(&item.channel_id)?;
    let mut result = probe_url(&ffmpeg, &ffprobe, &variant.url, timeout);
    result = apply_offline_slate(result, &ffmpeg, &variant.url, timeout);
    if settings.black_detect_enabled {
        result = apply_black_detect(result, &ffmpeg, &variant.url, timeout);
    }
    result.target_type = "variant".into();
    result.target_id = variant.id.clone();
    result.job_id = Some(job.id.clone());
    result.channel_id = Some(item.channel_id.clone());
    result.channel_name = Some(item.channel_name.clone());
    result.group_title = Some(item.group_title.clone());
    result.tvg_id = ch.as_ref().and_then(|c| c.tvg_id.clone());
    result.error_class = Some(if result.ok {
        String::new()
    } else {
        classify_error(result.error.as_deref().unwrap_or(""))
    });
    store.insert_audit_result(&result)?;
    store.update_variant_audit(&variant.id, result.ok, &result.probed_at)?;

    if result.ok {
        job.ok_count += 1;
    } else {
        job.fail_count += 1;
    }
    let mut grades = parse_grades(&job.grades_json);
    *grades.entry(result.grade.clone()).or_insert(0) += 1;
    job.grades_json = serialize_grades(&grades);

    let card = feed_from_result(&item.channel_name, &item.group_title, stream_role, &result, feed_seq);
    feed_seq += 1;
    process.append_feed(&card)?;
    added.push(card);

    if !result.ok
        && job.auto_swap
        && settings.auto_swap_on_audit_fail
        && variant.visibility.eq_ignore_ascii_case("visible")
    {
        if let Some(full) = store.get_managed(&variant.managed_channel_id)? {
            let mut backups: Vec<StreamVariant> = full
                .variants
                .into_iter()
                .filter(|v| v.id != variant.id)
                .collect();
            backups.sort_by_key(|v| v.priority);
            for backup in backups {
                if delay > 0 {
                    std::thread::sleep(Duration::from_millis(delay));
                }
                let mut br = probe_url(&ffmpeg, &ffprobe, &backup.url, timeout);
                br = apply_offline_slate(br, &ffmpeg, &backup.url, timeout);
                if settings.black_detect_enabled {
                    br = apply_black_detect(br, &ffmpeg, &backup.url, timeout);
                }
                br.target_type = "variant".into();
                br.target_id = backup.id.clone();
                br.job_id = Some(job.id.clone());
                br.channel_id = Some(item.channel_id.clone());
                br.channel_name = Some(item.channel_name.clone());
                br.group_title = Some(item.group_title.clone());
                br.tvg_id = full.tvg_id.clone();
                br.error_class = Some(if br.ok {
                    String::new()
                } else {
                    classify_error(br.error.as_deref().unwrap_or(""))
                });
                store.insert_audit_result(&br)?;
                store.update_variant_audit(&backup.id, br.ok, &br.probed_at)?;
                if br.ok {
                    job.ok_count += 1;
                } else {
                    job.fail_count += 1;
                }
                *grades.entry(br.grade.clone()).or_insert(0) += 1;
                job.grades_json = serialize_grades(&grades);
                let role = if br.ok { "backup-swap" } else { "backup" };
                if br.ok {
                    store.swap_visible(&full.id, &variant.id, &backup.id, "auto_audit")?;
                }
                let row = feed_from_result(&item.channel_name, &item.group_title, role, &br, feed_seq);
                feed_seq += 1;
                process.append_feed(&row)?;
                added.push(row);
                if br.ok {
                    break;
                }
            }
        }
    }

    process.mark_queue_done(item.seq)?;
    job.current_index = item.seq + 1;
    if job.current_index >= job.total {
        job.state = "completed".into();
        job.finished_at = Some(now_iso());
    }
    process.update_job(&job)?;
    Ok(AuditStep {
        done: job.state == "completed",
        job,
        feed: added,
    })
}

fn feed_from_result(
    channel_name: &str,
    group_title: &str,
    stream_role: &str,
    r: &AuditResult,
    seq: i32,
) -> AuditFeedRow {
    let role = match stream_role {
        "backup" => "backup stream",
        "backup-swap" => "auto-swapped backup",
        _ => "primary",
    };
    AuditFeedRow {
        seq,
        is_header: false,
        title: channel_name.into(),
        subtitle: format!("{group_title} · {role}"),
        detail: if r.ok {
            r.detail_line()
        } else {
            r.error.clone().unwrap_or_else(|| "Failed".into())
        },
        grade: r.grade.clone(),
        status_label: if r.ok { "OK" } else { "FAIL" }.into(),
        latency_label: r
            .latency_ms
            .map(|ms| format!("{ms} ms"))
            .unwrap_or_default(),
        ok: r.ok,
    }
}

pub fn probe_url(ffmpeg: &str, ffprobe: &str, url: &str, timeout_ms: i32) -> AuditResult {
    let start = Instant::now();
    if ffmpeg.is_empty() || !Path::new(ffmpeg).is_file() {
        return finalize_grade(AuditResult {
            id: uuid::Uuid::new_v4().simple().to_string(),
            ok: false,
            error: Some(format!("ffmpeg not found: {ffmpeg}")),
            latency_ms: Some(0),
            engine: "ffmpeg".into(),
            probed_at: now_iso(),
            target_type: "variant".into(),
            grade: "F".into(),
            ..AuditResult::default()
        });
    }
    let timeout = Duration::from_millis((timeout_ms.max(1000) + 2000) as u64);
    let rw = (timeout_ms as i64) * 1000;
    let (ok, stderr, killed) = run_tool(
        ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-rw_timeout",
            &rw.to_string(),
            "-i",
            url,
            "-t",
            "2",
            "-f",
            "null",
            "-",
        ],
        timeout,
    );
    let latency = start.elapsed().as_millis() as i32;
    if killed {
        return finalize_grade(AuditResult {
            id: uuid::Uuid::new_v4().simple().to_string(),
            ok: false,
            error: Some("Timeout".into()),
            latency_ms: Some(latency),
            engine: "ffmpeg".into(),
            probed_at: now_iso(),
            target_type: "variant".into(),
            grade: "F".into(),
            ..AuditResult::default()
        });
    }
    let error = if ok {
        None
    } else {
        let last = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "exit 1".into());
        Some(last)
    };
    let mut result = AuditResult {
        id: uuid::Uuid::new_v4().simple().to_string(),
        ok,
        error,
        latency_ms: Some(latency),
        engine: "ffmpeg".into(),
        probed_at: now_iso(),
        target_type: "variant".into(),
        grade: String::new(),
        ..AuditResult::default()
    };
    if ok && Path::new(ffprobe).is_file() {
        enrich_ffprobe(&mut result, ffprobe, url, timeout_ms.min(12000));
    }
    finalize_grade(result)
}

fn apply_offline_slate(mut result: AuditResult, ffmpeg: &str, url: &str, timeout_ms: i32) -> AuditResult {
    if !result.ok {
        return result;
    }
    let Some(hash) = hash_live_frame(ffmpeg, url, timeout_ms.min(8000)) else {
        return result;
    };
    if let Some(dist) = match_slate(&hash) {
        result.ok = false;
        result.error = Some(format!("Offline slate (match distance {dist})"));
        result.engine = "ffmpeg+slate".into();
        return finalize_grade(result);
    }
    result
}

fn apply_black_detect(mut result: AuditResult, ffmpeg: &str, url: &str, timeout_ms: i32) -> AuditResult {
    if !result.ok {
        return result;
    }
    let secs = 5i32;
    let timeout = Duration::from_millis((timeout_ms.min(12000).max(secs * 1000 + 3000)) as u64);
    let clean = url.replace('"', "");
    let t = secs.to_string();
    let (_, stderr, _) = run_tool(
        ffmpeg,
        &[
            "-hide_banner",
            "-nostdin",
            "-t",
            &t,
            "-i",
            &clean,
            "-vf",
            "blackdetect=d=1.5:pix_th=0.12",
            "-an",
            "-f",
            "null",
            "-",
        ],
        timeout,
    );
    if is_mostly_black(&stderr, secs as f64, 0.7) {
        result.ok = false;
        result.error = Some("Black screen (ffmpeg blackdetect)".into());
        result.engine = "ffmpeg+blackdetect".into();
        return finalize_grade(result);
    }
    result
}

fn hash_live_frame(ffmpeg: &str, url: &str, timeout_ms: i32) -> Option<Vec<u8>> {
    if !Path::new(ffmpeg).is_file() || url.trim().is_empty() {
        return None;
    }
    let tmp = std::env::temp_dir().join(format!(
        "epg-monster-slate-{}.raw",
        uuid::Uuid::new_v4().simple()
    ));
    let rw = timeout_ms.max(1) as i64 * 1000;
    let vf = format!("crop=iw/2:ih/2,scale={HASH_SIZE}:{HASH_SIZE}:flags=fast_bilinear,format=gray");
    let timeout = Duration::from_millis((timeout_ms + 2500) as u64);
    let tmp_s = tmp.to_string_lossy().into_owned();
    let (ok, _, _) = run_tool(
        ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-rw_timeout",
            &rw.to_string(),
            "-i",
            url,
            "-an",
            "-frames:v",
            "1",
            "-vf",
            &vf,
            "-f",
            "rawvideo",
            "-y",
            &tmp_s,
        ],
        timeout,
    );
    let raw = if ok { std::fs::read(&tmp).ok() } else { None };
    let _ = std::fs::remove_file(&tmp);
    let raw = raw?;
    if raw.len() != HASH_BYTES {
        return None;
    }
    hash_gray_16x16(&raw).ok()
}

fn match_slate(digest: &[u8]) -> Option<i32> {
    let templates = SLATE_TEMPLATES.lock().ok()?;
    matches_hash(digest, &templates)
}

static SLATE_TEMPLATES: std::sync::Mutex<Vec<Vec<u8>>> = std::sync::Mutex::new(Vec::new());
static SLATE_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn ensure_slate_templates(ffmpeg: &str) {
    if SLATE_READY.load(std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let mut hashes = Vec::new();
    let dir = offline_slates_directory();
    let builtin = dir.join("offline-slate.png");
    if !builtin.is_file() || builtin.metadata().map(|m| m.len() == 0).unwrap_or(true) {
        let bytes = include_bytes!("../resources/offline-slate.png");
        let _ = std::fs::write(&builtin, bytes);
    }
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for ent in rd.flatten() {
            let p = ent.path();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
                if let Some(h) = hash_image_file(ffmpeg, &p) {
                    hashes.push(h);
                }
            }
        }
    }
    if let Ok(mut g) = SLATE_TEMPLATES.lock() {
        *g = hashes;
    }
    SLATE_READY.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn hash_image_file(ffmpeg: &str, image: &Path) -> Option<Vec<u8>> {
    if !Path::new(ffmpeg).is_file() || !image.is_file() {
        return None;
    }
    let tmp = std::env::temp_dir().join(format!(
        "epg-monster-slate-t-{}.raw",
        uuid::Uuid::new_v4().simple()
    ));
    let vf = format!("crop=iw/2:ih/2,scale={HASH_SIZE}:{HASH_SIZE}:flags=fast_bilinear,format=gray");
    let tmp_s = tmp.to_string_lossy().into_owned();
    let img = image.to_string_lossy().into_owned();
    let (ok, _, _) = run_tool(
        ffmpeg,
        &[
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &img,
            "-an",
            "-frames:v",
            "1",
            "-vf",
            &vf,
            "-f",
            "rawvideo",
            "-y",
            &tmp_s,
        ],
        Duration::from_millis(15000),
    );
    let raw = if ok { std::fs::read(&tmp).ok() } else { None };
    let _ = std::fs::remove_file(&tmp);
    let raw = raw?;
    if raw.len() != HASH_BYTES {
        return None;
    }
    hash_gray_16x16(&raw).ok()
}

/// Hash an already-decoded 16×16 gray file against templates stored as 32-byte digests
/// in memory is done by callers; this helper matches two raw hashes.
pub fn matches_hash(digest: &[u8], templates: &[Vec<u8>]) -> Option<i32> {
    if digest.len() != DIGEST_BYTES || templates.is_empty() {
        return None;
    }
    let mut best = i32::MAX;
    for t in templates {
        let d = hamming(digest, t);
        if d < best {
            best = d;
        }
    }
    if best <= MATCH_DISTANCE {
        Some(best)
    } else {
        None
    }
}

fn enrich_ffprobe(result: &mut AuditResult, ffprobe: &str, url: &str, timeout_ms: i32) {
    let rw = timeout_ms as i64 * 1000;
    let timeout = Duration::from_millis((timeout_ms + 1500) as u64);
    let (ok, stdout, _) = run_tool_stdout(
        ffprobe,
        &[
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            "-probesize",
            "2M",
            "-analyzeduration",
            "2M",
            "-rw_timeout",
            &rw.to_string(),
            "-i",
            url,
        ],
        timeout,
    );
    if !ok || stdout.trim().is_empty() {
        return;
    }
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(&stdout) else {
        return;
    };
    let Some(streams) = doc.get("streams").and_then(|s| s.as_array()) else {
        return;
    };
    for s in streams {
        let codec_type = s.get("codec_type").and_then(|v| v.as_str()).unwrap_or("");
        let codec = s.get("codec_name").and_then(|v| v.as_str()).map(|x| x.to_string());
        if codec_type.eq_ignore_ascii_case("video") {
            result.video_codec = codec;
            result.width = s.get("width").and_then(|v| v.as_i64()).map(|n| n as i32);
            result.height = s.get("height").and_then(|v| v.as_i64()).map(|n| n as i32);
            if let Some(dar) = s.get("display_aspect_ratio").and_then(|v| v.as_str()) {
                if !dar.is_empty() && dar != "0:1" && dar != "N/A" {
                    result.aspect_ratio = Some(dar.into());
                }
            }
            if result.aspect_ratio.is_none() {
                if let (Some(w), Some(h)) = (result.width, result.height) {
                    if w > 0 && h > 0 {
                        result.aspect_ratio = Some(simplify_aspect(w, h));
                    }
                }
            }
            let mut fps = s.get("avg_frame_rate").and_then(|v| v.as_str());
            if fps.is_none() || matches!(fps, Some("0/0" | "N/A" | "")) {
                fps = s.get("r_frame_rate").and_then(|v| v.as_str());
            }
            result.fps = parse_fps(fps.unwrap_or(""));
        } else if codec_type.eq_ignore_ascii_case("audio") && result.audio_codec.is_none() {
            result.audio_codec = codec;
        }
    }
}

pub fn parse_fps(rate: &str) -> Option<f64> {
    if rate.is_empty() || rate == "0/0" || rate == "N/A" {
        return None;
    }
    if let Some((a, b)) = rate.split_once('/') {
        let num: f64 = a.parse().ok()?;
        let den: f64 = b.parse().ok()?;
        if den > 0.0 {
            let fps = num / den;
            if fps > 0.0 && fps < 240.0 {
                return Some((fps * 100.0).round() / 100.0);
            }
        }
        return None;
    }
    let plain: f64 = rate.parse().ok()?;
    if plain > 0.0 && plain < 240.0 {
        Some((plain * 100.0).round() / 100.0)
    } else {
        None
    }
}

pub fn simplify_aspect(w: i32, h: i32) -> String {
    if w <= 0 || h <= 0 {
        return format!("{w}:{h}");
    }
    let ratio = w as f64 / h as f64;
    if (ratio - 16.0 / 9.0).abs() < 0.03 {
        return "16:9".into();
    }
    if (ratio - 4.0 / 3.0).abs() < 0.03 {
        return "4:3".into();
    }
    if (ratio - 21.0 / 9.0).abs() < 0.05 {
        return "21:9".into();
    }
    let g = gcd(w, h);
    format!("{}:{}", w / g, h / g)
}

fn gcd(mut a: i32, mut b: i32) -> i32 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}

fn run_tool(bin: &str, args: &[&str], timeout: Duration) -> (bool, String, bool) {
    let mut child = match Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, e.to_string(), false),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut err = String::new();
                if let Some(mut s) = child.stderr.take() {
                    let _ = s.read_to_string(&mut err);
                }
                return (status.success(), err, false);
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    kill_tree(&mut child);
                    return (false, String::new(), true);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return (false, e.to_string(), false),
        }
    }
}

fn run_tool_stdout(bin: &str, args: &[&str], timeout: Duration) -> (bool, String, bool) {
    let mut child = match Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, e.to_string(), false),
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut out = String::new();
                if let Some(mut s) = child.stdout.take() {
                    let _ = s.read_to_string(&mut out);
                }
                return (status.success(), out, false);
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    kill_tree(&mut child);
                    return (false, String::new(), true);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return (false, e.to_string(), false),
        }
    }
}

fn kill_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let pid = child.id();
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_class_buckets_ffmpeg_messages() {
        let cases = [
            ("Offline slate (match distance 4)", "offline-slate"),
            ("Error opening input files: Error number -138 occurred", "connect-timeout"),
            ("Error opening input files: Server returned 404 Not Found", "http-404"),
            ("Error opening input files: I/O error", "io-error"),
            ("Timeout", "probe-timeout"),
            ("Terminating thread with return code -1145393733", "decode-abort"),
        ];
        for (err, expect) in cases {
            assert_eq!(classify_error(err), expect, "{err}");
        }
    }

    #[test]
    fn average_hash_identical_frames_are_distance_zero() {
        let gray: Vec<u8> = (0..256).map(|i| ((i * 17) % 256) as u8).collect();
        let a = hash_gray_16x16(&gray).unwrap();
        let b = hash_gray_16x16(&gray).unwrap();
        assert_eq!(hamming(&a, &b), 0);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn average_hash_uniform_frames_hash_to_zero() {
        let black = hash_gray_16x16(&vec![0u8; 256]).unwrap();
        let white = hash_gray_16x16(&vec![255u8; 256]).unwrap();
        assert_eq!(hamming(&black, &white), 0);
        assert!(black.iter().all(|b| *b == 0));
    }

    #[test]
    fn average_hash_center_text_blob_is_not_black() {
        let mut slate = vec![0u8; 256];
        for y in 6..=10 {
            for x in 3..=12 {
                slate[y * 16 + x] = 220;
            }
        }
        let h = hash_gray_16x16(&slate).unwrap();
        let black = hash_gray_16x16(&vec![0u8; 256]).unwrap();
        let dist = hamming(&h, &black);
        assert!(
            dist > MATCH_DISTANCE,
            "distance {dist} should exceed match threshold"
        );
    }

    #[test]
    fn process_store_roundtrip_job_queue_feed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auditprocess.db");
        {
            let store = ProcessStore::open(Some(&path)).unwrap();
            let job = AuditJob {
                id: "job1".into(),
                state: "running".into(),
                scope: "visible".into(),
                auto_swap: true,
                total: 3,
                current_index: 1,
                ok_count: 1,
                fail_count: 0,
                started_at: "2026-08-16T12:00:00Z".into(),
                updated_at: "2026-08-16T12:01:00Z".into(),
                finished_at: None,
                pid: 99,
                grades_json: serialize_grades(&HashMap::from([("A".into(), 1)])),
                first_eta_seconds: 0,
                elapsed_ms: 0,
            };
            store
                .replace_job(
                    &job,
                    &[
                        AuditQueueItem {
                            seq: 0,
                            variant_id: "v0".into(),
                            channel_id: "c0".into(),
                            channel_name: "CNN".into(),
                            group_title: "NEWS".into(),
                            visibility: "visible".into(),
                            done: true,
                        },
                        AuditQueueItem {
                            seq: 1,
                            variant_id: "v1".into(),
                            channel_id: "c1".into(),
                            channel_name: "ESPN".into(),
                            group_title: "SPORTS".into(),
                            visibility: "visible".into(),
                            done: false,
                        },
                        AuditQueueItem {
                            seq: 2,
                            variant_id: "v2".into(),
                            channel_id: "c2".into(),
                            channel_name: "BBC".into(),
                            group_title: "UK".into(),
                            visibility: "visible".into(),
                            done: false,
                        },
                    ],
                )
                .unwrap();
            store
                .append_feed(&AuditFeedRow {
                    seq: 0,
                    is_header: true,
                    title: "NEWS".into(),
                    subtitle: "starting group".into(),
                    detail: String::new(),
                    grade: String::new(),
                    status_label: String::new(),
                    latency_label: String::new(),
                    ok: false,
                })
                .unwrap();
            store
                .append_feed(&AuditFeedRow {
                    seq: 1,
                    is_header: false,
                    title: "CNN".into(),
                    subtitle: "NEWS · primary".into(),
                    detail: "1920×1080".into(),
                    grade: "A".into(),
                    status_label: "OK".into(),
                    latency_label: String::new(),
                    ok: true,
                })
                .unwrap();
        }
        let reload = ProcessStore::open(Some(&path)).unwrap();
        let loaded = reload.load_job().unwrap().unwrap();
        assert_eq!(loaded.id, "job1");
        assert_eq!(loaded.state, "running");
        assert_eq!(loaded.total, 3);
        assert_eq!(loaded.current_index, 1);
        assert!(loaded.has_remaining());
        let q = reload.load_queue().unwrap();
        assert_eq!(q.len(), 3);
        assert!(q[0].done);
        assert!(!q[1].done);
        assert_eq!(q[1].channel_name, "ESPN");
        let feed = reload.load_feed().unwrap();
        assert_eq!(feed.len(), 2);
        assert!(feed[0].is_header);
        assert_eq!(feed[1].title, "CNN");
        assert!(!feed[1].detail.to_ascii_lowercase().contains("http"));
    }

    #[test]
    fn process_store_clear_removes_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("clear.db");
        let store = ProcessStore::open(Some(&path)).unwrap();
        store
            .replace_job(
                &AuditJob {
                    id: "x".into(),
                    state: "paused".into(),
                    scope: "visible".into(),
                    auto_swap: false,
                    total: 2,
                    current_index: 0,
                    ok_count: 0,
                    fail_count: 0,
                    started_at: now_iso(),
                    updated_at: now_iso(),
                    finished_at: None,
                    pid: 1,
                    grades_json: "{}".into(),
                    first_eta_seconds: 0,
                    elapsed_ms: 0,
                },
                &[AuditQueueItem {
                    seq: 0,
                    variant_id: "v".into(),
                    channel_id: "c".into(),
                    channel_name: "X".into(),
                    group_title: "G".into(),
                    visibility: "visible".into(),
                    done: false,
                }],
            )
            .unwrap();
        assert!(store.load_job().unwrap().is_some());
        store.clear().unwrap();
        assert!(store.load_job().unwrap().is_none());
        assert!(store.load_queue().unwrap().is_empty());
        assert!(store.load_feed().unwrap().is_empty());
    }

    #[test]
    fn finalize_grade_dead_is_f() {
        let r = finalize_grade(AuditResult {
            ok: false,
            ..AuditResult::default()
        });
        assert_eq!(r.grade, "F");
    }

    #[test]
    fn parse_fps_fraction() {
        assert_eq!(parse_fps("30000/1001"), Some(29.97));
        assert_eq!(parse_fps("0/0"), None);
    }
}
