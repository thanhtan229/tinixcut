@echo off
setlocal enabledelayedexpansion

title TinixCut - Desktop Video Editor (Dev Runtime)

echo ========================================================================
echo   TinixCut - Launching Desktop Application (Tauri + React + Rust)
echo ========================================================================
echo.

:: 1. Check prerequisites
where pnpm >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] 'pnpm' is not found in PATH.
    echo Please install pnpm or ensure it is accessible in your environment.
    pause
    exit /b 1
)

where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] 'cargo' is not found in PATH.
    echo Please install Rust or ensure Cargo is accessible in your environment.
    pause
    exit /b 1
)

echo Starting Vite frontend server and Tauri desktop window...
echo (First build may take a minute to compile native Rust crates)
echo.

call pnpm tauri dev

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Application failed to launch or exited with an error.
    pause
)
