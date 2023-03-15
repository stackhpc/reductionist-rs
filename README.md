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
    ]
}
```

The currently supported reducers are `max`, `min`, `mean`, `sum`, `select` and `count`. All reducers return the result using the same datatype as specified in the request except for `count` which always returns the result as `int64`.

[//]: <> (TODO: No OpenAPI support yet).
[//]: <> (For a running instance of the proxy server, the full OpenAPI specification is browsable as a web page at the `{proxy-address}/redoc/` endpoint or in raw JSON form at `{proxy-address}/openapi.json`.)

## Caveats

This is a very early-stage project, and as such supports limited functionality.

In particular, the following are known limitations which we intend to address:

  * Error handling and reporting is minimal
  * No support for missing data
  * No support for compressed or encrypted objects

## Build

### Prerequisites

This project is written in Rust, and as such requires a Rust toolchain to be installed in order to build it.
The Minimum Supported Rust Version (MSRV) is 1.62.1, due to a dependency on the [AWS SDK](https://github.com/awslabs/aws-sdk-rust).
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
cargo build
```

The active storage server may be run using Cargo:

```sh
cargo run
```

Or installed to the system:

```sh
cargo install
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
chmod +x ./scripts/minio-run
./scripts/minio-run
```

The Minio server will run until it is stopped using `Ctrl+C`.

### Upload some test data

A script is provided to upload some test data to minio.
In a separate terminal, set up the Python virtualenv then upload some sample data:

```sh
# Create a virtualenv
python -m venv ./venv
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

A request to an active storage proxy running on localhost with Minio as the S3 source is as simple as:

```python
import json
import numpy as np
import requests

request_data = {
  'source': 'http://localhost:9000',
  'bucket': 'sample-data',
  'object': 'data-float32.dat',
  'dtype': 'float32',
  # All other fields assume their default values
}

reducer = 'sum'
response = requests.post(
  f'http://localhost:8000/v1/{reducer}',
  json=request_data, 
  auth=('minioadmin', 'minioadmin')
)
shape = json.loads(response.headers['x-activestorage-shape'])
sum_result = np.frombuffer(response.content, dtype=response.headers['x-activestorage-dtype'])
sum_result = sum_result.reshape(shape)
```

The proxy adds two custom headers `x-activestorage-dtype` and `x-activestrorage-shape` to the HTTP response to allow the numeric result to be reconstructed from the binary content of the response.

---

## A note on row-major ('C') vs column-major ('F') ordering

Since we use `ndarray` to implement all array options, it is simplest to perform all internal operations using C ordering. To accomplish this, if the incoming requests specifies that the source data is Fortran-ordered (via `order = 'F'` in the request body) then the data bytes are first read from the S3 source into an array of the correct shape before transposing this array to convert from 'F' to 'C' ordering. Once the data reduction is complete, the result is then converted back to raw bytes using the same ordering convention as specified in the incoming request. This ensures that all internal numpy operations are performed efficiently while also returning the raw response bytes in the order requested.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for information about contributing to S3 active storage.

## License

This project is licensed under the Apache-2.0 License.
