# S3 Active Storage

This project implements simple reductions on S3 objects containing numeric binary data.
By implementing these reductions in the storage system the volume of data that needs to be
transferred to the end user is vastly reduced, leading to faster computations.

The work is funded by the
[ExCALIBUR project](https://www.metoffice.gov.uk/research/approach/collaboration/spf/excalibur)
and is done in collaboration with the
[University of Reading](http://www.reading.ac.uk/).

This is a performant implementation of the active storage server.
The original Python functional prototype is available [here](https://github.com/stackhpc/s3-active-storage).

## Concepts

The S3 active storage proxy supports the application of reductions to S3 objects that contain numeric binary data. These reductions are specified by making a HTTP post request to the active storage proxy service.

The S3 active storage proxy does not attempt to infer the datatype - it must be told the datatype to use based on knowledge that the client already has about the S3 object.

For example, if the original object has the following URL:

```
http[s]://s3.example.org/my-bucket/path/to/object
```

Then S3 active storage proxy could be used by making post requests to specfic reducer endpoints:

```
http[s]://s3-proxy.example.org/v1/{reducer}/
```

with a JSON payload of the form:

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
    "compression": "gzip|zlib"
}
```

The currently supported reducers are `max`, `min`, `mean`, `sum`, `select` and `count`. All reducers return the result using the same datatype as specified in the request except for `count` which always returns the result as `int64`.

The proxy adds two custom headers `x-activestorage-dtype` and `x-activestrorage-shape` to the HTTP response to allow the numeric result to be reconstructed from the binary content of the response. An additional `x-activestorage-count` header is also returned which contains the number of array elements operated on while performing the requested reduction. This header is useful, for example, to calculate the mean over multiple requests where the number of items operated on may differ between chunks.

[//]: <> (TODO: No OpenAPI support yet).
[//]: <> (For a running instance of the proxy server, the full OpenAPI specification is browsable as a web page at the `{proxy-address}/redoc/` endpoint or in raw JSON form at `{proxy-address}/openapi.json`.)

## Caveats

This is a very early-stage project, and as such supports limited functionality.

In particular, the following are known limitations which we intend to address:

  * Error handling and reporting is minimal
  * No support for missing data
  * No support for encrypted objects

## Running

There are various ways to run the S3 Active Storage server.

### Running in a container

The simplest method is to run it in a container using a pre-built image:

```sh
docker run -it --detach --rm --net=host --name s3-active-storage ghcr.io/stackhpc/s3-active-storage-rs:latest
```

Images are published to [GitHub Container Registry](https://github.com/stackhpc/s3-active-storage-rs/pkgs/container/s3-active-storage-rs) when the project is released.
The `latest` tag corresponds to the most recent release, or you can use a specific release e.g. `0.1.0`.

This method does not require access to the source code.

### Building a container image

If you need to use unreleased changes, but still want to run in a container, it is possible to build an image.
First, clone this repository:

```sh
git clone https://github.com/stackhpc/s3-active-storage-rs.git
cd s3-active-storage-rs
```

```sh
make build
```

The image will be tagged as `s3-active-storage`.
The image may be pushed to a registry, or deployed locally.

```sh
make run
```

## Build

If you prefer not to run the S3 Active Storage server in a container, it will be necessary to build a binary.
Building locally may also be preferable during development to take advantage of incremental compilation.

### Prerequisites

This project is written in Rust, and as such requires a Rust toolchain to be installed in order to build it.
The Minimum Supported Rust Version (MSRV) is 1.66.1, due to a dependency on the [AWS SDK](https://github.com/awslabs/aws-sdk-rust).
It may be necessary to use [rustup](https://rustup.rs/) rather than the OS provided Rust toolchain to meet this requirement.
See the [Rust book](https://doc.rust-lang.org/book/ch01-01-installation.html) for toolchain installation.

### Build and run S3 active storage

First, clone this repository:

```sh
git clone https://github.com/stackhpc/s3-active-storage-rs.git
cd s3-active-storage-rs
```

Next, use Cargo to build the package:

```sh
cargo build --release
```

The active storage server may be run using Cargo:

```sh
cargo run --release
```

Or installed to the system:

```sh
cargo install --path . --locked
```

Then run:

```sh
s3-active-storage
```

## Testing

For simple testing purposes Minio is a convenient object storage server.

### Deploy Minio object storage

Start a local [Minio](https://min.io/) server which serves the test data:

```sh
./scripts/minio-start
```

The Minio server will run in a detached container and may be stopped:

```sh
./scripts/minio-stop
```

Note that object data is not preserved when the container is stopped.

### Upload some test data

A script is provided to upload some test data to minio.
In a separate terminal, set up the Python virtualenv then upload some sample data:

```sh
# Create a virtualenv
python3 -m venv ./venv
# Activate the virtualenv
source ./venv/bin/activate
# Install dependencies
pip install scripts/requirements.txt
# Upload some sample data to the running minio server
python ./scripts/upload_sample_data.py
```

### Compliance test suite

Proxy functionality can be tested using the [S3 active storage compliance suite](https://github.com/stackhpc/s3-active-storage-compliance-suite).

### Making requests to active storage endpoints

Request authentication is implemented using [Basic Auth](https://en.wikipedia.org/wiki/Basic_access_authentication) with the username and password consisting of your S3 Access Key ID and Secret Access Key, respectively. These credentials are then used internally to authenticate with the upstream S3 source using [standard AWS authentication methods](https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-auth-using-authorization-header.html)

A basic Python client is provided in `scripts/client.py`.
First install dependencies in a Python virtual environment:

```sh
# Create a virtualenv
python3 -m venv ./venv
# Activate the virtualenv
source ./venv/bin/activate
# Install dependencies
pip install scripts/requirements.txt
```

Then use the client to make a request:
```sh
venv/bin/python ./scripts/client.py sum --server http://localhost:8080 --source http://localhost:9000 --username minioadmin --password minioadmin --bucket sample-data --object data-uint32.dat --dtype uint32
```

---

## Documentation

The source code is documented using [rustdoc](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html).
Currently the `s3-active-storage` crate is not uploaded to https://crates.io, so we do not benefit from hosted documentation on https://docs.rs.
It is however possible to build the documentation locally:

```sh
cargo doc
```

Cargo builds documentation for the `s3-active-storage` crate and all of its dependencies.
The resulting documentation is available under `target/doc`, and may be viewed in a web browser using file:///path/to/s3-active-storage/target/doc/s3-active-storage/index.html.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for information about contributing to S3 active storage.

## License

This project is licensed under the Apache-2.0 License.
