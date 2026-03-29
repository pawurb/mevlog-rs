use crate::{
    config::{host, routes::html_response},
    misc::utils::deployed_at,
};
use askama::Template;
use axum::response::IntoResponse;
use reqwest::StatusCode;

#[derive(Template)]
#[template(path = "terms.html")]
struct TermsTemplate {
    host: String,
    page: String,
    deployed_at: String,
    title: String,
    description: String,
    canonical_url: String,
}

#[hotpath::measure]
pub async fn terms() -> impl IntoResponse {
    let h = host();
    let template = TermsTemplate {
        title: "Privacy Policy & Terms - mevlog.rs".to_string(),
        description: "Privacy policy and terms of use for mevlog.rs.".to_string(),
        canonical_url: format!("{h}/terms"),
        host: h,
        page: "terms".to_string(),
        deployed_at: deployed_at(),
    };

    html_response(template.render().unwrap(), StatusCode::OK)
}
