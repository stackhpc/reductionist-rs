from enum import Enum
import numpy as np
import pathlib
import s3fs

NUM_ITEMS = 10
OBJECT_PREFIX = "data"

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
for d in AllowedDatatypes.__members__.keys():
    with s3_fs.open(bucket / f'{OBJECT_PREFIX}-{d}.dat', 'wb') as s3_file:
        s3_file.write(np.arange(NUM_ITEMS, dtype=d).tobytes())

print("Data upload successful. \nBucket contents:\n", s3_fs.ls(bucket))
