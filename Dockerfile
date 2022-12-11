# syntax=docker/dockerfile:1

## Build
FROM rust:1.65 AS builder

WORKDIR /app

COPY . .

RUN apt-get update && apt-get install -y \
    pkg-config \
    cmake \
    libclang-dev \
    libfftw3-dev \
    libavutil-dev \
    libavformat-dev \
    libswresample-dev \
    libavcodec-dev

RUN cargo install --path needle/

## Deploy
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    libavutil56 \
    libavformat58 \
    libswresample3 \
    libavcodec58

COPY --from=builder /usr/local/cargo/bin/needle /usr/local/bin/needle

CMD ["needle"]
