# syntax=docker/dockerfile:1

# Adapted from the multi-stage build example in https://hub.docker.com/_/rust

# Which Cargo profile to use.
ARG PROFILE=release

# Stage 1: builder
FROM rust:1.70 as builder
ARG PROFILE
WORKDIR /build
COPY . .
# NOTE: By default 'cargo install' ignores the Cargo.lock file, and pulls in
# the latest allowed versions. This can result in builds failing, so use the
# --locked argument to use Cargo.lock.
RUN cargo install --path . --profile $PROFILE --locked

# Stage 2: final image
FROM debian:bullseye-slim
# AWS SDK requires CA certificates to be present.
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates \
    && update-ca-certificates
COPY --from=builder /usr/local/cargo/bin/reductionist /usr/local/bin/reductionist
CMD ["reductionist"]
