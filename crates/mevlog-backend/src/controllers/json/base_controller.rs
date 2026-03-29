use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use serde_json::Value;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::controllers::base_controller::{DATA_FETCH_ERROR, decorate_error_message};

#[hotpath::measure]
pub async fn call_json_command<T: serde::de::DeserializeOwned>(
    cmd: &mut Command,
) -> Result<T, Value> {
    let timeout_duration = Duration::from_secs(10);

    match timeout(timeout_duration, cmd.output()).await {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);

                let friendly_error = decorate_error_message(&error_msg);

                return Err(serde_json::json!({
                    "error": friendly_error
                }));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<T>(&stdout) {
                Ok(value) => Ok(value),
                Err(e) => Err(serde_json::json!({
                    "error": format!("Failed to parse JSON: {e}")
                })),
            }
        }
        Ok(Err(e)) => Err(serde_json::json!({
            "error": e.to_string()
        })),
        Err(_) => Err(serde_json::json!({
            "error": decorate_error_message(DATA_FETCH_ERROR)
        })),
    }
}

pub async fn call_json_command_first_line<T: serde::de::DeserializeOwned>(
    cmd: &mut Command,
) -> Result<T, Value> {
    tracing::trace!("cmd: {:?}", &cmd);
    let timeout_duration = Duration::from_secs(10);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let timeout_result = timeout(timeout_duration, async {
        let mut child = cmd.spawn().map_err(|e| {
            serde_json::json!({
                "error": format!("Failed to spawn command: {e}")
            })
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            serde_json::json!({
                "error": "Failed to capture stdout"
            })
        })?;

        let mut reader = BufReader::new(stdout).lines();

        let next_line_future = hotpath::future!(reader.next_line(), log = true);

        if let Some(line) = next_line_future.await.map_err(|e| {
            serde_json::json!({
                "error": format!("Failed to read line: {e}")
            })
        })? {
            match serde_json::from_str::<T>(&line) {
                Ok(value) => Ok(value),
                Err(_) => Err(serde_json::json!({
                    "error": "No valid JSON found in output"
                })),
            }
        } else {
            Err(serde_json::json!({
                "error": "No output received from command"
            }))
        }
    })
    .await;

    match timeout_result {
        Ok(result) => result,
        Err(_) => Err(serde_json::json!({
            "error": decorate_error_message(DATA_FETCH_ERROR)
        })),
    }
}

pub fn error_json_response(e: &str) -> String {
    format!("{{\"error\": \"{e}\"}}")
}

pub fn extract_json_query_params<T>(
    query: Result<Query<T>, axum::extract::rejection::QueryRejection>,
) -> Result<T, impl IntoResponse>
where
    T: for<'de> Deserialize<'de>,
{
    match extract_query_params(query) {
        Ok(params) => Ok(params),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )),
    }
}

pub fn extract_query_params<T>(
    query: Result<Query<T>, axum::extract::rejection::QueryRejection>,
) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    match query {
        Ok(Query(params)) => Ok(params),
        Err(e) => Err(e.to_string()),
    }
}
