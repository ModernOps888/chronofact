[CmdletBinding()]
param(
    [switch]$Quiet
)

# ChronoFact Launcher
# Starts the high-performance Rust Epistemic Backend on :3030 and React Cockpit on :5173

if (-not $Quiet) {
    Write-Host "==========================================================" -ForegroundColor Cyan
    Write-Host " 🚀 Starting ChronoFact Epistemic Backbone & Cockpit" -ForegroundColor Cyan
    Write-Host "==========================================================" -ForegroundColor Cyan
}

# Resolve the best release binary
$binPath = "c:\chronofact\bin\chronofact.exe"
$targetBin = "c:\chronofact\target\release\chronofact.exe"

if (Test-Path $targetBin) {
    try {
        Copy-Item $targetBin $binPath -Force -ErrorAction SilentlyContinue
    } catch {}
} elseif (-not (Test-Path $binPath)) {
    if (-not $Quiet) { Write-Host "Release binary not found. Compiling now..." -ForegroundColor Yellow }
    cargo build --release --manifest-path "c:\chronofact\Cargo.toml"
    try {
        Copy-Item $targetBin $binPath -Force -ErrorAction SilentlyContinue
    } catch {}
}

# Determine which binary to execute
$exeToRun = if (Test-Path $targetBin) { $targetBin } elseif (Test-Path $binPath) { $binPath } else { "chronofact.exe" }

# Start Rust Backend on 3030 if not already running
$port3030 = Get-NetTCPConnection -LocalPort 3030 -State Listen -ErrorAction SilentlyContinue
if (-not $port3030) {
    if (-not $Quiet) { Write-Host "`n[1/2] Launching Rust Epistemic Engine on http://127.0.0.1:3030 ..." -ForegroundColor Green }
    $windowStyle = if ($Quiet) { "Hidden" } else { "Minimized" }
    Start-Process -FilePath $exeToRun -ArgumentList "serve", "--port", "3030" -WindowStyle $windowStyle
    Start-Sleep -Seconds 1
} else {
    if (-not $Quiet) { Write-Host "`n[1/2] Rust Epistemic Engine already listening on http://127.0.0.1:3030" -ForegroundColor Green }
}

# Start React Frontend Cockpit on 5173
$port5173 = Get-NetTCPConnection -LocalPort 5173 -State Listen -ErrorAction SilentlyContinue
if (-not $port5173) {
    if (-not $Quiet) { Write-Host "[2/2] Launching React / TypeScript Cockpit on http://localhost:5173 ..." -ForegroundColor Green }
    $frontWindowStyle = if ($Quiet) { "Hidden" } else { "Minimized" }
    Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile", "-Command", "Set-Location 'c:\chronofact\frontend'; npm run dev" -WindowStyle $frontWindowStyle
    Start-Sleep -Seconds 2
} else {
    if (-not $Quiet) { Write-Host "[2/2] React / TypeScript Cockpit already active on http://localhost:5173" -ForegroundColor Green }
}

# Launch browser if not running in background boot mode
if (-not $Quiet) {
    Start-Process "http://localhost:5173"
    Write-Host "`n[ACTIVE] ChronoFact Cockpit is open at http://localhost:5173" -ForegroundColor Cyan
    Write-Host "To stop services at any time, run: .\stop.bat or .\stop_chronofact.bat`n" -ForegroundColor Yellow
}
