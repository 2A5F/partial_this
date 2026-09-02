<#
.SYNOPSIS
    Tests the partial_this workspace.

.DESCRIPTION
    Runs `cargo test` for both debug and release profiles, `cargo clippy`, and
    Miri (when the nightly toolchain and the `miri` component are available).
    The script stops on the first error.

.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts/test.ps1
#>

$ErrorActionPreference = "Stop"

# Run from the repository root regardless of the current working directory.
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location (Join-Path $scriptDir "..")

Write-Host "==> Testing (debug)..." -ForegroundColor Cyan
cargo test --workspace

Write-Host "==> Testing (release)..." -ForegroundColor Cyan
cargo test --workspace --release

Write-Host "==> Running clippy..." -ForegroundColor Cyan
cargo clippy --workspace --all-targets -- -D warnings

# Miri requires the nightly toolchain plus the `miri` component; skip if absent.
& cargo +nightly miri --version *> $null
if ($LASTEXITCODE -eq 0) {
    Write-Host "==> Running Miri tests..." -ForegroundColor Cyan
    cargo +nightly miri test --workspace
}
else {
    Write-Warning "Miri is not available (needs nightly + 'miri' component). Skipping."
}

Write-Host "Tests complete." -ForegroundColor Green
