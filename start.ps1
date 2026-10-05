# ChronoFact Launcher
# Starts the high-performance Rust Epistemic Backend on :3030 and React Cockpit on :5173

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host " 🚀 Starting ChronoFact Epistemic Backbone & Cockpit" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# Ensure isolated release binary exists
$binPath = "c:\chronofact\bin\chronofact.exe"
if (-not (Test-Path $binPath)) {
    if (Test-Path "c:\chronofact\target\release\chronofact.exe") {
        Copy-Item "c:\chronofact\target\release\chronofact.exe" "c:\chronofact\bin\chronofact.exe" -Force
    } else {
        Write-Host "Release binary not found. Compiling now..." -ForegroundColor Yellow
        cargo build --release --manifest-path "c:\chronofact\Cargo.toml"
        Copy-Item "c:\chronofact\target\release\chronofact.exe" "c:\chronofact\bin\chronofact.exe" -Force
    }
}

# Start Rust Backend on 3030 if not already running
$port3030 = Get-NetTCPConnection -LocalPort 3030 -State Listen -ErrorAction SilentlyContinue
if (-not $port3030) {
    Write-Host "`n[1/2] Launching Rust Epistemic Engine on http://127.0.0.1:3030 ..." -ForegroundColor Green
    Start-Process -FilePath "c:\chronofact\bin\chronofact.exe" -ArgumentList "serve", "--port", "3030" -WindowStyle Minimized
    Start-Sleep -Seconds 1
} else {
    Write-Host "`n[1/2] Rust Epistemic Engine already listening on http://127.0.0.1:3030" -ForegroundColor Green
}

# Start React Frontend Cockpit on 5173
$port5173 = Get-NetTCPConnection -LocalPort 5173 -State Listen -ErrorAction SilentlyContinue
if (-not $port5173) {
    Write-Host "[2/2] Launching React / TypeScript Cockpit on http://localhost:5173 ..." -ForegroundColor Green
    Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile", "-Command", "Set-Location 'c:\chronofact\frontend'; npm run dev" -WindowStyle Minimized
    Start-Sleep -Seconds 2
} else {
    Write-Host "[2/2] React / TypeScript Cockpit already active on http://localhost:5173" -ForegroundColor Green
}

# Launch browser
Start-Process "http://localhost:5173"
Write-Host "`n[ACTIVE] ChronoFact Cockpit is open at http://localhost:5173" -ForegroundColor Cyan
Write-Host "To stop services at any time, run: .\stop.bat or .\stop_chronofact.bat`n" -ForegroundColor Yellow
