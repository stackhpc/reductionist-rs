//! Tracing (logging)

use crate::cli::CommandLineArgs;

use opentelemetry::runtime::Tokio;
use opentelemetry::sdk::trace::Tracer;
use opentelemetry::trace::TraceError;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialise and return a Jaeger tracer.
fn init_tracer() -> Result<Tracer, TraceError> {
    opentelemetry_jaeger::new_agent_pipeline()
        .with_service_name("reductionist")
        // Avoid over-sized UDP packets with automatic batching.
        .with_auto_split_batch(true)
        .install_batch(Tokio)
}

/// Initlialise tracing (logging)
///
/// Applies a filter based on the `RUST_LOG` environment variable, falling back to enable debug
/// logging for this crate and tower_http if not set.
///
/// # Arguments
///
/// * `args`: Command line arguments.
pub fn init_tracing(args: &CommandLineArgs) {
    let tracer = init_tracer().expect("Failed to initialize tracer");
    let subscriber = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "reductionist=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer());
    if args.enable_jaeger {
        subscriber
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .init();
    } else {
        subscriber.init();
    }
}

/// Shutdown tracing (logging)
pub fn shutdown_tracing() {
    opentelemetry::global::shutdown_tracer_provider();
}
