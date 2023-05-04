//! Tracing (logging)

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initlialise tracing (logging)
///
/// Applies a filter based on the `RUST_LOG` environment variable, falling back to enable debug
/// logging for this crate and tower_http if not set.
pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "s3_active_storage=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}
