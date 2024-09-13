# Make multiple requests to the active storage server in parallel.
#
# Example usage with minio and sum of uint32 sample data:
#
# python ./scripts/parallel-client.py \
#   sum \
#   --server http://localhost:8080 \
#   --source http://localhost:9000 \
#   --username minioadmin --password minioadmin \
#   --bucket sample-data --object data-uint32.dat \
#   --dtype uint32 \
#   --num-requests 10
#
# Different techniques are possible:
# --aiohttp : Use asyncio + aiohttp
# --httpx : Use asyncio + httpx
# --http2 : Enable HTTP/2 (may still fall back to HTTP1.1)
# --num-threads : Use multiple threads

import aiohttp
import argparse
import asyncio
import concurrent.futures
import http.client
import httpx
import json
import requests
import numpy as np
import ssl
import time
import urllib3


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
    parser.add_argument("--byte-order", type=str, choices=["big", "little"])
    parser.add_argument("--offset", type=int)
    parser.add_argument("--size", type=int)
    parser.add_argument("--shape", type=str)
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
    parser.add_argument("--aiohttp", action=argparse.BooleanOptionalAction)
    parser.add_argument("--httpx", action=argparse.BooleanOptionalAction)
    parser.add_argument("--http2", action=argparse.BooleanOptionalAction)
    parser.add_argument("--num-requests", type=int, default=1)
    parser.add_argument("--num-threads", type=int)
    parser.add_argument("--cacert", type=str)
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


def make_ssl_context(args):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    if args.cacert:
        context.load_verify_locations(args.cacert)
    return context


def request(session, url: str, request_data: dict):
    response = session.post(
        url,
        json=request_data,
    )
    return response


def display(response, verbose=False):
    #print(response.content)
    dtype = response.headers['x-activestorage-dtype']
    shape = json.loads(response.headers['x-activestorage-shape'])
    result = np.frombuffer(response.content, dtype=dtype)
    result = result.reshape(shape)
    if verbose:
        print("\nResponse headers:", response.headers)
        print("\nResult:", result)
    else:
        print(result)


def display_error(response):
    print(response.status_code, http.client.responses[response.status_code])
    try:
        print(json.dumps(response.json()))
    except requests.exceptions.JSONDecodeError:
        print(response.content)


async def display_error_aiohttp(response):
    status_code = response.status
    print(response.status, http.client.responses[response.status])
    try:
        print(json.dumps(await response.json()))
    except requests.exceptions.JSONDecodeError:
        print(response.content)


def run(session, args):
    request_data = build_request_data(args)
    if args.verbose:
        print("\nRequest data:", request_data)
    url = f'{args.server}/v1/{args.operation}/'
    return request(session, url, request_data)


def run_serially(args):
    num_errors = 0
    with requests.Session() as session:
        session.auth = (args.username, args.password)
        session.verify = args.cacert or True
        for _ in range(args.num_requests):
            response = run(session, args)
            if response.ok:
                #display(response, verbose=args.verbose)
                pass
            else:
                display_error(response)
                num_errors += 1
    return num_errors


def run_threads(args):
    num_errors = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.num_threads) as executor:
        with requests.Session() as session:
            session.auth = (args.username, args.password)
            session.verify = args.cacert or True
            futures = [executor.submit(run, session, args) for _ in range(args.num_requests)]
            for future in concurrent.futures.as_completed(futures):
                try:
                    response = future.result()
                except Exception as exc:
                    print(f"Failed! {exc}")
                else:
                    if response.ok:
                        #display(response, verbose=args.verbose)
                        pass
                    else:
                        display_error(response)
                        num_errors += 1
    return num_errors


async def run_async_aiohttp(args):
    num_errors = 0
    responses = []
    limit = 1000 # FIXME!
    auth = aiohttp.BasicAuth(args.username, args.password)
    ssl_context = make_ssl_context(args)
    connector = aiohttp.TCPConnector(limit=limit, ssl=ssl_context)
    async with aiohttp.ClientSession(auth=auth, connector=connector) as session:
        url = f'{args.server}/v1/{args.operation}/'
        for _ in range(args.num_requests):
            request_data = build_request_data(args)
            if args.verbose:
                print("\nRequest data:", request_data)
            responses.append(request(session, url, request_data))

        responses = await asyncio.gather(*responses)

        for response in responses:
            if response.ok:
                #display(response, verbose=args.verbose)
                pass
            else:
                await display_error_aiohttp(response)
                num_errors += 1
    return num_errors


async def run_async_httpx(args):
    num_errors = 0
    responses = []
    http2 = args.http2
    auth = (args.username, args.password)
    verify = make_ssl_context(args)
    limits = httpx.Limits(max_connections=1000)
    async with httpx.AsyncClient(http2=http2, auth=auth, verify=verify, limits=limits) as client:
        url = f'{args.server}/v1/{args.operation}/'
        for _ in range(args.num_requests):
            request_data = build_request_data(args)
            if args.verbose:
                print("\nRequest data:", request_data)
            responses.append(request(client, url, request_data))

        responses = await asyncio.gather(*responses)

        for response in responses:
            if response.is_success:
                #display(response, verbose=args.verbose)
                assert response.http_version == "HTTP/2" if http2 else "HTTP/1.1"
                pass
            else:
                display_error(response)
                num_errors += 1
    return num_errors


def main():
    try:
        urllib3.disable_warnings(urllib3.exceptions.SubjectAltNameWarning)
    except:
        pass
    args = get_args()
    start = time.time()
    num_errors = 0
    if args.num_threads is not None:
        num_errors = run_threads(args)
    elif args.aiohttp:
        num_errors = asyncio.run(run_async_aiohttp(args))
    elif args.httpx:
        num_errors = asyncio.run(run_async_httpx(args))
    else:
        num_errors = run_serially(args)
    end = time.time()
    duration = end - start
    print(f"Performed {args.num_requests} requests ({args.num_requests/duration:.2f} req/s) in {duration:.2f}s with {num_errors} ({100*num_errors/args.num_requests:.2f}%) errors")


if __name__ == "__main__":
    main()
