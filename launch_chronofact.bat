@echo off
setlocal enabledelayedexpansion
title ChronoFact 4-Pillar Epistemic AI Backbone
color 0B

echo =======================================================================
echo     CHRONOFACT: 4-PILLAR EPISTEMIC AI BACKBONE & COCKPIT LAUNCHER
echo =======================================================================
echo.

cd /d "C:\chronofact"

:: Verify executable
if not exist "bin\chronofact.exe" (
    echo [INFO] bin\chronofact.exe not found. Checking target\release...
    if exist "target\release\chronofact.exe" (
        copy /Y "target\release\chronofact.exe" "bin\chronofact.exe" >nul
    ) else (
        echo [INFO] Compiling release binary...
        cargo build --release
        copy /Y "target\release\chronofact.exe" "bin\chronofact.exe" >nul
    )
)

:: 1. Launch Rust Backend on Port 3030
netstat -ano | findstr /R ":3030 .*LISTENING" >nul
if %ERRORLEVEL% EQU 0 (
    echo [OK] ChronoFact Backend already active on http://127.0.0.1:3030
) else (
    echo [1/2] Starting Rust Backend on http://127.0.0.1:3030 ...
    start "ChronoFact Core Backend" /min "C:\chronofact\bin\chronofact.exe" serve --port 3030
    timeout /t 2 /nobreak >nul
)

:: 2. Launch Vite Frontend Cockpit on Port 5173
netstat -ano | findstr /R ":5173 .*LISTENING" >nul
if %ERRORLEVEL% EQU 0 (
    echo [OK] ChronoFact Cockpit already active on http://localhost:5173
) else (
    echo [2/2] Starting React 19 Cockpit on http://localhost:5173 ...
    cd /d "C:\chronofact\frontend"
    start "ChronoFact Cockpit Frontend" /min cmd /c "npm run dev"
    cd /d "C:\chronofact"
    timeout /t 3 /nobreak >nul
)

:: 3. Automatically launch browser
echo.
echo [BROWSER] Opening ChronoFact Cockpit in your default browser...
start http://localhost:5173

echo.
echo =======================================================================
echo   STATUS: ONLINE AND READY
echo   Cockpit UI:  http://localhost:5173
echo   REST API:    http://127.0.0.1:3030
echo   MCP Server:  C:\chronofact\bin\chronofact.exe mcp
echo =======================================================================
echo.
echo [COMMANDS]
echo   [O] Open Cockpit UI in browser
echo   [K] Kill and stop all ChronoFact services
echo   [Q] Quit launcher (services remain running in background)
echo.

:MENU
set "CHOICE="
set /p CHOICE="Choose an option [O/K/Q]: "
if /i "%CHOICE%"=="O" (
    start http://localhost:5173
    goto MENU
)
if /i "%CHOICE%"=="K" (
    call "C:\chronofact\stop_chronofact.bat"
    echo.
    echo Services terminated. Exiting in 3 seconds...
    timeout /t 3 /nobreak >nul
    exit /b
)
if /i "%CHOICE%"=="Q" (
    exit /b
)
goto MENU
