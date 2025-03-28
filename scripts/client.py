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
    parser.add_argument("--cacert", type=str)
    parser.add_argument("--source", required=True, type=str)
    parser.add_argument("--username", required=True, type=str)
    parser.add_argument("--password", required=True, type=str)
    parser.add_argument("--bucket", required=True, type=str)
    parser.add_argument("--object", required=True, type=str)
    parser.add_argument("--dtype", required=True, type=str) #, choices=DTYPES) allow invalid
    parser.add_argument("--byte-order", type=str, choices=["big", "little"])
    parser.add_argument("--offset", type=int)
    parser.add_argument("--size", type=int)
    parser.add_argument("--shape", type=str)
    parser.add_argument("--axis", type=str)
    parser.add_argument("--order", default="C") #, choices=["C", "F"]) allow invalid for testing
    parser.add_argument("--selection", type=str)
    parser.add_argument("--compression", type=str)
    parser.add_argument("--shuffle", action=argparse.BooleanOptionalAction)
    missing = parser.add_mutually_exclusive_group()
    missing.add_argument("--missing-value", type=str)
    missing.add_argument("--missing-values", type=str)
    missing.add_argument("--valid-min", type=str)
    missing.add_argument("--valid-max", type=str)
    missing.add_argument("--valid-range", type=str)
    parser.add_argument("--verbose", action=argparse.BooleanOptionalAction)
    return parser.parse_args()


def parse_number(s: str):
    try:
        return int(s)
    except ValueError:
        return float(s)


def build_request_data(args: argparse.Namespace) -> dict:
    request_data = {
        'source': args.source,
        'bucket': args.bucket,
        'object': args.object,
        'dtype': args.dtype,
        'offset': args.offset,
        'size': args.size,
        'order': args.order,
        'compression': args.compression,
    }
    if args.byte_order:
        request_data["byte_order"] = args.byte_order
    if args.shape:
        request_data["shape"] = json.loads(args.shape)
    if args.axis is not None:
        request_data["axis"] = json.loads(args.axis)
    if args.selection:
        request_data["selection"] = json.loads(args.selection)
    if args.compression:
        request_data["compression"] = {"id": args.compression}
    filters = []
    if args.shuffle:
        element_size = 4 if "32" in args.dtype else 8
        filters.append({"id": "shuffle", "element_size": element_size})
    if filters:
        request_data["filters"] = filters
    missing = None
    if args.missing_value:
        missing = {"missing_value": parse_number(args.missing_value)}
    if args.missing_values:
        missing = {"missing_values": [parse_number(n) for n in args.missing_values.split(",")]}
    if args.valid_min:
        missing = {"valid_min": parse_number(args.valid_min)}
    if args.valid_max:
        missing = {"valid_max": parse_number(args.valid_max)}
    if args.valid_range:
        min, max = args.valid_range.split(",")
        missing = {"valid_range": [parse_number(min), parse_number(max)]}
    if missing:
        request_data["missing"] = missing
    return {k: v for k, v in request_data.items() if v is not None}


def request(url: str, username: str, password: str, request_data: dict, cacert: str):
    response = requests.post(
        url,
        json=request_data,
        auth=(username, password),
        verify=cacert or True,
    )
    return response


def display(response, verbose=False):
    #print(response.content)
    dtype = response.headers['x-activestorage-dtype']
    shape = json.loads(response.headers['x-activestorage-shape'])
    counts = json.loads(response.headers['x-activestorage-count'])
    counts = np.array(counts)
    if len(counts) > 1:
        counts = counts.reshape(shape)
    result = np.frombuffer(response.content, dtype=dtype).reshape(shape)
    if verbose:
        sep = "\n" if len(counts.shape) > 1 else " "
        print("\nResponse headers:", response.headers)
        print("\nNon-missing count(s):", counts, sep=sep)
        print("\nResult:", result, sep=sep)
    else:
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
    if args.verbose:
        print("\nRequest data:", request_data)
    url = f'{args.server}/v1/{args.operation}/'
    response = request(url, args.username, args.password, request_data, args.cacert)
    if response.ok:
        display(response, verbose=args.verbose)
    else:
        display_error(response)
        sys.exit(1)


if __name__ == "__main__":
    main()
