@echo off
REM Debug script with required build env (LLVM / CMake / MSVC includes)

setlocal EnableDelayedExpansion
cd /d "%~dp0"

set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;%PATH%"
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
set "CMAKE_GENERATOR=Visual Studio 18 2026"
set "CMAKE_GENERATOR_INSTANCE=C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools"

set "MSVC_ROOT=%CMAKE_GENERATOR_INSTANCE%\VC\Tools\MSVC"
if not exist "%MSVC_ROOT%" (
  set "CMAKE_GENERATOR_INSTANCE=C:\Program Files\Microsoft Visual Studio\18\Community"
  set "MSVC_ROOT=%CMAKE_GENERATOR_INSTANCE%\VC\Tools\MSVC"
)

set "MSVC_INCLUDE="
for /d %%D in ("%MSVC_ROOT%\*") do set "MSVC_INCLUDE=%%~fD\include"

set "WINSDK_INCLUDE=C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0"
set BINDGEN_EXTRA_CLANG_ARGS=-I"%MSVC_INCLUDE%" -I"%WINSDK_INCLUDE%\ucrt" -I"%WINSDK_INCLUDE%\shared" -I"%WINSDK_INCLUDE%\um" -I"%WINSDK_INCLUDE%\winrt" -fms-compatibility -fms-extensions
set "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu --use-angle=swiftshader"

echo Checking prerequisites...
echo.
node --version
pnpm.cmd --version
cargo --version
cmake --version
echo LIBCLANG_PATH=%LIBCLANG_PATH%
echo CMAKE_GENERATOR=%CMAKE_GENERATOR%
echo CMAKE_GENERATOR_INSTANCE=%CMAKE_GENERATOR_INSTANCE%
echo MSVC_INCLUDE=%MSVC_INCLUDE%
echo.

if not exist "%MSVC_INCLUDE%\vcruntime.h" (
  echo [error] vcruntime.h missing at %MSVC_INCLUDE%
  pause
  exit /b 1
)

if not exist "src-tauri\models\ggml-base.en.bin" (
  echo Whisper model missing — fetching...
  pnpm.cmd fetch-models
  echo.
)

set RUST_BACKTRACE=1
pnpm.cmd tauri dev 2>&1

echo.
echo Process exited with code: %errorlevel%
echo.
pause
