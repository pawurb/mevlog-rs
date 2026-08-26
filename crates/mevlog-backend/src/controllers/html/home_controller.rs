use askama::Template;
use axum::response::IntoResponse;
use reqwest::StatusCode;

use crate::{
    config::{host, routes::html_response},
    misc::utils::deployed_at,
};

// force html views recompilation by changing this value
const _VIEW_VERSION: u64 = 20;

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
pub(crate) async fn home() -> impl IntoResponse {
    tracing::debug!("Home controller called");

    let h = host();
    let template = HomeTemplate {
        title: "mevlog-rs | Free Open-Source Dune Alternative - SQL Analytics for Ethereum & EVM Chains".to_string(),
        description: "Free, open-source Dune Analytics alternative. Query EVM transactions across 2000+ chains with SQL - no paid plans, no rate limits. Search by events, method calls, ENS names, ERC20 transfers, and more.".to_string(),
        canonical_url: format!("{h}/"),
        host: h,
        page: "home".to_string(),
        deployed_at: deployed_at(),
    };

    html_response(template.render().unwrap(), StatusCode::OK)
}
