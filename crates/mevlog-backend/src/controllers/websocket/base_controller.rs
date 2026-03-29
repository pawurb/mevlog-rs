use axum::extract::ws::{Message, WebSocket};
use futures::{
    sink::SinkExt,
    stream::{SplitSink, StreamExt},
};
use std::time::{Duration, Instant};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{ChildStderr, ChildStdout, Command},
};
use tokio_stream::wrappers::LinesStream;

use crate::controllers::base_controller::{DATA_FETCH_ERROR, decorate_error_message};

#[hotpath::measure]
pub fn cmd_output_stream(
    cmd: &mut Command,
) -> (
    LinesStream<BufReader<ChildStdout>>,
    LinesStream<BufReader<ChildStderr>>,
) {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_reader = tokio::io::BufReader::new(stdout).lines();
    let stderr_reader = tokio::io::BufReader::new(stderr).lines();

    let stdout_lines = tokio_stream::wrappers::LinesStream::new(stdout_reader);
    let stderr_lines = tokio_stream::wrappers::LinesStream::new(stderr_reader);

    (stdout_lines, stderr_lines)
}

#[hotpath::measure]
pub async fn stream_output_lines(
    mut stdout_lines: LinesStream<BufReader<ChildStdout>>,
    mut stderr_lines: LinesStream<BufReader<ChildStderr>>,
    mut sender: SplitSink<WebSocket, Message>,
) {
    let start_time = Instant::now();
    let timeout_duration = Duration::from_secs(10);
    loop {
        // Check if we've exceeded the timeout
        if start_time.elapsed() > timeout_duration {
            let timeout_error = serde_json::json!({
                "error": DATA_FETCH_ERROR
            })
            .to_string();

            if sender
                .send(Message::Text(timeout_error.into()))
                .await
                .is_err()
            {
                tracing::error!("Failed to send timeout message to client, disconnecting");
            }
            break;
        }

        tokio::select! {
            Some(line) = stdout_lines.next() => {
                if let Ok(line) = line
                    && sender.send(Message::Text(line.into())).await.is_err() {
                        tracing::error!("Failed to send message to client, disconnecting");
                        break;
                    }
            }
            Some(line) = stderr_lines.next() => {
                if let Ok(line) = line {
                    let friendly_error = decorate_error_message(&line);

                    if sender.send(Message::Text(friendly_error.into()))
                        .await
                        .is_err()
                    {
                        tracing::error!("Failed to send error message to client, disconnecting");
                        break;
                    }
                }
            }
            else => break
        }
    }
}
