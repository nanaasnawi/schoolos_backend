# Multi-stage Dockerfile for SchoolOS Rust Backend (api-server)
# Optimized for Railway & containerized production deployment

# ── 1. Builder Stage ──────────────────────────────────────────────────────────
FROM rust:bookworm AS builder

WORKDIR /usr/src/schoolos

# Install essential build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY school-core ./school-core
COPY api-server ./api-server
COPY local-bridge ./local-bridge
COPY hash-gen ./hash-gen
COPY migrations ./migrations
COPY .sqlx ./.sqlx

ENV SQLX_OFFLINE=true

# Build the api-server binary in release mode
RUN cargo build --release -p api-server --bin api-server

# ── 2. Runtime Stage ──────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runner

WORKDIR /app

# Install minimal runtime dependencies (ca-certificates & OpenSSL for TLS)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r schoolos && useradd -r -g schoolos schoolos

# Copy compiled binary from builder
COPY --from=builder /usr/src/schoolos/target/release/api-server /usr/local/bin/api-server

# Create directory for uploads if local file storage is used
RUN mkdir -p /app/uploads && chown -R schoolos:schoolos /app

USER schoolos

# Default port for Railway (Railway will override PORT at runtime)
ENV PORT=8080
EXPOSE 8080

CMD ["api-server"]
