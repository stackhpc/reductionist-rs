//! This file defines the reductionist binary entry point.

use reductionist::app;
use reductionist::cli;
use reductionist::metrics;
use reductionist::server;
use reductionist::tracing;

/// Application entry point
#[tokio::main]
async fn main() {
    let args = cli::parse();
    tracing::init_tracing(&args);
    metrics::register_metrics();
    let service = app::service();
    server::serve(&args, service).await;
    tracing::shutdown_tracing();
}
