@echo off
title ChronoFact Epistemic AI Backbone
echo ========================================================
echo   Launching ChronoFact Epistemic AI Backbone & Cockpit
echo ========================================================

:: Check if backend is already listening on port 3030
netstat -ano | findstr :3030 >nul
if %ERRORLEVEL% EQU 0 (
    echo [OK] ChronoFact API backend is already active on port 3030.
) else (
    echo [STARTING] Launching ChronoFact Core Daemon on port 3030...
    start /b "" "C:\chronofact\bin\chronofact.exe" serve --port 3030
    timeout /t 2 /nobreak >nul
)

:: Check if frontend is already listening on port 5173
netstat -ano | findstr :5173 >nul
if %ERRORLEVEL% EQU 0 (
    echo [OK] ChronoFact Cockpit frontend is already active on port 5173.
) else (
    echo [STARTING] Launching ChronoFact Cockpit on port 5173...
    cd /d "C:\chronofact\frontend"
    start /b "" npm run dev
    timeout /t 3 /nobreak >nul
)

echo [ACTIVE] ChronoFact Cockpit: http://localhost:5173
echo [ACTIVE] ChronoFact Backend: http://127.0.0.1:3030
echo [ACTIVE] ChronoFact MCP: Stdio JSON-RPC 2.0 (Integrated in Antigravity)
echo ========================================================
