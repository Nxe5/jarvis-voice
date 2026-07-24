@echo off
REM Check if all dependencies are installed

echo ==========================================
echo Jarvis Voice - Dependency Check
echo ==========================================
echo.

echo Checking Node.js...
node --version 2>nul
if %errorlevel% neq 0 (
    echo [MISSING] Node.js not found
    echo Install from https://nodejs.org/ or run: winget install OpenJS.NodeJS
) else (
    echo [OK] Node.js found
)
echo.

echo Checking pnpm...
pnpm --version 2>nul
if %errorlevel% neq 0 (
    echo [MISSING] pnpm not found
    echo Install with: npm install -g pnpm
) else (
    echo [OK] pnpm found
)
echo.

echo Checking Rust...
cargo --version 2>nul
if %errorlevel% neq 0 (
    echo [MISSING] Rust/Cargo not found
    echo Install from https://rustup.rs/
) else (
    echo [OK] Rust found
)
echo.

echo Checking Git...
git --version 2>nul
if %errorlevel% neq 0 (
    echo [MISSING] Git not found
    echo Install with: winget install Git.Git
) else (
    echo [OK] Git found
)
echo.

echo Checking model file...
if exist "src-tauri\models\ggml-base.en.bin" (
    echo [OK] Whisper model found
) else (
    echo [MISSING] Whisper model not found
    echo Run: pnpm fetch-models
)
echo.

echo ==========================================
echo.

pause
