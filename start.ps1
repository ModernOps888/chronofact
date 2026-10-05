# ChronoFact Launcher
# Starts the high-performance Rust Epistemic Backend on :3030 and React Cockpit on :5173

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " 🚀 Starting ChronoFact Epistemic Backbone & Cockpit" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# Ensure target release binary exists
$binPath = "c:\chronofact\target\release\chronofact.exe"
if (-not (Test-Path $binPath)) {
    Write-Host "Release binary not found. Compiling now..." -ForegroundColor Yellow
    cargo build --release --manifest-path "c:\chronofact\Cargo.toml"
}

# Start Rust Backend on 3030
Write-Host "`n[1/2] Launching Rust Epistemic Engine on http://127.0.0.1:3030 ..." -ForegroundColor Green
$backendJob = Start-Job -ScriptBlock {
    & "c:\chronofact\target\release\chronofact.exe" serve --port 3030
}

# Start React Frontend Cockpit
Write-Host "[2/2] Launching React / TypeScript Cockpit on http://localhost:5173 ..." -ForegroundColor Green
Set-Location "c:\chronofact\frontend"
npm run dev
