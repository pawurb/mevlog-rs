use crate::{
    config::{host, routes::html_response},
    misc::utils::deployed_at,
};
use askama::Template;
use axum::response::IntoResponse;
use reqwest::StatusCode;

#[derive(Template)]
#[template(path = "404.html")]
struct NotFoundTemplate {
    host: String,
    page: String,
    deployed_at: String,
    title: String,
    description: String,
    canonical_url: String,
}

pub async fn not_found() -> impl IntoResponse {
    let h = host();
    let template = NotFoundTemplate {
        title: "Page Not Found - mevlog.rs".to_string(),
        description: "The page you are looking for does not exist.".to_string(),
        canonical_url: format!("{h}/"),
        host: h,
        page: "404".to_string(),
        deployed_at: deployed_at(),
    };

    html_response(template.render().unwrap(), StatusCode::NOT_FOUND)
}
