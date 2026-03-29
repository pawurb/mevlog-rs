use askama::Template;
use axum::response::IntoResponse;
use reqwest::StatusCode;

use crate::{
    config::{host, routes::html_response},
    misc::utils::deployed_at,
};

// force html views recompilation by changing this value
const _VIEW_VERSION: u64 = 18;

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    host: String,
    page: String,
    deployed_at: String,
    title: String,
    description: String,
    canonical_url: String,
}

#[hotpath::measure]
pub async fn home() -> impl IntoResponse {
    tracing::debug!("Home controller called");

    let h = host();
    let template = HomeTemplate {
        title: "mevlog.rs - Explore EVM chains in one place, powered by Revm".to_string(),
        description: "Open-source web interface for querying EVM transactions across 2000+ chains. Search by events, method calls, ENS names, ERC20 transfers, and more.".to_string(),
        canonical_url: format!("{h}/"),
        host: h,
        page: "home".to_string(),
        deployed_at: deployed_at(),
    };

    html_response(template.render().unwrap(), StatusCode::OK)
}
