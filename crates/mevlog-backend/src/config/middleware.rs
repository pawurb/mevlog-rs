use axum::http::Method;
use axum::{
    extract::Request,
    http::{HeaderValue, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
};
use reqwest::header::CACHE_CONTROL;
use tower_http::cors::{Any, CorsLayer};

use reqwest::StatusCode;
use time::UtcOffset;

use std::time::Instant;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::info_span;
use tracing_futures::Instrument;
use tracing_subscriber::fmt::time::OffsetTime;
use uuid::Uuid;

#[derive(Debug, PartialEq)]
pub enum Env {
    Development,
    Production,
    Test,
}

impl Env {
    pub fn current() -> Self {
        match std::env::var("ENV")
            .unwrap_or_else(|_| "development".to_string())
            .as_str()
        {
            "development" => Self::Development,
            "production" => Self::Production,
            "test" => Self::Test,
            _ => panic!("Invalid ENV"),
        }
    }

    pub fn is_dev(&self) -> bool {
        self == &Self::Development
    }
}

const STATIC_EXTENSIONS: [&str; 11] = [
    ".js", ".css", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2", ".ttf",
];

pub async fn request_tracing(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();

    // Skip instrumenting static asset requests
    if STATIC_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
        return next.run(request).await;
    }

    hotpath::gauge!("requests_count").inc(1);
    hotpath::dbg!(&path);

    let uuid = Uuid::new_v4();
    let uuid_str = uuid.to_string().replace("-", "");
    let request_id = &uuid_str[0..12];
    let method = request.method().clone();

    let info_span = info_span!("req", id = %request_id, method = %method, path = %path);

    async move {
        let start = Instant::now();
        let response = next.run(request).await;
        let duration = start.elapsed();

        tracing::info!(
            status = %response.status(),
            duration_ms = duration.as_millis(),
        );

        response
    }
    .instrument(info_span)
    .await
}

pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    response.headers_mut().insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("X-Frame-Options", HeaderValue::from_static("SAMEORIGIN"));
    response.headers_mut().insert(
        "referrer-policy",
        HeaderValue::from_static("no-referrer-when-downgrade"),
    );
    response.headers_mut().insert(
        "Strict-Transport-Security",
        HeaderValue::from_static("Strict-Transport-Security: max-age=31536000; includeSubDomains"),
    );

    response
}

pub fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(Any)
}

pub fn cache_control() -> SetResponseHeaderLayer<HeaderValue> {
    let cache_control = if Env::current().is_dev() {
        HeaderValue::from_static("no-cache, no-store, must-revalidate, max-age=0")
    } else {
        HeaderValue::from_static("public, max-age=1382400, immutable")
    };

    SetResponseHeaderLayer::overriding(CACHE_CONTROL, cache_control)
}

pub async fn only_ssl(request: Request, next: Next) -> Response {
    let ssl = request
        .headers()
        .get("x-ssl")
        .and_then(|header| header.to_str().ok())
        == Some("true");

    if ssl || Env::current().is_dev() {
        next.run(request).await
    } else {
        let authority = request
            .headers()
            .get("host")
            .and_then(|header| header.to_str().ok())
            .unwrap_or("localhost");

        let uri = Uri::builder()
            .scheme("https")
            .authority(authority)
            .path_and_query(request.uri().path_and_query().unwrap().clone())
            .build()
            .unwrap();

        Response::builder()
            .status(StatusCode::MOVED_PERMANENTLY)
            .header("Location", uri.to_string())
            .body(axum::body::Body::empty())
            .unwrap()
            .into_response()
    }
}

pub fn init_logs(filename: &str) {
    match Env::current() {
        Env::Production => {
            let file_appender = tracing_appender::rolling::never("./", filename);

            let offset = UtcOffset::from_hms(2, 0, 0).expect("should get CET offset");
            let time_format =
                time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]")
                    .unwrap();
            let timer = OffsetTime::new(offset, time_format);

            tracing_subscriber::fmt()
                .with_writer(file_appender)
                .with_timer(timer)
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .init();
        }
        _ => {
            let filter = tracing_subscriber::EnvFilter::from_default_env();

            tracing_subscriber::fmt().with_env_filter(filter).init()
        }
    }
}

pub fn host() -> String {
    match Env::current() {
        Env::Development | Env::Test => "http://localhost:3000",
        Env::Production => "https://mevlog.rs",
    }
    .to_string()
}
