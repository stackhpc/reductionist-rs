# API

The Reductionist API accepts HTTP POST requests to `/v2/{operation}`, where `{operation}` is the name of the operation to perform, one of `count`, `min`, `max`, `sum` or `select`.
The request body should be a JSON object of the form:

```
{
    // The object store backend to be used
    // - required
    "interface_type": "s3|http|https",

    // The URL for the S3 or HTTP object
    // - required
    "url": "https://s3.example.com/bucket/object",

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

    // The axis or axes over which to perform the reduction operation
    // - optional, can be either a single axis or list of axes, defaults
    // to a reduction over all axes
    "axis": 0,

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
    "compression": {"id": "blosc2|gzip|zlib"},

    // List of algorithms used to filter the data
    // - optional, defaults to no filters
    // - for byte-shuffle filtering, use either "shuffle" or "shuffle_simd"
    "filters": [
        {"id": "shuffle", "element_size": 4},
        {"id": "shuffle_simd", "element_size": 4}
    ],

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
    },

    // Optional: disable the chunk cache for this request
    "option_disable_chunk_cache": false,

    // Optional: convert the shape to bytes for more efficient serialisation
    "option_shape_as_bytes": false,

    // Optional: convert the count to bytes for more efficient serialisation
    "option_count_as_bytes": false
}
```

Request authentication is implemented using [Basic Auth](https://en.wikipedia.org/wiki/Basic_access_authentication).
For S3 object stores the username and password should consist of your S3 Access Key ID and Secret Access Key, respectively.
Unauthenticated access to S3 is possible by omitting the basic auth header.
For HTTP(s) object stores authentication is also implemented using Basic Auth and unauthenticated access is possible by omitting the basic auth header.

On success, all operations return HTTP 200 OK with the response body being a [CBOR](https://cbor.io/) object of the following format:

```
{
    // Response data. May be a scalar or multi-dimensional array.
    "bytes": b"\0\0\0\0\x06\0\0\0\t\0\0\0\0\0\0\0\x01\0\0\0\x03\0\0\0\x05\0\0\0\t\0\0\0\x08\0\0\0\x01\0\0\0\0\0\0\0\x08\0\0\0\t\0\0\0\x05\0\0\0\x04\0\0\0\x03\0\0\0\x05\0\0\0\x05\0\0\0\x02\0\0\0\0\0\0\0",
    // The data type of the bytes result, represented by lower case string.
    // One of int32, int64, uint32, uint64, float32 or float64.
    "dtype": "int32",
    // An array of numbers describing the shape of the data in the bytes returned.
    // May be empty for a scalar result.
    "shape": [10, 2],
    // If the option `option_shape_as_bytes` was enabled in the request
    // `shape` will be encoded into the following `shape_as_bytes` as a byte stream
    // and `shape` will be an empty `[]` vector.
    // The `shape_as_bytes` field encodes the shape as consecutive 64-bit unsigned
    // integers (u64), 8 bytes per element, using the response `byte_order`
    // ("big" or "little") for endianness when decoding.
    "shape_as_bytes": b"",
    // The number of non-missing array elements operated on while performing the requested reduction.
    // This is useful, for example, to calculate the mean over multiple requests
    // where the number of items operated on may differ between chunks.
    "count": [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    // If the option `option_count_as_bytes` was enabled in the request
    // `count` will be encoded into the following `count_as_bytes` as a byte stream
    // and `count` will be an empty `[]` vector.
    // The `count_as_bytes` field encodes the count as consecutive 64-bit signed
    // integers (i64), 8 bytes per element, using the response `byte_order`
    // ("big" or "little") for endianness when decoding.
    "count_as_bytes": b"",    // The byte order of the data in the response payload.
    // Either `big` or `little`.
    "byte_order": "little",
}

```

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
