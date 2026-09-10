# School OS — Cloud Backend Engine (Rust & Axum)

High-performance, secure, and production-ready microservices backend for the **School OS** academic platform. Built with modern Rust, Axum, and SQLx.

---

## 🚀 One-Click Railway Deployment

This repository is pre-configured with a multi-stage `Dockerfile` and `railway.toml` for seamless deployment on [Railway](https://railway.app).

### 1. Environment Variables

Configure the following environment variables in your Railway project service settings:

| Variable | Required | Description | Example |
|---|:---:|---|---|
| `DATABASE_URL` | **Yes** | PostgreSQL connection URI (auto-provided if using Railway Postgres plugin) | `postgres://postgres:password@postgres.railway.internal:5432/railway` |
| `JWT_SECRET` | **Yes** | Secret string for HMAC-SHA256 JWT auth token generation | `secure_random_jwt_secret_key_32_chars` |
| `PORT` | Auto | Provided automatically by Railway | `8080` |
| `RUST_LOG` | No | Logging verbosity filter | `api_server=info,school_core=info` |

### 2. Database Schema & Auto-Migrations

All 41 database schema migrations located in `/migrations` are compiled into the binary and executed **automatically on server boot**. When you link a fresh PostgreSQL database on Railway, the tables and seed data are initialized automatically without manual CLI intervention.

### 3. Healthcheck Endpoint

- Path: `/api/v1/system/maintenance-status`
- Response: `200 OK` (`{"success": true, ...}`)
- Swagger UI Documentation: `/swagger-ui/`
- OpenAPI Specification: `/api-docs/openapi.json`
- Metrics: `/metrics` (Prometheus)

---

## 🛠 Local Development

```bash
# 1. Start PostgreSQL
# Ensure PostgreSQL is running and DATABASE_URL is set in .env

# 2. Run API Server
cargo run -p api-server

# 3. Build for Release
cargo build --release -p api-server
```

---

## 📁 Repository Structure

```
.
├── Cargo.toml            # Workspace manifest
├── Cargo.lock            # Locked dependency graph
├── Dockerfile            # Multi-stage release container build
├── railway.toml          # Railway platform deployment configuration
├── migrations/           # SQL schema migrations (embedded in binary)
├── api-server/           # Axum HTTP API service & routing layer
├── school-core/          # Domain models, clean architecture use cases & repositories
├── local-bridge/         # Local integration bridge & test harnesses
├── hash-gen/             # Password hashing utility
├── database/             # Database seeds and reference SQL
└── scripts/              # Setup, migration, and management utilities
```
