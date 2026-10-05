# ChronoFact Background Auto-Launcher
$backendRunning = Get-NetTCPConnection -LocalPort 3030 -ErrorAction SilentlyContinue
if (-not $backendRunning) {
    Write-Host "[STARTING] Launching ChronoFact Core Daemon on port 3030..." -ForegroundColor Cyan
    Start-Process -FilePath "C:\chronofact\bin\chronofact.exe" -ArgumentList "serve --port 3030" -WorkingDirectory "C:\chronofact" -WindowStyle Hidden
    Start-Sleep -Seconds 2
} else {
    Write-Host "[OK] ChronoFact API backend active on port 3030." -ForegroundColor Green
}

$frontendRunning = Get-NetTCPConnection -LocalPort 5173 -ErrorAction SilentlyContinue
if (-not $frontendRunning) {
    Write-Host "[STARTING] Launching ChronoFact Cockpit on port 5173..." -ForegroundColor Cyan
    Start-Process -FilePath "cmd.exe" -ArgumentList "/c npm run dev" -WorkingDirectory "C:\chronofact\frontend" -WindowStyle Hidden
    Start-Sleep -Seconds 3
} else {
    Write-Host "[OK] ChronoFact Cockpit frontend active on port 5173." -ForegroundColor Green
}

Write-Host "🚀 ChronoFact is 100% active and monitoring Antigravity IDE." -ForegroundColor Yellow
Write-Host "Cockpit: http://localhost:5173" -ForegroundColor Green
