# ========================================================
# School OS Database Migration Runner (PowerShell)
# ========================================================
param (
    [string]$TargetEnv = "local"
)

$ErrorActionPreference = "Stop"

$MigrationsDir = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "..\migrations"
$MigrationsDir = (Resolve-Path $MigrationsDir).Path

if ($TargetEnv -eq "production") {
    $DbUrl = if ($env:PROD_DATABASE_URL) { $env:PROD_DATABASE_URL } else { $env:DATABASE_URL }
    Write-Warning "Running migrations on PRODUCTION database!"
} elseif ($TargetEnv -eq "staging") {
    $DbUrl = if ($env:STAGING_DATABASE_URL) { $env:STAGING_DATABASE_URL } else { $env:DATABASE_URL }
    Write-Host "Running migrations on Staging database..." -ForegroundColor Cyan
} else {
    $DbUrl = if ($env:DATABASE_URL) { $env:DATABASE_URL } else { "postgres://school_admin:secretpassword@localhost:5433/school_os" }
    Write-Host "Running migrations on Local/Dev database..." -ForegroundColor Green
}

if (-not $DbUrl) {
    Write-Error "DATABASE_URL is not configured."
}

Write-Host "Migrations source: $MigrationsDir"

if (Get-Command sqlx -ErrorAction SilentlyContinue) {
    sqlx migrate run --source "$MigrationsDir" --database-url "$DbUrl"
    Write-Host "✅ Migrations completed successfully!" -ForegroundColor Green
} else {
    Write-Host "sqlx-cli not detected, installing..." -ForegroundColor Yellow
    cargo install sqlx-cli --no-default-features --features native-tls,postgres
    sqlx migrate run --source "$MigrationsDir" --database-url "$DbUrl"
    Write-Host "✅ Migrations completed successfully!" -ForegroundColor Green
}
