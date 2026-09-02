<#
.SYNOPSIS
    Builds the partial_this workspace and generates documentation.

.DESCRIPTION
    Runs `cargo build` for both debug and release profiles, then generates
    documentation with `cargo doc`. The script stops on the first error.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/build.ps1
#>

$ErrorActionPreference = "Stop"

# Run from the repository root regardless of the current working directory.
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location (Join-Path $scriptDir "..")

Write-Host "==> Building (debug)..." -ForegroundColor Cyan
cargo build --workspace

Write-Host "==> Building (release)..." -ForegroundColor Cyan
cargo build --workspace --release

Write-Host "==> Generating documentation..." -ForegroundColor Cyan
cargo doc --workspace --no-deps

Write-Host "Build complete." -ForegroundColor Green
