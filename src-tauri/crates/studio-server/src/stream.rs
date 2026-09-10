// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::TryStreamExt;
use serde::Deserialize;
use studio_core::logo::PLAYER_UA;

use crate::dispatch::Host;
use crate::App;

#[derive(Deserialize)]
pub struct StreamQuery {
    url: String,
    #[serde(default)]
    #[serde(rename = "sourceId")]
    source_id: Option<String>,
}

pub async fn stream_handler(
    State(app): State<App>,
    Query(q): Query<StreamQuery>,
    inbound: HeaderMap,
) -> impl IntoResponse {
    let url = q.url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return (StatusCode::BAD_REQUEST, "url must be http(s)").into_response();
    }
    let extra = source_headers(&app.host, q.source_id.as_deref());
    let mut ua = PLAYER_UA.to_string();
    for (k, v) in &extra {
        if k.eq_ignore_ascii_case("user-agent") && !v.trim().is_empty() {
            ua = v.clone();
        }
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .build();
    let Ok(client) = client else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "http client").into_response();
    };
    let mut req = client.get(url).header(header::USER_AGENT, &ua);
    for (k, v) in &extra {
        if k.eq_ignore_ascii_case("user-agent") {
            continue;
        }
        req = req.header(k.as_str(), v.as_str());
    }
    if let Some(range) = inbound.get(header::RANGE) {
        req = req.header(header::RANGE, range);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let mut builder = axum::response::Response::builder().status(status.as_u16());
            if let Some(ct) = resp.headers().get(header::CONTENT_TYPE) {
                builder = builder.header(header::CONTENT_TYPE, ct);
            } else {
                builder = builder.header(header::CONTENT_TYPE, "video/mp2t");
            }
            if let Some(cr) = resp.headers().get(header::CONTENT_RANGE) {
                builder = builder.header(header::CONTENT_RANGE, cr);
            }
            builder = builder.header(header::CACHE_CONTROL, "no-store");
            let stream = resp
                .bytes_stream()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e));
            match builder.body(Body::from_stream(stream)) {
                Ok(r) => r,
                Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
            }
        }
        Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
    }
}

fn source_headers(host: &Host, source_id: Option<&str>) -> BTreeMap<String, String> {
    let Some(id) = source_id.filter(|s| !s.is_empty()) else {
        return BTreeMap::new();
    };
    let Ok(store) = host.store.lock() else {
        return BTreeMap::new();
    };
    let Ok(list) = store.list_sources() else {
        return BTreeMap::new();
    };
    list.into_iter()
        .find(|s| s.id == id)
        .and_then(|s| serde_json::from_str(&s.headers_json).ok())
        .unwrap_or_default()
}
