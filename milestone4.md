# ExCALIWork Active Object Storage Milestone Report - WP2.1-4 Testing

Mark Goddard and Scott Davidson

22nd September 2023

This section describes ExCALIWork Work Package WP2.1-4 (Testing), and covers [Reductionist](https://github.com/stackhpc/reductionist-rs), the Active Object Storage Server as of version 0.8.0.
The following topics are covered:


1. Reductionist overview
1. Reductionist API
1. Architecture and implementation of Reductionist
1. Deployment of Reductionist
1. Testing of Reductionist
1. Integration of Reductionist and PyActiveStorage

Note: The original S3 Active Storage project was renamed to Reductionist, to avoid confusion due to overuse of the term Active Storage.

## Reductionist overview

Reductionist is a web service that provides an API to perform reductions on numerical data stored in an S3-compatible object store.
It provides the following features:

* HTTP(S) API with JSON request data
* Access to data stored in S3-compatible storage
* Basic numerical operations on multi-dimensional arrays (count, min, max, select, sum)
* Perform calculations on a selection/slice of an array
* Perform calculations allowing for missing data
* Compressed data (GZip, Zlib)
* Filtered data (byte shuffle)
* Data with non-native byte order (endianness)
* Server resource (CPU, memory, files) management
* [Prometheus](https://prometheus.io/) metrics
* Tracing with an option to send data to [Jaeger](https://www.jaegertracing.io/)

## Reductionist API

The Reductionist API accepts HTTP POST requests to `/v1/{operation}`, where `{operation}` is the name of the operation to perform, one of `count`, `min`, `max`, `sum` or `select`.
The request body should be a JSON object of the form:

```
{
    // The URL for the S3 source
    // - required
    "source": "https://s3.example.com/,

    // The name of the S3 bucket
    // - required
    "bucket": "my-bucket",

    // The path to the object within the bucket
    // - required
    "object": "path/to/object",

    // The data type to use when interpreting binary data
    // - required
    "dtype": "int32|int64|uint32|uint64|float32|float64",

    // The byte order (endianness) of the data
    // - optional, defaults to native byte order of Reductionist server
    "byte_order": "big|little",

    // The offset in bytes to use when reading data
    // - optional, defaults to zero
    "offset": 0,

    // The number of bytes to read
    // - optional, defaults to the size of the entire object
    "size": 128,

    // The shape of the data (i.e. the size of each dimension)
    // - optional, defaults to a simple 1D array
    "shape": [20, 5],

    // Indicates whether the data is in C order (row major)
    // or Fortran order (column major, indicated by 'F')
    // - optional, defaults to 'C'
    "order": "C|F",

    // An array of [start, end, stride] tuples indicating the data to be operated on
    // (if given, you must supply one tuple per element of "shape")
    // - optional, defaults to the whole array
    "selection": [
        [0, 19, 2],
        [1, 3, 1]
    ],

    // Algorithm used to compress the data
    // - optional, defaults to no compression
    "compression": {"id": "gzip|zlib"},

    // List of algorithms used to filter the data
    // - optional, defaults to no filters
    "filters": [{"id": "shuffle", "element_size": 4}],

    // Missing data description
    // - optional, defaults to no missing data
    // - exactly one of the keys below should be specified
    // - the values should match the data type (dtype)
    "missing": {
        "missing_value": 42,
        "missing_values": [42, -42],
        "valid_min": 42,
        "valid_max": 42,
        "valid_range": [-42, 42],
    }
}
```

The currently supported operations are `max`, `min`, `sum`, `select` and `count`.

Request authentication is implemented using [Basic Auth](https://en.wikipedia.org/wiki/Basic_access_authentication) with the username and password consisting of your S3 Access Key ID and Secret Access Key, respectively.

On success, all operations return HTTP 200 OK with the response using the same datatype as specified in the request except for `count` which always returns the result as `int64`.
The proxy returns the following headers to the HTTP response:

* `x-activestorage-dtype`: The data type of the data in the response payload. One of `int32`, `int64`, `uint32`, `uint64`, `float32` or `float64`.
* `x-activestorage-byte-order`: The byte order of the data in the response payload. Either `big` or `little`.
* `x-activestorage-shape`: A JSON-encoded list of numbers describing the shape of the data in the response payload. May be an empty list for a scalar result.
* `x-activestorage-count`: The number of non-missing array elements operated on while performing the requested reduction. This header is useful, for example, to calculate the mean over multiple requests where the number of items operated on may differ between chunks.

On error, an HTTP 4XX (client) or 5XX (server) response code will be returned, with the response body being a JSON object of the following format:

```
{
    "error": {
        // Main error message
        "message": "error receiving object from S3 storage",

        // Optional list of lower-level errors, with the root cause last
        "caused_by": [
            "IO error",
            "unexpected end of file"
        ]
    }
}

```

## Architecture and implementation of Reductionist

Reductionist is written in [Rust](https://www.rust-lang.org/), a language that is rapidly gaining popularity for a variety of use cases.
It provides high level abstractions with low runtime overhead, a modern toolchain, and has a unique approach that provides safe automatic memory management without garbage collection.
While the Rust standard library is not as comprehensive as some other "batteries included" languages, the [crates.io](https://crates.io/) ecosystem is relatively mature and provides a number of de-facto standard libraries.
Reductionist is built on top of a number of popular open source components.

A few properties make it relatively easy to build a conceptual mental model of how Reductionist works.

* All operations share the same request processing pipeline.
* The request processing pipeline for each request is a fairly linear sequence of steps.
* There is no persistent state.
* The only external service that is interacted with is an S3-compatible object store.

The more challenging aspects of the system are the lower level details of asynchronous programming, memory management, the Rust type system and working with multi-dimensional arrays.

A diagram of the request processing pipeline is shown in figure 1.

TODO: request processing pipeline diagram

### Axum web server

[Axum](https://docs.rs/axum) is an asynchronous web framework that performs well in [various benchmarks](https://github.com/programatik29/rust-web-benchmarks/blob/master/result/hello-world.md) and is built on top of various popular components, including the [hyper](https://hyper.rs/) HTTP library.
It integrates well with [Tokio](https://tokio.rs/), the most popular asynchronous Rust runtime, and allows us to easily define an API route for each operation.
[Extractors](https://docs.rs/axum/latest/axum/extract/index.html) make it easy to consume data from the request in a type-safe way.
The operation request handler is the `operation_handler` function in `src/app.rs`.

### API request data

The JSON request data is deserialised into the `RequestData` struct defined in `src/models.rs` using the [serde](https://serde.rs/) library.
Serde handles conversion errors at the type level, while further validation of request data invariants is performed using the [validator](https://crates.io/crates/validator) crate.

### S3 object download

Object data is downloaded from the object store using the [AWS SDK](https://docs.aws.amazon.com/sdk-for-rust/).
The `S3Client` struct in `src/s3_client.rs` provides a simplified wrapper around the AWS SDK.
Typically we will be operating on a "storage chunk", a hyperslab within the larger dataset that the object contains.
In this case a byte range is specified in the S3 `GetObject` request to avoid downloading the whole object.
The AWS SDK is asynchronous and does provide a streaming response, however we read the whole storage chunk into memory to simplify later stages of the pipeline.
Storage chunks are expected to be small enough (O(MiB)) that this should not be a problem.

Construction of [aws_sdk_s3::Client](https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/client/struct.Client.html) structs is a relatively slow task.
A key performance improvement involves the use of a shared client object for each combination of object store URL and credentials.
This is implemented using the `S3ClientMap` in `src/s3_client.rs` and benchmarked in `benches/s3_client.rs`.

Downloaded storage chunk data is returned to the request handler as a [Bytes](https://docs.rs/bytes/latest/bytes/struct.Bytes.html) object, which is a wrapper around a `u8` (byte) array.

### Filters and compression

When a variable in a netCDF, HDF5 or Zarr dataset is created, it may be compressed to reduce storage requirements.
Additionally, prior to compression one or more filters may be applied to the data with the aim of increasing the compression ratio.
When consuming such data, Reductionist needs to reverse any compression and filters applied.
The filter pipeline is implemented in `src/filter_pipeline.rs`.

First, if a compression algorithm is specified in the request data, the storage chunk is decompressed using the same algorithm.
Currently the Gzip and Zlib algorithms are supported using the [flate2](https://docs.rs/flate2) and [zune-inflate](https://docs.rs/zune-inflate) libraries respectively.
This mix of libraries was chosen based on performance benchmarks in `benches/compression.rs`.
Compression is implemented in `src/compression.rs`.

Next, if any filters are specified in the request data, they are decoded in reverse order.
Currently the byte shuffle filter is supported.
This filter reorders the data to place the Nth bytes of each data value together, with the aim of grouping leading zeroes.
The shuffle filter is implemented in `src/filters/shuffle.rs`, and has several optimisations including loop unrolling that were benchmarked using `benches/shuffle.rs`.

### The Operation trait

Here the implementation becomes specific to the requested operation (min, max, etc.).
This is achieved using the `Operation` trait defined in `src/operation.rs`.

```rust
/// Trait for active storage operations.
///
/// This forms the contract between the API layer and operations.
pub trait Operation {
    /// Execute the operation.
    ///
    /// Returns a [models::Response](crate::models::Response) object with response data.
    ///
    /// # Arguments
    ///
    /// * `request_data`: RequestData object for the request
    /// * `data`: [`Vec<u8>`] containing data to operate on.
    fn execute(
        request_data: &models::RequestData,
        data: Vec<u8>,
    ) -> Result<models::Response, ActiveStorageError>;
}
```

This interface accepts the request data and a byte array containing the storage chunk data in its original byte order.
On success, it returns a `Response` struct which contains a byte array of the response data as well as the data type, shape and a count of non-missing elements in the array.

A second `NumOperation` trait with an `execute_t` method handles the dynamic dispatch between the runtime data type in the request data and the generic implementation for that type.

### Operations

Each operation is implemented by a struct that implements the `NumOperation` trait.
For example, the sum operation is implemented by the `Sum` struct in `src/operations.rs`.
The `Sum` struct's `execute_t` method does the following:

* Zero copy conversion of the byte array to a multi-dimensional [ndarray::ArrayView](https://docs.rs/ndarray/latest/ndarray/type.ArrayView.html) object of the data type, shape and byte order specified in the request data
* If a selection was specified in the request data, create a sliced `ndarray::ArrayView` onto the original array view
* If missing data was specified in the request data:
  * Create an iterator over the array view that filters out missing data, performs the sum operation and counts non-missing elements
* Otherwise:
  * Use the array view's native `sum` and `len` methods to take the sum and element count
* Convert the sum to a byte array and return with the element count

The procedure for other operations varies slightly but generally follows the same pattern.

### Error handling

The `ActiveStorageError` enum in `src/error.rs` describes the various errors that may be returned by the Reductionist API, as well as how to format them for the JSON error response body.
Low-level errors are converted to higher-level errors and ultimately wrapped by `ActiveStorageError`.
This is a common pattern in Rust and allows us to describe all of the errors that a function or application may return.

### Configuration

Reductionist configuration is implemented in `src/cli.rs` using the [clap](https://docs.rs/clap) library, and accepts command line arguments and environment variables.

### Resource management

Reductionist supports optional restriction of resource usage.
This is implemented in `src/resource_manager.rs` using [Tokio Semaphores](https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html).
This allows Reductionist to limit the quantity of various resources used at any time:

* S3 connections
* memory used for numeric data (this is more of a rough guide than a perfect limit)
* threads used for CPU-bound work

### CPU-bound work

There is particular friction between the asynchronous and synchronous types of work in the system.
Axum and Tokio very efficiently handle the asynchronous aspects such as the HTTP server and S3 object download.
The other work such as decompression, filtering and numerical operations are more CPU-bound, and can easily block the Tokio runtime from efficiently handling asynchronous tasks.
Two alternative methods were developed to alleviate this issue.

1. The resource manager can limit the number of threads used for CPU-bound work, by default leaving one CPU core free for handling asynchronous tasks.
2. Integration with [Rayon](https://docs.rs/rayon), a library that provides a thread pool.

Limited benchmarking was done to compare the two approaches, however the first appeared to have lower overhead.
The second approach may leave the server more responsive if more CPU-heavy operations are used in future.

### Monitoring

Prometheus metrics are implemented in `src/metrics.rs` and are exposed by the Reductionist API under the `/metrics` path.
These include:

* incoming requests (counter)
* outgoing response (counter)
* response time (histogram)

### Tracing and profiling

Reductionist integrates with Jaeger, a distributed tracing platform.
Various sections of the request processing pipeline are instrumented with spans, making it easy to visualise the relative durations in the Jaeger UI.
Testing with a sum over some CMIP6 temperature data, this showed that in terms of wall clock time, the S3 storage chunk download takes the majority of the time, followed by decompression, byte shuffle, and finally the actual numerical operation.

Flame graphs created using [flamegraph-rs](https://docs.rs/flamegraph/) were useful to visualise which parts of the code consume the most CPU cycles.
This was useful to determine where to focus performance improvements, and showed that decompression is the most CPU-heavy task.

## Deployment of Reductionist

An Ansible playbook in the `deployment` directory can be used to deploy Reductionist and supporting services to one or more hosts.
The following services are supported:

* Docker engine
* Step CA Certificate Authority (generates certificates for Reductionist)
* Step CLI (requests and renews certificates)
* Minio object store (optional, for testing)
* Prometheus (monitors Reductionist and HAProxy)
* Jaeger (distributed tracing UI)
* Reductionist
* HAProxy (load balancer for Reductionist)

The playbook allows for a secure, scale-out deployment of Reductionist, with an HAProxy load balancer proxying requests to any number of Reductionist backend servers.
Usage instructions are provided in `deployment/README.md`.

## Testing of Reductionist

A majority of the application code in Reductionist is unit tested.
Unit tests in Rust code typically reside in the same file as the module being tested.
Code style and lint checks are applied using a Git pre-commit hook.

The [S3 Active Storage compliance test suite](https://github.com/stackhpc/s3-active-storage-compliance-suite/) developed for WP2.1-2 has been updated to test new features in Reductionist.

Benchmark tests in the `benches` directory were created for various modules and used to make performance improvements.

A GitHub Actions workflow runs several jobs against pull requests that perform code linting, unit testing, compliance testing, Ansible deployment, documentation builds, and more.

## Integration of Reductionist and PyActiveStorage

Reductionist has been integrated with the PyActiveStorage library, and acts as a client of the Reductionist server.
PyActiveStorage currently works with data in netCDF4 format, and is able to perform reductions on a variable within such a dataset.
Numerical operations are performed on individual storage chunks, with the results later aggregated.
The original POSIX/NumPy storage chunk reduction in PyActiveStorage is implementated in a `reduce_chunk` Python function in `activestorage/storage.py`, and this interface was used as the basis for the integration of Reductionist.
The following code snippet shows the `reduce_chunk` function signature.

```python
def reduce_chunk(rfile, offset, size, compression, filters, missing, dtype, shape, order, chunk_selection, method=None):
    """ We do our own read of chunks and decoding etc 
    
    rfile - the actual file with the data 
    offset, size - where and what we want ...
    compression - optional `numcodecs.abc.Codec` compression codec
    filters - optional list of `numcodecs.abc.Codec` filter codecs
    dtype - likely float32 in most cases. 
    shape - will be a tuple, something like (3,3,1), this is the dimensionality of the 
            chunk itself
    order - typically 'C' for c-type ordering
    chunk_selection - python slice tuples for each dimension, e.g.
                        (slice(0, 2, 1), slice(1, 3, 1), slice(0, 1, 1))
                        this defines the part of the chunk which is to be obtained
                        or operated upon.
    method - computation desired 
            (in this Python version it's an actual method, in 
            storage implementations we'll change to controlled vocabulary)
                    
    """
```

For Reductionist, the `reduce_chunk` function signature in `activestorage/reductionist.py` is similar, but replaces the local file path with a `requests.Session` object, the Reductionist server URL, S3-compatible object store URL, and the bucket and object containing the data.

```python
def reduce_chunk(session, server, source, bucket, object,
                 offset, size, compression, filters, missing, dtype, shape,
                 order, chunk_selection, operation):
    """Perform a reduction on a chunk using Reductionist.

    :param session: requests.Session object
    :param server: Reductionist server URL
    :param source: S3 URL
    :param bucket: S3 bucket
    :param object: S3 object
    :param offset: offset of data in object
    :param size: size of data in object
    :param compression: optional `numcodecs.abc.Codec` compression codec
    :param filters: optional list of `numcodecs.abc.Codec` filter codecs
    :param missing: optional 4-tuple describing missing data
    :param dtype: numpy data type
    :param shape: will be a tuple, something like (3,3,1), this is the
                  dimensionality of the chunk itself
    :param order: typically 'C' for c-type ordering
    :param chunk_selection: N-tuple where N is the length of `shape`, and each
                            item is an integer or slice.  e.g.  (slice(0, 2,
                            1), slice(1, 3, 1), slice(0, 1, 1))
                            this defines the part of the chunk which is to be
                            obtained or operated upon.
    :param operation: name of operation to perform
    :returns: the reduced data as a numpy array or scalar
    :raises ReductionistError: if the request to Reductionist fails
    """
```

Within the `reduce_chunk` implementation for Reductionist, the following steps are taken:

* build Reductionist API request data
* build Reductionist API URL
* perform an HTTP(S) POST request to Reductionist
* on success, return a NumPy array containing the data in the response payload, with data type, shape and count determined by response headers
* on failure, raise a `ReductionistError` with the response status code and JSON encoded error response

The use of a `requests.Session` object allows for TCP connection pooling, reducing connection overhead when multiple requests are made within a short timeframe.

It should be possible to provide a unified interface to storage systems by abstracting away the details of the storage system and data source, but this has not yet been done.

Other changes to the main `activestorage.Active` class were necessary for integration of Reductionist.
These include:

* Support for reading netCDF metadata from files stored in S3 using the [s3fs](https://s3fs.readthedocs.io/) and [h5netcdf](https://pypi.org/project/h5netcdf/) libraries
* Configuration options in `activestorage/config.py` to specify the Reductionist API URL, S3-compatible object store URL, S3 access key, secret key and bucket
* Constructor `storage_type` argument for `activestorage.Active` to specify the storage backend
* Use of a thread pool to execute storage chunk reductions in parallel
* Unit tests to cover new and modified code
* Integration test changes to allow running against a POSIX or S3 storage backend
