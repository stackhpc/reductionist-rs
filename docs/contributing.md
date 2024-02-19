# Contributing

## Testing of Reductionist

A majority of the application code in Reductionist is unit tested, and any new code should include unit tests where practical.
Unit tests in Rust code typically reside in the same file as the module being tested.

The [S3 Active Storage compliance test suite](https://github.com/stackhpc/s3-active-storage-compliance-suite/) should be updated to test any new features added to Reductionist.

### Continuous Integration (CI)

GitHub Actions is used for CI for pull requests.
It checks that the package builds, and passes various checks, unit and integration tests.

### Benchmarks

Benchmark tests in the `benches` directory were created for various modules and used to make performance improvements.
These can be run using `cargo bench`, or a specific benchmark with `cargo bench --bench <benchmark name>`

## Pre-commit hook

A pre-commit hook is provided in `tools/pre-commit` that runs formatting, clippy, and unit tests. After cloning this repository, copy it to `.git/hooks/pre-commit`.

## Development environment

!!! note 

    For production deployments Reductionist provides an Ansible playbook to easily deploy it and supporting services to one or more hosts. See the [deployment guide](deployment.md) for details.

There are various ways to run the Reductionist server for development purposes.

### Running in a container

The simplest method is to run it in a container using a pre-built image:

```sh
docker run -it --detach --rm --net=host --name reductionist ghcr.io/stackhpc/reductionist-rs:latest
```

Images are published to [GitHub Container Registry](https://github.com/stackhpc/reductionist-rs/pkgs/container/reductionist-rs) when the project is released.
The `latest` tag corresponds to the most recent release, or you can use a specific release e.g. `0.1.0`.

This method does not require access to the source code.

### Building a container image

If you need to use unreleased changes, but still want to run in a container, it is possible to build an image.
First, clone this repository:

```sh
git clone https://github.com/stackhpc/reductionist-rs.git
cd reductionist-rs
```

```sh
make build
```

The image will be tagged as `reductionist`.
The image may be pushed to a registry, or deployed locally.

```sh
make run
```

### Building a Reductionist binary

If you prefer not to run the Reductionist server in a container, it will be necessary to build a binary.
Building locally may also be preferable during development to take advantage of incremental compilation.

#### Prerequisites

This project is written in Rust, and as such requires a Rust toolchain to be installed in order to build it.
The Minimum Supported Rust Version (MSRV) is 1.70.0, due to a dependency on the [AWS SDK](https://github.com/awslabs/aws-sdk-rust).
It may be necessary to use [rustup](https://rustup.rs/) rather than the OS provided Rust toolchain to meet this requirement.
See the [Rust book](https://doc.rust-lang.org/book/ch01-01-installation.html) for toolchain installation.

#### Build and run Reductionist

First, clone this repository:

```sh
git clone https://github.com/stackhpc/reductionist-rs.git
cd reductionist-rs
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
reductionist
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

Request authentication is implemented using [Basic Auth](https://en.wikipedia.org/wiki/Basic_access_authentication) with the username and password consisting of your S3 Access Key ID and Secret Access Key, respectively. If provided, these credentials are then used internally to authenticate with the upstream S3 source using [standard AWS authentication methods](https://docs.aws.amazon.com/AmazonS3/latest/API/sigv4-auth-using-authorization-header.html). If no basic auth header is provided, an unauthenticated request will be made to S3.

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

### Application documentation

This documentation is built using [MkDocs](https://www.mkdocs.org/) and hosted on [GitHub pages](https://stackhpc.github.io/reductionist-rs/).
The configuration file is [mkdocs.yml](https://github.com/stackhpc/reductionist-rs/blob/main/mkdocs.yml), and documentation Markdown source is in [docs/](https://github.com/stackhpc/reductionist-rs/blob/main/docs).
GitHub Actions workflows build the documentation in pull requests, and deploy it to GitHub pages on pushes to `main`.

To build and serve the documentation locally at <http://127.0.0.1:8000/reductionist-rs>:
```
python3 -m venv mkdocs-venv
source mkdocs-venv/bin/activate
pip install -U pip
pip install -r docs-requirements.txt
mkdocs serve --strict
```

### Source code documentation

The source code is documented using [rustdoc](https://doc.rust-lang.org/rustdoc/what-is-rustdoc.html).
Documentation is available on [docs.rs](https://docs.rs/reductionist/latest/reductionist/).
It is also possible to build the documentation locally:

```sh
cargo doc --no-deps
```

The resulting documentation is available under `target/doc`, and may be viewed in a web browser using file:///path/to/reductionist/target/doc/reductionist/index.html.
