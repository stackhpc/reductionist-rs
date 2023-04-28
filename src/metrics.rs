use axum::{body::Body, http::Request, response::Response};
use lazy_static::lazy_static;
use prometheus::{self, Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};
use tracing::Span;

lazy_static! {
    // Registry for holding metric state
    pub static ref REGISTRY: Registry = Registry::new();
    // Simple request counter
    pub static ref INCOMING_REQUESTS: IntCounterVec = IntCounterVec::new(
        Opts::new("incoming_requests", "The number of HTTP requests received"),
        &["http_method"]
    ).unwrap();
    // Request counter by status code
    pub static ref RESPONSE_CODE_COLLECTOR: IntCounterVec = IntCounterVec::new(
        Opts::new("outgoing_response", "The number of responses sent."),
        &["status_code"]
    ).unwrap();
    // Request histogram by response time
    pub static ref RESPONSE_TIME_COLLECTOR: HistogramVec = HistogramVec::new(
        HistogramOpts{
            common_opts: Opts::new("response_time", "The time taken to respond to each request"),
            buckets: prometheus::DEFAULT_BUCKETS.to_vec(), // Change buckets here if desired
        },
        &[],
    ).unwrap();
}

pub fn register_metrics() {
    REGISTRY
        .register(Box::new(INCOMING_REQUESTS.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(RESPONSE_CODE_COLLECTOR.clone()))
        .unwrap();
    REGISTRY
        .register(Box::new(RESPONSE_TIME_COLLECTOR.clone()))
        .unwrap();
}

pub async fn metrics_handler() -> String {
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();

    encoder.encode(&REGISTRY.gather(), &mut buffer).unwrap();

    let output = String::from_utf8(buffer.clone()).unwrap();
    buffer.clear();

    output
}

/// Increments the prometheus counter on all incoming requests, labelled by http method
pub fn request_counter(request: &Request<Body>, _span: &Span) {
    INCOMING_REQUESTS
        .with_label_values(&[&request.method().to_string().to_ascii_uppercase()])
        .inc();
}

/// Increment the prometheus counter on all outgoing responses, labelled by status code
pub fn record_response_metrics<B>(
    response: &Response<B>,
    latency: std::time::Duration,
    _span: &Span,
) {
    RESPONSE_CODE_COLLECTOR
        .with_label_values(&[response.status().as_str()])
        .inc();

    RESPONSE_TIME_COLLECTOR
        .with_label_values(&[])
        .observe(latency.as_secs_f64());
}
