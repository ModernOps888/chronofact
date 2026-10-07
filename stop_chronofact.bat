@echo off
title Stop ChronoFact Services
echo ========================================================
echo   Stopping ChronoFact Backend and Cockpit Services
echo ========================================================

:: Kill processes listening on Port 3030 (Rust backend)
for /f "tokens=5" %%a in ('netstat -aon ^| findstr /R ":3030 .*LISTENING"') do (
    echo Terminating ChronoFact Backend PID: %%a ...
    taskkill /F /PID %%a >nul 2>&1
)

:: Kill processes listening on Port 5173 (Vite frontend)
for /f "tokens=5" %%a in ('netstat -aon ^| findstr /R ":5173 .*LISTENING"') do (
    echo Terminating ChronoFact Frontend PID: %%a ...
    taskkill /F /PID %%a >nul 2>&1
)

:: Finished - MCP stdio servers are preserved
echo [OK] All ChronoFact network services have been stopped cleanly.
