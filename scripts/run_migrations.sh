#!/usr/bin/env bash
set -e

# ========================================================
# School OS Database Migration Runner (Staging / Production)
# ========================================================

TARGET_ENV="${1:-local}"

if [ "$TARGET_ENV" = "production" ]; then
    DB_URL="${PROD_DATABASE_URL:-$DATABASE_URL}"
    echo "⚠️  RUNNING MIGRATIONS ON PRODUCTION DATABASE!"
elif [ "$TARGET_ENV" = "staging" ]; then
    DB_URL="${STAGING_DATABASE_URL:-$DATABASE_URL}"
    echo "🚀 Running migrations on Staging database..."
else
    DB_URL="${DATABASE_URL:-postgres://school_admin:secretpassword@localhost:5433/school_os}"
    echo "🛠️  Running migrations on Local/Dev database..."
fi

if [ -z "$DB_URL" ]; then
    echo "ERROR: Database URL is not defined."
    exit 1
fi

MIGRATIONS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../migrations" && pwd)"

echo "Migrations source: $MIGRATIONS_DIR"

if command -v sqlx >/dev/null 2>&1; then
    sqlx migrate run --source "$MIGRATIONS_DIR" --database-url "$DB_URL"
    echo "✅ Migrations completed successfully!"
else
    echo "sqlx-cli not found. Installing via cargo..."
    cargo install sqlx-cli --no-default-features --features native-tls,postgres
    sqlx migrate run --source "$MIGRATIONS_DIR" --database-url "$DB_URL"
    echo "✅ Migrations completed successfully!"
fi
