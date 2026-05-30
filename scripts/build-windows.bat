@echo off
REM RDPRemote Release Build Script for Windows
REM Usage: build-windows.bat [--target x86_64-pc-windows-msvc]

setlocal enabledelayedexpansion

set SCRIPT_DIR=%~dp0
set PROJECT_ROOT=%SCRIPT_DIR%..
cd /d "%PROJECT_ROOT%"

set TARGET=
set BUILD_TYPE=release
set OUTPUT_DIR=target\%BUILD_TYPE%

REM Parse arguments
:parse_args
if "%~1"=="" goto end_parse
if "%~1"=="--target" (
    set TARGET=%~2
    shift
    shift
    goto parse_args
)
if "%~1"=="--help" (
    echo Usage: build-windows.bat [--target ^<triple^>]
    echo.
    echo Options:
    echo   --target ^<triple^>  Build for specific target ^(e.g., x86_64-pc-windows-msvc^)
    echo   --help               Show this help message
    exit /b 0
)
echo [ERROR] Unknown option: %~1
exit /b 1
:end_parse

echo [INFO] Building RDPRemote in release mode...

REM Check for cargo
where cargo >nul 2>nul
if errorlevel 1 (
    echo [ERROR] cargo is not installed. Please install Rust first.
    echo Visit: https://rustup.rs
    exit /b 1
)

REM Show Rust version
for /f "tokens=*" %%i in ('rustc --version') do echo [INFO] Rust version: %%i

REM Build all crates in release mode
echo [INFO] Running cargo build --release...
if not "%TARGET%"=="" (
    cargo build --release --target %TARGET%
) else (
    cargo build --release
)

if errorlevel 1 (
    echo [ERROR] Build failed!
    exit /b 1
)

REM Copy binaries to output directory
echo [INFO] Copying binaries to %OUTPUT_DIR%...
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

REM Copy server binary
if exist "target\%BUILD_TYPE%\rdp-server.exe" (
    copy "target\%BUILD_TYPE%\rdp-server.exe" "%OUTPUT_DIR%\" >nul
    echo [INFO] Copied rdp-server.exe
)

REM Copy client binary
if exist "target\%BUILD_TYPE%\rdp-client.exe" (
    copy "target\%BUILD_TYPE%\rdp-client.exe" "%OUTPUT_DIR%\" >nul
    echo [INFO] Copied rdp-client.exe
)

REM Copy agent binary
if exist "target\%BUILD_TYPE%\rdp-agent.exe" (
    copy "target\%BUILD_TYPE%\rdp-agent.exe" "%OUTPUT_DIR%\" >nul
    echo [INFO] Copied rdp-agent.exe
)

REM Create release info
for /f "tokens=*" %%i in ('git describe --tags --always 2^>nul') do set RELEASE_VERSION=%%i
if "%RELEASE_VERSION%"=="" set RELEASE_VERSION=unknown

set BUILD_TIME=%date:~-4%-%date:~4,2%-%date:~7,2%T%time:~0,2%:%time:~3,2%:%time:~6,2%Z
set BUILD_TIME=%BUILD_TIME: =0%

(
echo RDPRemote Release Build
echo =======================
echo Version: %RELEASE_VERSION%
echo Build Time: %BUILD_TIME%
echo Target: %TARGET%
echo Platform: Windows
echo.
echo Binaries:
) > "%OUTPUT_DIR%\RELEASE_INFO.txt"

if exist "%OUTPUT_DIR%\rdp-server.exe" echo   - rdp-server.exe >> "%OUTPUT_DIR%\RELEASE_INFO.txt"
if exist "%OUTPUT_DIR%\rdp-client.exe" echo   - rdp-client.exe >> "%OUTPUT_DIR%\RELEASE_INFO.txt"
if exist "%OUTPUT_DIR%\rdp-agent.exe" echo   - rdp-agent.exe >> "%OUTPUT_DIR%\RELEASE_INFO.txt"

echo.
echo [INFO] Release build complete!
echo [INFO] Binaries available in: %OUTPUT_DIR%
dir /b "%OUTPUT_DIR%"

echo.
echo [INFO] To run the server: %OUTPUT_DIR%\rdp-server.exe
echo [INFO] To run the client: %OUTPUT_DIR%\rdp-client.exe

endlocal