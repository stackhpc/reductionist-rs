# Make a request to the active storage server.

import json
import requests
import numpy as np
import sys

request_data = {
  'source': 'http://localhost:9000',
  'bucket': 'sample-data',
  'object': 'data-uint32.dat',
  'dtype': 'uint32',
  # All other fields assume their default values
}

if len(sys.argv) > 1:
    reducer = sys.argv[1]
else:
    reducer = 'min'

response = requests.post(
  f'http://localhost:8000/v1/{reducer}/',
  json=request_data,
  auth=('minioadmin', 'minioadmin')
)
print(response.content)
sum_result = np.frombuffer(response.content, dtype=response.headers['x-activestorage-dtype'])
shape = json.loads(response.headers['x-activestorage-shape'])
sum_result = sum_result.reshape(shape)
print(sum_result)
