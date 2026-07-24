@echo off
setlocal EnableDelayedExpansion
cd /d "%~dp0"

set "PATH=%USERPROFILE%\.cargo\bin;C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;%PATH%"
set "LIBCLANG_PATH=C:\Program Files\LLVM\bin"
set "CMAKE_GENERATOR=Visual Studio 18 2026"
set "CMAKE_GENERATOR_INSTANCE=C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools"

REM Prefer Build Tools 2026 MSVC; fall back to Community 2026 if present.
set "MSVC_ROOT=%CMAKE_GENERATOR_INSTANCE%\VC\Tools\MSVC"
if not exist "%MSVC_ROOT%" (
  set "CMAKE_GENERATOR_INSTANCE=C:\Program Files\Microsoft Visual Studio\18\Community"
  set "MSVC_ROOT=%CMAKE_GENERATOR_INSTANCE%\VC\Tools\MSVC"
)

set "MSVC_INCLUDE="
for /d %%D in ("%MSVC_ROOT%\*") do set "MSVC_INCLUDE=%%~fD\include"

set "WINSDK_INCLUDE=C:\Program Files (x86)\Windows Kits\10\Include\10.0.26100.0"
if not exist "%MSVC_INCLUDE%\vcruntime.h" (
  echo [error] MSVC includes not found under: %MSVC_ROOT%
  pause
  exit /b 1
)

set BINDGEN_EXTRA_CLANG_ARGS=-I"%MSVC_INCLUDE%" -I"%WINSDK_INCLUDE%\ucrt" -I"%WINSDK_INCLUDE%\shared" -I"%WINSDK_INCLUDE%\um" -I"%WINSDK_INCLUDE%\winrt" -fms-compatibility -fms-extensions
set "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--disable-gpu --use-angle=swiftshader"
if not defined WEBVIEW2_USER_DATA_FOLDER set "WEBVIEW2_USER_DATA_FOLDER=%TEMP%\jarvis-webview2"

echo LIBCLANG_PATH=%LIBCLANG_PATH%
echo CMAKE_GENERATOR=%CMAKE_GENERATOR%
echo CMAKE_GENERATOR_INSTANCE=%CMAKE_GENERATOR_INSTANCE%
echo MSVC_INCLUDE=%MSVC_INCLUDE%
echo.

REM Vite on :1420 (needed because this app still uses cfg(dev) / devUrl)
netstat -ano | findstr ":1420" | findstr "LISTENING" >nul
if errorlevel 1 (
  echo Starting Vite on http://localhost:1420/ ...
  start "jarvis-vite" /MIN cmd /c "pnpm.cmd dev"
  timeout /t 2 /nobreak >nul
) else (
  echo Vite already listening on :1420
)

echo Bumping build number...
call pnpm.cmd bump-build
if errorlevel 1 (
  echo Build-number bump failed.
  pause
  exit /b 1
)

echo Building Jarvis (release — debug CRT aborts under tauri/cargo on this machine)...
pushd src-tauri
cargo build --release
set ERR=%ERRORLEVEL%
popd
if not "%ERR%"=="0" (
  echo Build failed.
  pause
  exit /b %ERR%
)

echo.
echo Launching Jarvis...
start "Jarvis" /D "%~dp0src-tauri" "%~dp0src-tauri\target\release\jarvis.exe"
echo.
echo Jarvis started.
echo If you see "Mic unavailable", enable a microphone in Windows settings.
echo.
pause
exit /b 0
