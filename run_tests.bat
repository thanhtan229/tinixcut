@echo off
setlocal enabledelayedexpansion

title TinixCut - Test ^& Validation Suite

echo ========================================================================
echo   TinixCut - Test ^& Validation Suite (DEVELOPMENT_WORKFLOW.md)
echo ========================================================================
echo.

:: 1. Check prerequisites
where pnpm >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] 'pnpm' is not found in PATH.
    echo Please install pnpm or ensure it is accessible in your environment.
    goto :fail
)

where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] 'cargo' is not found in PATH.
    echo Please install Rust or ensure Cargo is accessible in your environment.
    goto :fail
)

:: 2. Parse mode
set MODE=%~1
if "%MODE%"=="" set MODE=all

set FAIL_COUNT=0
set TYPECHECK_STATUS=SKIPPED
set VITEST_STATUS=SKIPPED
set CARGO_STATUS=SKIPPED
set CLIPPY_STATUS=SKIPPED

if /i "%MODE%"=="help" goto :show_help
if /i "%MODE%"=="-h" goto :show_help
if /i "%MODE%"=="--help" goto :show_help
if /i "%MODE%"=="/?" goto :show_help

if /i "%MODE%"=="rust" goto :run_rust_only
if /i "%MODE%"=="backend" goto :run_rust_only
if /i "%MODE%"=="frontend" goto :run_frontend_only
if /i "%MODE%"=="quick" goto :run_quick
if /i "%MODE%"=="test" goto :run_quick

:: ========================================================================
:: Mode: ALL (Full validation workflow)
:: ========================================================================
echo [1/4] Running Frontend Typecheck (pnpm run typecheck)...
call pnpm run typecheck
if %ERRORLEVEL% equ 0 (
    set TYPECHECK_STATUS=PASSED
    echo [OK] Typecheck passed cleanly.
) else (
    set TYPECHECK_STATUS=FAILED
    set /a FAIL_COUNT+=1
    echo [FAIL] Typecheck failed.
)
echo.

echo [2/4] Running Frontend Unit Tests (pnpm test)...
call pnpm test
if %ERRORLEVEL% equ 0 (
    set VITEST_STATUS=PASSED
    echo [OK] Vitest passed cleanly.
) else (
    set VITEST_STATUS=FAILED
    set /a FAIL_COUNT+=1
    echo [FAIL] Vitest failed.
)
echo.

echo [3/4] Running Native Core Tests (cargo test)...
cargo test --manifest-path src-tauri/Cargo.toml
if %ERRORLEVEL% equ 0 (
    set CARGO_STATUS=PASSED
    echo [OK] Cargo tests passed cleanly.
) else (
    set CARGO_STATUS=FAILED
    set /a FAIL_COUNT+=1
    echo [FAIL] Cargo tests failed.
)
echo.

echo [4/4] Running Native Linter (cargo clippy)...
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
if %ERRORLEVEL% equ 0 (
    set CLIPPY_STATUS=PASSED
    echo [OK] Clippy passed with 0 warnings.
) else (
    set CLIPPY_STATUS=FAILED
    set /a FAIL_COUNT+=1
    echo [FAIL] Clippy found warnings or errors.
)
echo.

goto :summary

:: ========================================================================
:: Mode: QUICK (pnpm run test:all)
:: ========================================================================
:run_quick
echo Running combined test suite (pnpm run test:all)...
call pnpm run test:all
if %ERRORLEVEL% equ 0 (
    echo.
    echo ========================================================================
    echo   [SUCCESS] All targeted tests passed!
    echo ========================================================================
    exit /b 0
) else (
    echo.
    echo ========================================================================
    echo   [FAILED] Some tests failed. Check output above.
    echo ========================================================================
    exit /b 1
)

:: ========================================================================
:: Mode: RUST ONLY
:: ========================================================================
:run_rust_only
echo Running Native Core Tests (cargo test)...
cargo test --manifest-path src-tauri/Cargo.toml
if %ERRORLEVEL% equ 0 (
    echo.
    echo ========================================================================
    echo   [SUCCESS] Cargo tests passed!
    echo ========================================================================
    exit /b 0
) else (
    echo.
    echo ========================================================================
    echo   [FAILED] Cargo tests failed.
    echo ========================================================================
    exit /b 1
)

:: ========================================================================
:: Mode: FRONTEND ONLY
:: ========================================================================
:run_frontend_only
echo Running Frontend Typecheck ^& Vitest...
call pnpm run typecheck
if %ERRORLEVEL% neq 0 (
    echo [FAIL] Typecheck failed.
    exit /b 1
)
call pnpm test
if %ERRORLEVEL% equ 0 (
    echo.
    echo ========================================================================
    echo   [SUCCESS] Frontend checks passed!
    echo ========================================================================
    exit /b 0
) else (
    echo.
    echo ========================================================================
    echo   [FAILED] Vitest failed.
    echo ========================================================================
    exit /b 1
)

:: ========================================================================
:: Final Summary
:: ========================================================================
:summary
echo ========================================================================
echo                         VALIDATION SUMMARY
echo ========================================================================
echo   Frontend Typecheck (tsc) : %TYPECHECK_STATUS%
echo   Frontend Tests (vitest)  : %VITEST_STATUS%
echo   Native Core (cargo test) : %CARGO_STATUS%
echo   Linter (cargo clippy)    : %CLIPPY_STATUS%
echo ========================================================================

if %FAIL_COUNT% equ 0 (
    echo   RESULT: ALL CHECKS PASSED [100%% - All 4 Checks Green]
    echo ========================================================================
    exit /b 0
) else (
    echo   RESULT: %FAIL_COUNT% CHECKS FAILED.
    echo ========================================================================
    exit /b 1
)

:show_help
echo Usage: run_tests.bat [option]
echo.
echo Options:
echo   (no option) / all  : Run complete validation suite (Typecheck, Vitest, Cargo Test, Clippy)
echo   quick / test       : Run fast test suite (pnpm run test:all)
echo   rust / backend     : Run native Rust unit and integration tests (cargo test)
echo   frontend           : Run frontend typecheck and Vitest
echo   help / /?          : Show this help message
echo.
exit /b 0

:fail
exit /b 1
