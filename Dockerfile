# syntax=docker/dockerfile:1

FROM rust:latest AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    cmake \
    libclang-dev \
    libfftw3-dev \
    libavutil-dev \
    libavformat-dev \
    libswresample-dev \
    libavcodec-dev

# Copy dependency files, then fetch dependencies.
# This layer will only be rebuilt when we modify dependencies.
COPY needle/Cargo.toml needle/Cargo.lock /app/needle/
RUN cargo fetch --manifest-path needle/Cargo.toml

# Copy the code and build the binary.
COPY . .
RUN cargo install --path needle/

FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    libfftw3-3 \
    libavutil56 \
    libavformat58 \
    libswresample3 \
    libavcodec58

COPY --from=builder /usr/local/cargo/bin/needle /usr/local/bin/needle

ENTRYPOINT ["needle"]
CMD ["--help"]
