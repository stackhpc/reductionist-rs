# API

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

Request authentication is implemented using [Basic Auth](https://en.wikipedia.org/wiki/Basic_access_authentication) with the username and password consisting of your S3 Access Key ID and Secret Access Key, respectively.

On success, all operations return HTTP 200 OK with the response using the same datatype as specified in the request except for `count` which always returns the result as `int64`.
The server returns the following headers with the HTTP response:

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

The [scripts/client.py](https://github.com/stackhpc/reductionist-rs/blob/main/scripts/client.py) provides an example Python client and Command Line Interface (CLI).
