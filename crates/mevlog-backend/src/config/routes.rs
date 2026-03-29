use crate::{controllers::*, misc::utils::deployed_at};
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
};
use tower::Layer;
use tower_http::services::{ServeDir, ServeFile};

use super::{cache_control, host};

pub async fn app() -> Router {
    let deployed_at = deployed_at();

    Router::new()
        .route("/", get(html::home_controller::home))
        .route("/search", get(html::search_controller::search))
        .route("/terms", get(html::terms_controller::terms))
        .route("/explore", get(html::explore_controller::explore))
        .route("/tui", get(html::tui_controller::tui))
        .route(
            "/api/chain-info",
            get(json::chain_info_controller::chain_info),
        )
        .route("/api/chains", get(json::chains_controller::chains))
        .route("/api/explore", get(json::explore_controller::explore))
        .route("/ws/search", get(websocket::search_controller::ws_handler))
        .route("/uptime", get(|| async move { "OK".into_response() }))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route_service(
            &format!("/{deployed_at}-scripts.js"),
            cache_control().layer(ServeFile::new(format!("assets/{deployed_at}-scripts.js"))),
        )
        .route_service(
            &format!("/{deployed_at}-styles.css"),
            cache_control().layer(ServeFile::new(format!("assets/{deployed_at}-styles.css"))),
        )
        .route_service(
            &format!("/{deployed_at}-terminal.css"),
            cache_control().layer(ServeFile::new(format!("assets/{deployed_at}-terminal.css"))),
        )
        .route_service(
            &format!("/{deployed_at}-react-bundle.js"),
            cache_control().layer(ServeFile::new(format!(
                "assets/{deployed_at}-react-bundle.js"
            ))),
        )
        .nest_service("/assets", cache_control().layer(ServeDir::new("assets")))
        .route_service(
            "/all-chains.png",
            cache_control().layer(ServeFile::new("assets/all-chains.png")),
        )
        .route_service(
            "/custom-queries.png",
            cache_control().layer(ServeFile::new("assets/custom-queries.png")),
        )
        .route_service(
            "/favicon.ico",
            cache_control().layer(ServeFile::new("assets/favicon.ico")),
        )
        .route_service(
            "/find-outliers.png",
            cache_control().layer(ServeFile::new("assets/find-outliers.png")),
        )
        .route_service(
            "/mevlog-logo.png",
            cache_control().layer(ServeFile::new("assets/mevlog-logo.png")),
        )
        .route_service(
            "/open-source.png",
            cache_control().layer(ServeFile::new("assets/open-source.png")),
        )
        .route_service(
            "/mevlog-demo.mp4",
            cache_control().layer(ServeFile::new("assets/mevlog-demo.mp4")),
        )
        .route_service(
            "/media/mevlog-tui-demo.mp4",
            cache_control().layer(ServeFile::new("media/mevlog-tui-demo.mp4")),
        )
        .fallback(html::not_found_controller::not_found)
}

async fn robots_txt() -> Response<Body> {
    let h = host();
    let body = format!("User-agent: *\nAllow: /\n\nSitemap: {h}/sitemap.xml\n");
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    (StatusCode::OK, headers, body).into_response()
}

async fn sitemap_xml() -> Response<Body> {
    let h = host();
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>{h}/</loc>
    <priority>1.0</priority>
  </url>
  <url>
    <loc>{h}/explore</loc>
    <priority>0.9</priority>
  </url>
  <url>
    <loc>{h}/search</loc>
    <priority>0.9</priority>
  </url>
  <url>
    <loc>{h}/tui</loc>
    <priority>0.7</priority>
  </url>
  <url>
    <loc>{h}/terms</loc>
    <priority>0.3</priority>
  </url>
</urlset>
"#
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    (StatusCode::OK, headers, body).into_response()
}

pub fn invalid_req(reason: &str) -> Response<Body> {
    (StatusCode::BAD_REQUEST, reason.to_string()).into_response()
}

#[hotpath::measure]
pub fn html_response(body: String, status: StatusCode) -> Response<Body> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("text/html; charset=utf-8"),
    );

    (status, headers, body).into_response()
}

pub fn json_response(body: String, status: StatusCode) -> Response<Body> {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    (status, headers, body).into_response()
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use axum::http::Request;
    use eyre::Result;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    pub async fn get_test_app() -> Result<Router> {
        Ok(app().await)
    }

    #[tokio::test]
    async fn uptime_test() -> Result<()> {
        let app = get_test_app().await?;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/uptime")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body, "OK");
        Ok(())
    }
}
