//! Web server

use crate::cli;

use std::{net::SocketAddr, process::exit, str::FromStr, time::Duration};

use axum::ServiceExt;
use axum_server::{tls_rustls::RustlsConfig, Handle};
use expanduser::expanduser;
use tokio::signal;

/// Serve the S3 Active Storage service
///
/// # Arguments
///
/// * `args`: Command line arguments
/// * `service`: The [crate::app::Service] to serve
pub async fn serve(args: &cli::CommandLineArgs, service: crate::app::Service) {
    let addr = SocketAddr::from_str(&format!("{}:{}", args.host, args.port))
        .expect("invalid host name, IP address or port number");

    // Catch ctrl+c and try to shutdown gracefully
    let handle = Handle::new();
    tokio::spawn(shutdown_signal(
        handle.clone(),
        args.graceful_shutdown_timeout,
    ));

    if args.https {
        // Expand files
        let abs_cert_file = expanduser(&args.cert_file)
            .expect("Failed to expand ~ to user name. Please provide an absolute path instead.")
            .canonicalize()
            .expect("failed to determine absolute path to TLS cerficate file");
        let abs_key_file = expanduser(&args.key_file)
            .expect("Failed to expand ~ to user name. Please provide an absolute path instead.")
            .canonicalize()
            .expect("failed to determine absolute path to TLS key file");
        // Check files exist
        if !abs_cert_file.exists() {
            println!(
                "TLS certificate file expected at '{}' but not found.",
                abs_cert_file.display()
            );
            exit(1)
        }
        if !abs_key_file.exists() {
            println!(
                "TLS key file expected at '{}' but not found.",
                abs_key_file.display()
            );
            exit(1)
        }
        // Set up TLS config
        let tls_config = RustlsConfig::from_pem_file(abs_cert_file, abs_key_file)
            .await
            .expect("Failed to load TLS certificate files");
        // run HTTPS server with hyper
        axum_server::bind_rustls(addr, tls_config)
            .handle(handle)
            .serve(service.into_make_service())
            .await
            .unwrap();
    } else {
        // run HTTP server with hyper
        axum_server::bind(addr)
            .handle(handle)
            .serve(service.into_make_service())
            .await
            .unwrap();
    }
}

/// Graceful shutdown handler
///
/// Installs signal handlers to catch Ctrl-C or SIGTERM and trigger a graceful shutdown.
async fn shutdown_signal(handle: Handle, timeout: u64) {
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
    handle.graceful_shutdown(Some(Duration::from_secs(timeout)));
}
