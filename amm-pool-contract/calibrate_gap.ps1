<#
.SYNOPSIS
    Calibrate the local-WASM vs network measurement gap across soroban-sdk versions.

.DESCRIPTION
    For each SDK version listed in $VERSIONS, this script:
      1. Updates the soroban-sdk version in amm-pool-contract/Cargo.toml
      2. Runs `cargo update` to resolve the dependency tree
      3. Builds the contract WASM with the workspace release profile
      4. Runs the calibrate_gap test to extract local CPU & memory estimates
      5. Reports the results

    Network figures (from cargo-budget-report / simulateTransaction) must be
    collected separately and plugged into the output table manually.

    Usage:
      .\amm-pool-contract\calibrate_gap.ps1

    Requirements:
      - Rust toolchain with wasm32v1-none target
      - PowerShell 5.1+
#>

using namespace System.Collections.Generic

# Versions to test — oldest first.
# The first entry is the "current" baseline so the script can be re-run
# when the pinned version changes.
$VERSIONS = @(
    "20.0.0",
    "20.1.0",
    "21.0.0",
    "21.1.0",
    "22.0.0"
)

$ROOT      = Resolve-Path "$PSScriptRoot/.."
$MANIFEST  = "$ROOT/amm-pool-contract/Cargo.toml"
$CRATE     = "amm-pool-contract"

$RESULTS = [List[hashtable]]::new()

function Set-SdkVersion {
    param([string]$Version)
    Write-Host "`n=== Pinning soroban-sdk to $Version ===" -ForegroundColor Cyan
    $content = Get-Content -LiteralPath $MANIFEST -Raw
    $content = $content -replace 'soroban-sdk = "[^"]+"', "soroban-sdk = `"$Version`""
    $content = $content -replace '(?<=soroban-sdk = \{ version = )"[^"]+"', "`"$Version`""
    Set-Content -LiteralPath $MANIFEST -Value $content -NoNewline
}

function Invoke-Cargo {
    param([string]$Command)
    $result = & cargo $Command 2>&1
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        Write-Host "cargo $Command failed with exit code $exitCode" -ForegroundColor Red
        Write-Host ($result -join "`n")
        return $false
    }
    return $true
}

function Get-Measurement {
    param([string]$Version)

    # SDK 20 and 21 use env.budget() instead of env.cost_estimate().budget().
    $isPre22 = $Version -match '^2[01]\.'
    $testArgs = @("test", "-p", $CRATE)
    if ($isPre22) {
        $testArgs += @("--features", "sdk20", "--test", "calibrate_gap_sdk20", "calibrate_gap_sdk20")
    } else {
        $testArgs += @("--test", "calibrate_gap", "calibrate_gap")
    }
    $testArgs += @("--", "--nocapture")

    $testOutput = & cargo @testArgs 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Test failed:" -ForegroundColor Red
        Write-Host ($testOutput -join "`n")
        return $null
    }

    $cpu = $null
    $mem = $null
    foreach ($line in $testOutput) {
        if ($line -match '^CALIBRATE_CPU=(\d+)') { $cpu = [uint64]::Parse($Matches[1]) }
        if ($line -match '^CALIBRATE_MEM=(\d+)') { $mem = [uint64]::Parse($Matches[1]) }
    }

    if ($cpu -eq $null -or $mem -eq $null) {
        Write-Host "Could not parse CALIBRATE_CPU / CALIBRATE_MEM from test output" -ForegroundColor Red
        return $null
    }

    return @{ CPU = $cpu; MEM = $mem }
}

# --- Main ---

$ErrorActionPreference = "Stop"

foreach ($ver in $VERSIONS) {
    Write-Host "`n============================================" -ForegroundColor Yellow
    Write-Host "  SDK version: $ver" -ForegroundColor Yellow
    Write-Host "============================================" -ForegroundColor Yellow

    Set-SdkVersion -Version $ver

    if (-not (Invoke-Cargo "update -p soroban-sdk")) {
        Write-Host "Skipping $ver due to cargo update failure" -ForegroundColor Red
        continue
    }

    # SDK 21's env-host needs an extra update step to resolve ed25519-dalek conflict
    if ($ver -match '^21\.') {
        Write-Host "  (SDK 21 detected; running cargo update -p soroban-env-host)" -ForegroundColor DarkYellow
        $null = Invoke-Cargo "update -p soroban-env-host"
    }

    if (-not (Invoke-Cargo "build --target wasm32v1-none --release -p $CRATE")) {
        Write-Host "Skipping $ver due to build failure" -ForegroundColor Red
        continue
    }

    $measures = Get-Measurement -Version $ver
    if ($measures -eq $null) {
        Write-Host "Skipping $ver due to measurement failure" -ForegroundColor Red
        continue
    }

    $RESULTS.Add(@{
        Version = $ver
        CPU     = $measures.CPU
        MEM     = $measures.MEM
    })
}

# Restore original version (last in list is current baseline)
if ($VERSIONS.Count -gt 0) {
    $restore = $VERSIONS[-1]
    Write-Host "`n=== Restoring soroban-sdk to $restore ===" -ForegroundColor Cyan
    Set-SdkVersion -Version $restore
    Invoke-Cargo "update -p soroban-sdk" | Out-Null
}

# Print results table
Write-Host "`n`n============================================" -ForegroundColor Green
Write-Host "  CALIBRATION RESULTS" -ForegroundColor Green
Write-Host "============================================" -ForegroundColor Green
Write-Host ""
Write-Host "SDK Version | Local CPU  | Local Mem  |"

foreach ($r in $RESULTS) {
    Write-Host ("{0,-12} | {1,10} | {2,10} |" -f $r.Version, $r.CPU, $r.MEM)
}

Write-Host "`n--- Procedure for network figures ---"
Write-Host "1. For each SDK version above, deploy the WASM and run:"
Write-Host "   cargo run --bin cargo-budget-report -- --network testnet"
Write-Host "2. Extract the CPU instruction cost from the simulation response"
Write-Host "3. Compute delta = (local - network) / network"
Write-Host "4. Add the complete row to MEASUREMENTS.md"
