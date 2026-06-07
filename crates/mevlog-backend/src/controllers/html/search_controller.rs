use crate::config::{host, routes::html_response};
use crate::controllers::base_controller::empty_string_as_none;
use crate::controllers::json::base_controller::extract_query_params;
use crate::misc::utils::deployed_at;
use askama::Template;
use axum::{extract::Query, response::IntoResponse};
use eyre::Result;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::controllers::base_controller::{error_message, get_default_blocks};

#[derive(Template)]
#[template(path = "search.html")]
struct SearchTemplate {
    blocks: String,
    sql: String,
    host: String,
    page: String,
    deployed_at: String,
    chain_id: String,
    title: String,
    description: String,
    canonical_url: String,
}

impl SearchTemplate {
    pub fn new(params: SearchParams) -> Self {
        let blocks = get_default_blocks(params.blocks);
        let h = host();
        let canonical_url = format!("{h}/search");

        Self {
            blocks,
            sql: params.sql.unwrap_or_default(),
            host: h,
            page: "search".to_string(),
            deployed_at: deployed_at(),
            chain_id: params.chain_id.unwrap_or(1).to_string(),
            title: "Query EVM Transactions - mevlog.rs".to_string(),
            description: "Run read-only SQL queries against indexed EVM transactions, logs, and blocks. Aggregate ERC20 transfers, rank by gas cost, resolve ENS names, and more.".to_string(),
            canonical_url,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SearchParams {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub blocks: Option<String>,
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub sql: Option<String>,
    pub chain_id: Option<u64>,
}

impl SearchParams {
    pub async fn validate(&self) -> Result<()> {
        Ok(())
    }
}

#[hotpath::measure]
pub async fn search(
    query: Result<Query<SearchParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_query_params(query) {
        Ok(params) => params,
        Err(e) => return error_message(&e).into_response(),
    };

    let status = match params.validate().await {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::BAD_REQUEST,
    };

    let template = SearchTemplate::new(params);

    html_response(template.render().unwrap(), status)
}
