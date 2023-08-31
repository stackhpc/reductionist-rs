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
    /// Memory limit in bytes. Default is no limit.
    #[arg(long, env = "REDUCTIONIST_MEMORY_LIMIT")]
    pub memory_limit: Option<usize>,
    /// S3 connection limit. Default is no limit.
    #[arg(long, env = "REDUCTIONIST_S3_CONNECTION_LIMIT")]
    pub s3_connection_limit: Option<usize>,
    /// Thread limit for CPU-bound tasks. Default is one less than the number of CPUs. Used only
    /// when use_rayon is false.
    #[arg(long, env = "REDUCTIONIST_THREAD_LIMIT")]
    pub thread_limit: Option<usize>,
}

/// Returns parsed command line arguments.
pub fn parse() -> CommandLineArgs {
    CommandLineArgs::parse()
}
