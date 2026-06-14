use crate::{
    content::{app_pages::APP_PAGES, doc_pages::DOC_PAGES},
    controllers::*,
    misc::utils::deployed_at,
};
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, HeaderValue, Response, StatusCode},
    middleware::from_fn,
    response::{IntoResponse, Redirect},
    routing::get,
};
use tower::Layer;
use tower_http::services::{ServeDir, ServeFile};

use super::{cache_control, docs_html_ext, docs_seo, host};

pub async fn app() -> Router {
    let deployed_at = deployed_at();

    Router::new()
        .route("/", get(html::home_controller::home))
        .route("/search", get(html::search_controller::search))
        .route("/explore", get(html::explore_controller::explore))
        .route(
            "/api/chain-info",
            get(json::chain_info_controller::chain_info),
        )
        .route("/api/chains", get(json::chains_controller::chains))
        .route("/api/db-info", get(json::db_info_controller::db_info))
        .route("/api/explore", get(json::explore_controller::explore))
        .route("/api/search", get(json::search_controller::search))
        .route("/uptime", get(|| async move { "OK".into_response() }))
        .route("/robots.txt", get(robots_txt))
        .route("/sitemap.xml", get(sitemap_xml))
        .route_service(
            &format!("/{deployed_at}-scripts.js"),
            from_fn(cache_control)
                .layer(ServeFile::new(format!("assets/{deployed_at}-scripts.js"))),
        )
        .route_service(
            &format!("/{deployed_at}-styles.css"),
            from_fn(cache_control)
                .layer(ServeFile::new(format!("assets/{deployed_at}-styles.css"))),
        )
        .route_service(
            &format!("/{deployed_at}-terminal.css"),
            from_fn(cache_control)
                .layer(ServeFile::new(format!("assets/{deployed_at}-terminal.css"))),
        )
        .route_service(
            &format!("/{deployed_at}-react-bundle.js"),
            from_fn(cache_control).layer(ServeFile::new(format!(
                "assets/{deployed_at}-react-bundle.js"
            ))),
        )
        .nest_service(
            "/assets",
            from_fn(cache_control).layer(ServeDir::new("assets")),
        )
        .route("/docs", get(|| async { Redirect::permanent("/docs/") }))
        .nest_service(
            "/docs/",
            from_fn(docs_html_ext).layer(from_fn(cache_control).layer(
                ServeDir::new("docs_html").not_found_service(ServeFile::new("docs_html/404.html")),
            )),
        )
        .route_service(
            "/all-chains.png",
            from_fn(cache_control).layer(ServeFile::new("assets/all-chains.png")),
        )
        .route_service(
            "/custom-queries.png",
            from_fn(cache_control).layer(ServeFile::new("assets/custom-queries.png")),
        )
        .route_service(
            "/favicon.ico",
            from_fn(cache_control).layer(ServeFile::new("assets/favicon.ico")),
        )
        .route_service(
            "/sql-functions.jpeg",
            from_fn(cache_control).layer(ServeFile::new("assets/sql-functions.jpeg")),
        )
        .route_service(
            "/github-stars.svg",
            from_fn(cache_control).layer(ServeFile::new("assets/github-stars.svg")),
        )
        .route_service(
            "/mevlog-logo.png",
            from_fn(cache_control).layer(ServeFile::new("assets/mevlog-logo.png")),
        )
        .route_service(
            "/open-source.png",
            from_fn(cache_control).layer(ServeFile::new("assets/open-source.png")),
        )
        .route_service(
            "/mevlog-demo.mp4",
            from_fn(cache_control).layer(ServeFile::new("assets/mevlog-demo.mp4")),
        )
        .fallback(html::not_found_controller::not_found)
        .layer(from_fn(docs_seo))
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
    let app_entries = APP_PAGES
        .iter()
        .map(|p| (p.path, p.sitemap_changefreq, p.sitemap_priority));
    let doc_entries = DOC_PAGES
        .iter()
        .map(|p| (p.path, p.sitemap_changefreq, p.sitemap_priority));
    let urls = app_entries
        .chain(doc_entries)
        .map(|(path, changefreq, priority)| {
            format!(
                "  <url>\n    <loc>{h}{path}</loc>\n    <changefreq>{changefreq}</changefreq>\n    <priority>{priority}</priority>\n  </url>\n",
            )
        })
        .collect::<String>();
    let body = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
{urls}</urlset>
"#
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    (StatusCode::OK, headers, body).into_response()
}

#[hotpath::measure]
pub(crate) fn html_response(body: String, status: StatusCode) -> Response<Body> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_static("text/html; charset=utf-8"),
    );

    (status, headers, body).into_response()
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use axum::http::Request;
    use eyre::Result;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    pub(crate) async fn get_test_app() -> Result<Router> {
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
