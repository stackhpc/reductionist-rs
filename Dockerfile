# syntax=docker/dockerfile:1

# Adapted from the multi-stage build example in https://hub.docker.com/_/rust

# Stage 1: builder
FROM rust:1.66 as builder
WORKDIR /usr/src/s3-active-storage
COPY . .
RUN cargo install --path .

# Stage 2: final image
FROM debian:bullseye-slim
# AWS SDK requires CA certificates to be present.
RUN apt update \
    && apt install -y --no-install-recommends ca-certificates \
    && update-ca-certificates
COPY --from=builder /usr/local/cargo/bin/s3-active-storage /usr/local/bin/s3-active-storage
CMD ["s3-active-storage"]
