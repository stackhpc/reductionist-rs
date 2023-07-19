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
//! [here](https://github.com/stackhpc/reductionist).
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

pub mod app;
pub mod array;
pub mod cli;
pub mod compression;
pub mod error;
pub mod filter_pipeline;
pub mod filters;
pub mod metrics;
pub mod models;
pub mod operation;
pub mod operations;
pub mod s3_client;
pub mod server;
#[cfg(test)]
pub mod test_utils;
pub mod tracing;
pub mod types;
pub mod validated_json;
