use axum::http::StatusCode;
use axum::middleware::from_fn;
use eyre::Result;
use mevlog_backend::config::{middleware, routes::app};
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::{
    catch_panic::CatchPanicLayer, compression::CompressionLayer, timeout::TimeoutLayer,
};
use tracing::info;

#[tokio::main(flavor = "current_thread")]
#[hotpath::main]
async fn main() -> Result<()> {
    let run_handle = tokio::spawn(async { run().await });

    tokio::select! {
        res = run_handle => {
            res??;
        }

        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Ctrl-C received, shutting down…");
        }
    }

    Ok(())
}

async fn run() -> Result<()> {
    middleware::init_logs();
    hotpath::tokio_runtime!();

    let app = hotpath::future!(app(), log = true)
        .await
        .layer(from_fn(middleware::request_tracing))
        .layer(from_fn(middleware::only_ssl))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(CompressionLayer::new())
        .layer(CatchPanicLayer::new())
        .layer(from_fn(middleware::security_headers));

    let port = std::env::var("PORT").unwrap_or_else(|_| "3002".to_string());

    // Loopback only: nginx proxies to 127.0.0.1, so the app is never directly
    // reachable from the network even if the cloud firewall is misconfigured.
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| eyre::eyre!("Failed to bind 127.0.0.1:{port}: {e}"))?;

    println!("Server started at http://localhost:{}", port);
    info!("Listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
