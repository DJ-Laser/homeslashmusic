FROM rust:1.100-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
