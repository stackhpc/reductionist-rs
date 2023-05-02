//! Command Line Interface (CLI) arguments.

use clap::Parser;

/// S3 Active Storage Proxy command line interface
#[derive(Debug, Parser)]
pub struct CommandLineArgs {
    /// The IP address on which the proxy should listen
    #[arg(long, default_value = "0.0.0.0", env = "S3_ACTIVE_STORAGE_HOST")]
    pub host: String,
    /// The port to which the proxy should bind
    #[arg(long, default_value_t = 8080, env = "S3_ACTIVE_STORAGE_PORT")]
    pub port: u16,
    /// Flag indicating whether HTTPS should be used
    #[arg(long, default_value_t = false, env = "S3_ACTIVE_STORAGE_HTTPS")]
    pub https: bool,
    /// Path to the certificate file to be used for HTTPS encryption
    #[arg(
        long,
        default_value = "~/.config/s3-active-storage/certs/cert.pem",
        env = "S3_ACTIVE_STORAGE_CERT_FILE"
    )]
    pub cert_file: String,
    /// Path to the key file to be used for HTTPS encryption
    #[arg(
        long,
        default_value = "~/.config/s3-active-storage/certs/key.pem",
        env = "S3_ACTIVE_STORAGE_KEY_FILE"
    )]
    pub key_file: String,
    /// Maximum time in seconds to wait for operations to complete upon receiving `ctrl+c` signal.
    #[arg(long, default_value_t = 60, env = "S3_ACTIVE_STORAGE_SHUTDOWN_TIMEOUT")]
    pub graceful_shutdown_timeout: u64,
}

/// Returns parsed command line arguments.
pub fn parse() -> CommandLineArgs {
    CommandLineArgs::parse()
}
