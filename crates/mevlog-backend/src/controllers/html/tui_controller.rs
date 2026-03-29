use crate::{
    config::{host, routes::html_response},
    misc::utils::deployed_at,
};
use askama::Template;
use axum::response::IntoResponse;
use reqwest::StatusCode;

#[derive(Template)]
#[template(path = "tui.html")]
struct TuiTemplate {
    host: String,
    page: String,
    deployed_at: String,
    title: String,
    description: String,
    canonical_url: String,
}

#[hotpath::measure]
pub async fn tui() -> impl IntoResponse {
    let h = host();
    let template = TuiTemplate {
        title: "TUI Terminal Interface - mevlog.rs".to_string(),
        description: "Query EVM transactions from your terminal with mevlog-rs TUI. Vim-style navigation, multi-chain support, flexible filters, and EVM tracing insights.".to_string(),
        canonical_url: format!("{h}/tui"),
        host: h,
        page: "tui".to_string(),
        deployed_at: deployed_at(),
    };

    html_response(template.render().unwrap(), StatusCode::OK)
}
