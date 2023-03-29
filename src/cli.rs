//! Command Line Interface (CLI) arguments.

use clap::Parser;

/// Reductionist command line interface
#[derive(Clone, Debug, Parser)]
pub struct CommandLineArgs {
    /// The IP address on which the proxy should listen
    #[arg(long, default_value = "0.0.0.0", env = "REDUCTIONIST_HOST")]
    pub host: String,
    /// The port to which the proxy should bind
    #[arg(long, default_value_t = 8080, env = "REDUCTIONIST_PORT")]
    pub port: u16,
    /// Flag indicating whether HTTPS should be used
    #[arg(long, default_value_t = false, env = "REDUCTIONIST_HTTPS")]
    pub https: bool,
    /// Path to the certificate file to be used for HTTPS encryption
    #[arg(
        long,
        default_value = "~/.config/reductionist/certs/cert.pem",
        env = "REDUCTIONIST_CERT_FILE"
    )]
    pub cert_file: String,
    /// Path to the key file to be used for HTTPS encryption
    #[arg(
        long,
        default_value = "~/.config/reductionist/certs/key.pem",
        env = "REDUCTIONIST_KEY_FILE"
    )]
    pub key_file: String,
    /// Maximum time in seconds to wait for operations to complete upon receiving `ctrl+c` signal.
    #[arg(long, default_value_t = 60, env = "REDUCTIONIST_SHUTDOWN_TIMEOUT")]
    pub graceful_shutdown_timeout: u64,
    /// Whether to enable sending traces to Jaeger.
    #[arg(long, default_value_t = false, env = "REDUCTIONIST_ENABLE_JAEGER")]
    pub enable_jaeger: bool,
    /// Whether to use Rayon for execution of CPU-bound tasks.
    #[arg(long, default_value_t = false, env = "REDUCTIONIST_USE_RAYON")]
    pub use_rayon: bool,
}

/// Returns parsed command line arguments.
pub fn parse() -> CommandLineArgs {
    CommandLineArgs::parse()
}
