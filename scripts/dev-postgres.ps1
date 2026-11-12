# Starts the local portable PostgreSQL 17 for Nook development.
# Safe to run repeatedly: if the server is already accepting connections, it exits.
#
# Usage:  powershell -ExecutionPolicy Bypass -File scripts\dev-postgres.ps1
# Databases expected: nook (server), nook_test (integration tests).

$ErrorActionPreference = "Stop"

$pgRoot = Join-Path $env:LOCALAPPDATA "pgsql17\pgsql"
$pgBin  = Join-Path $pgRoot "bin"
$dataDir = Join-Path $pgRoot "data"
$psql = Join-Path $pgBin "psql.exe"

function Test-PgUp {
    try {
        & $psql -U postgres -h localhost -p 5432 -t -c "SELECT 1;" 2>$null
        return $LASTEXITCODE -eq 0
    } catch {
        return $false
    }
}

if (-not (Test-Path (Join-Path $pgBin "pg_ctl.exe"))) {
    Write-Host "PostgreSQL not found at $pgRoot -- set it up first (see docs/DEPLOYMENT.md)."
    exit 1
}

if (Test-PgUp) {
    Write-Host "PostgreSQL is already running on localhost:5432."
    exit 0
}

if (-not (Test-Path (Join-Path $dataDir "PG_VERSION"))) {
    Write-Host "Initializing data directory at $dataDir ..."
    & (Join-Path $pgBin "initdb.exe") -D $dataDir -U postgres --auth=trust -E UTF8 | Select-Object -Last 3
}

Write-Host "Starting PostgreSQL ..."
& (Join-Path $pgBin "pg_ctl.exe") -D $dataDir -l (Join-Path $pgRoot "logfile") start | Out-Null

# pg_ctl returns before the server is accepting connections in some cases.
for ($i = 0; $i -lt 15; $i++) {
    Start-Sleep -Milliseconds 500
    if (Test-PgUp) { break }
}

if (-not (Test-PgUp)) {
    Write-Host "PostgreSQL did not come up; check $pgRoot\logfile."
    exit 1
}

foreach ($db in @("nook", "nook_test")) {
    $exists = & $psql -U postgres -h localhost -p 5432 -t -c "SELECT 1 FROM pg_database WHERE datname = '$db';"
    if (-not $exists) {
        & (Join-Path $pgBin "createdb.exe") -U postgres -h localhost -p 5432 $db
        Write-Host "Created database $db"
    }
}

Write-Host "PostgreSQL ready (localhost:5432, user postgres, databases nook + nook_test)."
