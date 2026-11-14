# Multi-stage Dockerfile for SchoolOS Rust Backend (api-server)
# Optimized for Railway & containerized production deployment with cargo-chef caching

# ── 1. Chef Base Stage ────────────────────────────────────────────────────────
# Pin Rust 1.99: Rust 1.100+ (latest-rust-1-bookworm since 2026-11-13) enables
# never-type (!) fallback changes (rust#148922) that break sqlx-postgres 0.7.4.
FROM lukemathwalker/cargo-chef:latest-rust-1.99-bookworm AS chef
WORKDIR /usr/src/schoolos

# ── 2. Planner Stage ──────────────────────────────────────────────────────────
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY school-core ./school-core
COPY api-server ./api-server
COPY local-bridge ./local-bridge
RUN cargo chef prepare --recipe-path recipe.json

# ── 3. Builder Stage (Caches all 400+ third-party crates!) ────────────────────
FROM chef AS builder
COPY --from=planner /usr/src/schoolos/recipe.json recipe.json

# Cook dependencies - this heavy layer is 100% cached by Docker unless Cargo.lock changes!
RUN cargo chef cook --release --recipe-path recipe.json

# Copy application source code and SQL queries
COPY Cargo.toml Cargo.lock ./
COPY school-core ./school-core
COPY api-server ./api-server
COPY local-bridge ./local-bridge
COPY migrations ./migrations
COPY .sqlx ./.sqlx

ENV SQLX_OFFLINE=true

# Fast build: compiles only application code in ~15-30s without hitting build timeout
RUN cargo build --release -p api-server --bin api-server

# ── 4. Runtime Stage ──────────────────────────────────────────────────────────
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
WORKDIR /app

# Default port for Railway (Railway will override PORT at runtime)
ENV PORT=8080
EXPOSE 8080

CMD ["api-server"]
