# SPDX-License-Identifier: Apache-2.0
# Multi-stage Docker build for the hwledger-server binary.
# The binary is the single entry point — bundles all 11 search crates.

# ---------- Builder ----------
FROM rust:1.85-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build with all features (lancedb enabled). Cache layer — only re-downloads
# on Cargo.toml/lock changes.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p hwledger-server --features hwledger-search-index/lancedb && \
    cp target/release/hwledger-server /app/hwledger-server

# ---------- Runtime ----------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/hwledger-server /usr/local/bin/hwledger-server

EXPOSE 8080
ENV PORT=8080 \
    DATA_DIR=/data

ENTRYPOINT ["hwledger-server"]
