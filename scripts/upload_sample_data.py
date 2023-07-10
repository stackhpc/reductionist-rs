from enum import Enum
import gzip
import numcodecs
import numpy as np
import pathlib
import s3fs
import zlib

NUM_ITEMS = 10
OBJECT_PREFIX = "data"
COMPRESSION_ALGS = [None, "gzip", "zlib"]
FILTER_ALGS = [None, "shuffle"]

#Use enum which also subclasses string type so that 
# auto-generated OpenAPI schema can determine allowed dtypes
class AllowedDatatypes(str, Enum):
    """ Data types supported by active storage proxy """
    int64 = 'int64'
    int32 = 'int32'
    float64 = 'float64'
    float32 = 'float32'
    uint64 = 'uint64'
    uint32 = 'uint32'

    def n_bytes(self):
        """ Returns the number of bytes in the data type """
        return np.dtype(self.name).itemsize

S3_URL = 'http://localhost:9000'

s3_fs = s3fs.S3FileSystem(key='minioadmin', secret='minioadmin', client_kwargs={'endpoint_url': S3_URL})
bucket = pathlib.Path('sample-data')

#Make sure s3 bucket exists
try:
    s3_fs.mkdir(bucket)
except FileExistsError:
    pass

# Create numpy arrays and upload to S3 as bytes
for compression in COMPRESSION_ALGS:
    compression_suffix = f"-{compression}" if compression else ""
    for filter in FILTER_ALGS:
        filter_suffix = f"-{filter}" if filter else ""
        for d in AllowedDatatypes:
            obj_name = f'{OBJECT_PREFIX}-{d}{compression_suffix}{filter_suffix}.dat'
            with s3_fs.open(bucket / obj_name, 'wb') as s3_file:
                data = np.arange(NUM_ITEMS, dtype=d).tobytes()
                if filter == "shuffle":
                    data = numcodecs.Shuffle(d.n_bytes()).encode(data)
                if compression == "gzip":
                    data = gzip.compress(data)
                elif compression == "zlib":
                    data = zlib.compress(data)
                s3_file.write(data)

print("Data upload successful. \nBucket contents:\n", "\n".join(s3_fs.ls(bucket)))
