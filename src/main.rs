//! This crate provides an Active Storage Server. It implements simple reductions on S3 objects
//! containing numeric binary data.  By implementing these reductions in the storage system the
//! volume of data that needs to be transferred to the end user is vastly reduced, leading to
//! faster computations.
//!
//! The work is funded by the
//! [ExCALIBUR project](https://www.metoffice.gov.uk/research/approach/collaboration/spf/excalibur)
//! and is done in collaboration with the
//! [University of Reading](http://www.reading.ac.uk/).
//!
//! This is a performant implementation of the Active Storage Server.
//! The original Python functional prototype is available
//! [here](https://github.com/stackhpc/s3-active-storage).
//!
//! The Active Storage Server is built on top of a number of open source components.
//!
//! * [Tokio](tokio), the most popular asynchronous Rust runtime.
//! * [Axum](axum) web framework, built by the Tokio team. Axum performs well in [various](https://github.com/programatik29/rust-web-benchmarks/blob/master/result/hello-world.md) [benchmarks](https://web-frameworks-benchmark.netlify.app/result?l=rust)
//!   and is built on top of various popular components, including the [hyper] HTTP library.
//! * [Serde](serde) performs (de)serialisation of JSON request and response data.
//! * [AWS SDK for S3](aws-sdk-s3) is used to interact with S3-compatible object stores.
//! * [ndarray] provides [NumPy](https://numpy.orgq)-like n-dimensional arrays used in numerical
//!   computation.

use std::{net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

use axum_server::{tls_rustls::RustlsConfig, Handle};
use clap::Parser;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod array;
mod error;
mod models;
mod operation;
mod operations;
mod s3_client;
mod validated_json;

/// S3 Active Storage Proxy command line interface
#[derive(Debug, Parser)]
struct CommandLineArgs {
    /// The IP address on which the proxy should listen
    #[arg(long, default_value = "0.0.0.0", env = "S3_ACTIVE_STORAGE_HOST")]
    host: String,
    /// The port to which the proxy should bind
    #[arg(long, default_value_t = 8080, env = "S3_ACTIVE_STORAGE_PORT")]
    port: u16,
    /// Flag indicating whether HTTPS should be used
    #[arg(long, default_value_t = false, env = "S3_ACTIVE_STORAGE_HTTPS")]
    https: bool,
    /// Path to the certificate file to be used for HTTPS encryption
    #[arg(
        long,
        default_value = ".certs/cert.pem",
        env = "S3_ACTIVE_STORAGE_CERT_FILE"
    )]
    cert_file: PathBuf,
    /// Path to the key file to be used for HTTPS encryption
    #[arg(
        long,
        default_value = ".certs/key.pem",
        env = "S3_ACTIVE_STORAGE_KEY_FILE"
    )]
    key_file: PathBuf,
}

/// Application entry point
#[tokio::main]
async fn main() {
    let args = CommandLineArgs::parse();

    // Make use of command line args
    let addr = SocketAddr::from_str(&format!("{}:{}", args.host, args.port)).unwrap();
    let cert_path = args.cert_file.canonicalize().unwrap();
    let key_path = args.key_file.canonicalize().unwrap();
    let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .unwrap();

    init_tracing();

    let router = app::router();

    // Catch ctrl+c and try to shutdown gracefully
    let handle = Handle::new();
    tokio::spawn(shutdown_signal_https(handle.clone()));

    if args.https {
        // run HTTPS server with hyper
        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(router.into_make_service())
            .await
            .unwrap();
    } else {
        // run HTTP server with hyper
        axum_server::bind(addr)
            .handle(handle)
            .serve(router.into_make_service())
            .await
            .unwrap();
    }
}

/// Initlialise tracing (logging)
///
/// Applies a filter based on the `RUST_LOG` environment variable, falling back to enable debug
/// logging for this crate and tower_http if not set.
fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "s3_active_storage=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Graceful shutdown handler
///
/// Installs signal handlers to catch Ctrl-C or SIGTERM and trigger a graceful shutdown.
async fn shutdown_signal_https(handle: Handle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("signal received, starting graceful shutdown");
    // Force shutdown if graceful shutdown takes longer than 10s
    handle.graceful_shutdown(Some(Duration::from_secs(10)));
}
