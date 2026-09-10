// SPDX-License-Identifier: GPL-3.0-or-later

mod auth;
mod dispatch;
mod stream;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{DefaultBodyLimit, Multipart, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use studio_core::audit;
use studio_core::paths::{app_data_directory, configure_from_launch_dir, database_path};
use studio_core::store::SqliteStore;
use studio_tuner::host::TunerSnapshot;
use studio_tuner::manager::TunerManager;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::auth::Auth;
use crate::dispatch::{invoke, Host};

#[derive(Clone)]
struct App {
    host: Host,
    auth: Arc<Mutex<Auth>>,
}

#[derive(Deserialize)]
struct InvokeReq {
    cmd: String,
    #[serde(default)]
    args: serde_json::Value,
}

#[derive(Serialize)]
struct InvokeOk {
    result: serde_json::Value,
}

#[derive(Serialize)]
struct InvokeErr {
    error: String,
}

fn flag(args: &[String], names: &[&str]) -> bool {
    args.iter().any(|a| names.iter().any(|n| a == n))
}

fn flag_value<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    args.iter()
        .position(|a| names.iter().any(|n| a == n))
        .and_then(|i| args.get(i + 1))
        .filter(|s| !s.starts_with('-'))
        .map(|s| s.as_str())
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).unwrap_or_default().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if flag(&args, &["--makepass", "makepass"]) {
        configure_from_launch_dir();
        match auth::make_temp_password(flag_value(&args, &["--makepass", "makepass"])) {
            Ok(pw) => {
                println!("Username: admin");
                println!("Temporary password: {pw}");
                println!("Sign in on the web UI, then set a new password. The temporary password will not work after that.");
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if flag(&args, &["--makekey", "makekey"]) {
        configure_from_launch_dir();
        match auth::create_api_key(flag_value(&args, &["--makekey", "makekey"])) {
            Ok(k) => {
                println!("Name: {}", k.name);
                println!("Desktop API key (shown once): {}", k.key);
                println!("Paste the key in desktop Settings → This computer → Connect.");
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let headless = flag(&args, &["--headless", "headless"]) || env_flag("STUDIO_HEADLESS");
    run_server(headless);
}

#[tokio::main]
async fn run_server(headless: bool) {
    configure_from_launch_dir();
    studio_core::crash::append_log("Info", "App", "studio-server starting");
    studio_core::update::cleanup_stale_update_files();
    let data = app_data_directory();
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(data.join("cache"));
    let _ = std::fs::create_dir_all(data.join("uploads"));
    let db = database_path();
    let store = match SqliteStore::open(&db) {
        Ok(s) => s,
        Err(e) => studio_core::crash::startup_fatal(
            "Could not open the studio database.",
            &format!("{}\n{e}", db.display()),
        ),
    };
    let audit_store = match audit::ProcessStore::open(None) {
        Ok(s) => s,
        Err(e) => studio_core::crash::startup_fatal(
            "Could not open the stream-audit database.",
            &e.to_string(),
        ),
    };
    let store = Arc::new(Mutex::new(store));
    let mut tuner = TunerManager::new();
    let snap = tuner_snapshot_fn(Arc::clone(&store));
    let loaded = store.lock().unwrap_or_else(|e| e.into_inner()).load_settings();
    if let Ok(mut settings) = loaded {
        tuner.apply(&mut settings, snap);
        let _ = store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .save_settings(&settings);
    }
    let (tx, _) = broadcast::channel::<String>(256);
    let root = std::env::var("EPG_MONSTER_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let host = Host {
        store,
        audit: Arc::new(Mutex::new(audit_store)),
        tuner: Arc::new(Mutex::new(tuner)),
        events: tx,
        root,
    };
    let bind = std::env::var("STUDIO_BIND").unwrap_or_else(|_| "0.0.0.0:1420".into());
    let addr: SocketAddr = bind.parse().expect("STUDIO_BIND host:port");
    let ui = if headless {
        None
    } else {
        std::env::var("STUDIO_UI_DIR").ok().map(PathBuf::from)
    };
    if headless && auth::api_key_count() == 0 {
        match auth::create_api_key(Some("headless")) {
            Ok(k) => {
                eprintln!("No desktop API key yet. Generated one (shown once):");
                eprintln!("  {}", k.key);
                eprintln!("Paste it in desktop Settings → This computer → Connect.");
            }
            Err(e) => eprintln!("could not create a desktop API key: {e}"),
        }
    }
    let app = App {
        host,
        auth: Arc::new(Mutex::new(Auth::default())),
    };
    let mut router = Router::new()
        .route("/api/invoke", post(invoke_handler))
        .route("/api/events", get(events_handler))
        .route("/api/upload", post(upload_handler))
        .route("/api/login", post(login_handler))
        .route("/api/logout", post(logout_handler))
        .route("/api/password", post(password_handler))
        .route("/api/session", get(session_handler))
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/stream", get(stream::stream_handler))
        .layer(middleware::from_fn_with_state(app.clone(), require_auth))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any))
        .layer(DefaultBodyLimit::max(80 * 1024 * 1024))
        .with_state(app);
    if let Some(dir) = ui.filter(|p| p.is_dir()) {
        let index = dir.join("index.html");
        router = router.fallback_service(ServeDir::new(&dir).not_found_service(ServeFile::new(index)));
    }
    if headless {
        eprintln!("epg.monster studio API (headless)  {addr}");
        eprintln!("No web UI. Connect from desktop Settings with the API key.");
    } else {
        eprintln!("epg.monster studio web  http://127.0.0.1:{}", addr.port());
    }
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    axum::serve(listener, router).await.expect("serve");
}

fn tuner_snapshot_fn(
    store: Arc<Mutex<SqliteStore>>,
) -> Arc<dyn Fn() -> TunerSnapshot + Send + Sync> {
    Arc::new(move || {
        let g = store.lock().ok();
        match g {
            Some(s) => {
                let channels = s.list_managed(None).unwrap_or_default();
                let programmes = Vec::new();
                let settings = s.load_settings().unwrap_or_default();
                studio_tuner::manager::snapshot_from_settings(channels, programmes, &settings)
            }
            None => studio_tuner::manager::snapshot_from_settings(
                Vec::new(),
                Vec::new(),
                &studio_core::settings::AppSettings::default(),
            ),
        }
    })
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct PasswordReq {
    current: String,
    next: String,
}

fn cookie_header(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers.get(header::COOKIE).and_then(|v| v.to_str().ok())
}

fn bearer_header(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
}

fn api_key_ok(headers: &axum::http::HeaderMap) -> bool {
    auth::parse_bearer(bearer_header(headers))
        .map(|k| auth::verify_api_key(&k))
        .unwrap_or(false)
}

async fn require_auth(State(app): State<App>, req: Request, next: Next) -> impl IntoResponse {
    let path = req.uri().path().to_string();
    let open = matches!(
        path.as_str(),
        "/api/login" | "/api/health" | "/api/session"
    ) || !path.starts_with("/api/");
    if open {
        return next.run(req).await;
    }
    if api_key_ok(req.headers()) {
        return next.run(req).await;
    }
    let token = Auth::parse_cookie(cookie_header(req.headers()));
    let sess = token.and_then(|t| app.auth.lock().ok().and_then(|g| g.session(&t)));
    match sess {
        Some(s) if s.must_change && path != "/api/password" && path != "/api/logout" => (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({ "error": "password change required", "mustChange": true })),
        )
            .into_response(),
        Some(_) => next.run(req).await,
        None => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "login required" })),
        )
            .into_response(),
    }
}

