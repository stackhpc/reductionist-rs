//! This file defines the s3-active-storage binary entry point.

use s3_active_storage::app;
use s3_active_storage::cli;
use s3_active_storage::metrics;
use s3_active_storage::server;
use s3_active_storage::tracing;

/// Application entry point
#[tokio::main]
async fn main() {
    let args = cli::parse();
    tracing::init_tracing();
    metrics::register_metrics();
    let service = app::service();
    server::serve(&args, service).await;
}
