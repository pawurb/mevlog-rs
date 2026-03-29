use crate::config::{host, routes::html_response};
use crate::controllers::json::explore_controller::ExploreParams;
use crate::misc::utils::deployed_at;
use askama::Template;
use axum::{extract::Query, response::IntoResponse};
use reqwest::StatusCode;

#[derive(Template)]
#[template(path = "explore.html")]
struct ExploreTemplate {
    host: String,
    page: String,
    deployed_at: String,
    chain_id: Option<u64>,
    block_number: Option<String>,
    title: String,
    description: String,
    canonical_url: String,
}

impl ExploreTemplate {
    pub fn new(chain_id: Option<u64>, block_number: Option<String>) -> Self {
        let h = host();
        let canonical_url = format!("{h}/explore");
        Self {
            title: "Explore EVM Blocks - mevlog.rs".to_string(),
            description: "Explore the latest blocks and transactions across 2000+ EVM-compatible chains. View transaction details with EVM tracing insights.".to_string(),
            canonical_url,
            host: h,
            page: "explore".to_string(),
            deployed_at: deployed_at(),
            chain_id,
            block_number,
        }
    }

    pub fn chain_id_value(&self) -> u64 {
        self.chain_id.unwrap_or(1)
    }

    pub fn block_number_value(&self) -> &str {
        self.block_number.as_deref().unwrap_or("latest")
    }
}

#[hotpath::measure]
pub async fn explore(Query(params): Query<ExploreParams>) -> impl IntoResponse {
    let chain_id = if params.chain_id == Some(1) {
        None
    } else {
        params.chain_id
    };
    let block_number = if params.block_number == Some("latest".to_string()) {
        None
    } else {
        params.block_number
    };

    let template = ExploreTemplate::new(chain_id, block_number);
    html_response(template.render().unwrap(), StatusCode::OK)
}