async fn session_handler(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    if api_key_ok(&headers) {
        return Json(serde_json::json!({
            "ok": true,
            "username": "apikey",
            "kind": "apikey",
            "mustChange": false,
        }))
        .into_response();
    }
    let token = Auth::parse_cookie(cookie_header(&headers));
    let sess = token.and_then(|t| app.auth.lock().ok().and_then(|g| g.session(&t)));
    match sess {
        Some(s) => Json(serde_json::json!({
            "ok": true,
            "username": s.username,
            "kind": "password",
            "mustChange": s.must_change,
        }))
        .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "ok": false })),
        )
            .into_response(),
    }
}

async fn login_handler(State(app): State<App>, Json(req): Json<LoginReq>) -> impl IntoResponse {
    let mut auth = match app.auth.lock() {
        Ok(g) => g,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "lock" })))
                .into_response();
        }
    };
    match auth.login(&req.username, &req.password) {
        Ok((token, sess)) => {
            let mut res = Json(serde_json::json!({
                "ok": true,
                "username": sess.username,
                "mustChange": sess.must_change,
            }))
            .into_response();
            if let Ok(v) = Auth::set_cookie(&token).parse() {
                res.headers_mut().insert(header::SET_COOKIE, v);
            }
            res
        }
        Err(error) => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": error }))).into_response(),
    }
}

async fn logout_handler(State(app): State<App>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    if let Some(t) = Auth::parse_cookie(cookie_header(&headers)) {
        if let Ok(mut g) = app.auth.lock() {
            g.logout(&t);
        }
    }
    let mut res = Json(serde_json::json!({ "ok": true })).into_response();
    if let Ok(v) = Auth::clear_cookie().parse() {
        res.headers_mut().insert(header::SET_COOKIE, v);
    }
    res
}

async fn password_handler(
    State(app): State<App>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PasswordReq>,
) -> impl IntoResponse {
    let Some(token) = Auth::parse_cookie(cookie_header(&headers)) else {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "error": "login required" }))).into_response();
    };
    let mut auth = match app.auth.lock() {
        Ok(g) => g,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "lock" })))
                .into_response();
        }
    };
    match auth.change_password(&token, &req.current, &req.next) {
        Ok(sess) => Json(serde_json::json!({
            "ok": true,
            "username": sess.username,
            "mustChange": sess.must_change,
        }))
        .into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": error }))).into_response(),
    }
}

async fn invoke_handler(State(app): State<App>, Json(req): Json<InvokeReq>) -> impl IntoResponse {
    let relaunch = req.cmd == "apply_studio_update";
    match invoke(&app.host, &req.cmd, req.args).await {
        Ok(result) => {
            if relaunch {
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                    std::process::exit(0);
                });
            }
            (StatusCode::OK, Json(InvokeOk { result })).into_response()
        }
        Err(error) => (StatusCode::BAD_REQUEST, Json(InvokeErr { error })).into_response(),
    }
}

async fn events_handler(
    State(app): State<App>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = app.host.events.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|r| match r {
        Ok(line) => Some(Ok(Event::default().data(line))),
        Err(_) => None,
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn upload_handler(mut multipart: Multipart) -> Result<Json<serde_json::Value>, String> {
    let dir = app_data_directory().join("uploads");
    let _ = std::fs::create_dir_all(&dir);
    while let Some(field) = multipart.next_field().await.map_err(|e| e.to_string())? {
        let name = field.file_name().unwrap_or("upload.bin").to_string();
        let data = field.bytes().await.map_err(|e| e.to_string())?;
        let safe = name.replace(['/', '\\', ':'], "_");
        let path = dir.join(format!("{}-{safe}", uuid::Uuid::new_v4().simple()));
        std::fs::write(&path, &data).map_err(|e| e.to_string())?;
        return Ok(Json(serde_json::json!({ "path": path.to_string_lossy() })));
    }
    Err("no file".into())
}
