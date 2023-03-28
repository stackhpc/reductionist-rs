# Make a request to the active storage server.

# Example usage with minio and sum of uint32 sample data:
#
# python ./scripts/client.py \
#   sum \
#   --server http://localhost:8080 \
#   --source http://localhost:9000 \
#   --username minioadmin --password minioadmin \
#   --bucket sample-data --object data-uint32.dat \
#   --dtype uint32

import argparse
import http.client
import json
import requests
import numpy as np
import sys


DTYPES = ["int32", "int64", "uint32", "uint64", "float32", "float64"]


def get_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("operation")
    parser.add_argument("--server", required=True, type=str)
    parser.add_argument("--source", required=True, type=str)
    parser.add_argument("--username", required=True, type=str)
    parser.add_argument("--password", required=True, type=str)
    parser.add_argument("--bucket", required=True, type=str)
    parser.add_argument("--object", required=True, type=str)
    parser.add_argument("--dtype", required=True, type=str) #, choices=DTYPES) allow invalid
    parser.add_argument("--offset", type=int)
    parser.add_argument("--size", type=int)
    parser.add_argument("--shape", type=str)
    parser.add_argument("--order", default="C") #, choices=["C", "F"]) allow invalid for testing
    parser.add_argument("--selection", type=str)
    return parser.parse_args()


def build_request_data(args: argparse.Namespace) -> dict:
    request_data = {
        'source': args.source,
        'bucket': args.bucket,
        'object': args.object,
        'dtype': args.dtype,
        'offset': args.offset,
        'size': args.size,
        'order': args.order,
    }
    if args.shape:
        request_data["shape"] = json.loads(args.shape)
    if args.selection:
        request_data["selection"] = json.loads(args.selection)
    return {k: v for k, v in request_data.items() if v is not None}


def request(url: str, username: str, password: str, request_data: dict):
    response = requests.post(
        url,
        json=request_data,
        auth=(username, password)
    )
    return response


def display(response):
    #print(response.content)
    dtype = response.headers['x-activestorage-dtype']
    shape = json.loads(response.headers['x-activestorage-shape'])
    result = np.frombuffer(response.content, dtype=dtype)
    result = result.reshape(shape)
    print(result)


def display_error(response):
    print(response.status_code, http.client.responses[response.status_code])
    try:
        print(json.dumps(response.json()))
    except requests.exceptions.JSONDecodeError:
        print(response.content)


def main():
    args = get_args()
    request_data = build_request_data(args)
    url = f'{args.server}/v1/{args.operation}/'
    response = request(url, args.username, args.password, request_data)
    if response.ok:
        display(response)
    else:
        display_error(response)
        sys.exit(1)


if __name__ == "__main__":
    main()
