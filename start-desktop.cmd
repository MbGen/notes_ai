@echo off
setlocal
cd /d "%~dp0"
set "SLINT_BACKEND=winit-skia"
if exist "dist\notes_ai-update.exe" (
    copy /y "dist\notes_ai-update.exe" "target\x86_64-pc-windows-msvc\debug\notes_ai.exe" >nul
    if errorlevel 1 (
        echo Close Notes AI, then run this launcher again to apply the update.
        pause
        exit /b 1
    )
    del "dist\notes_ai-update.exe"
)
if not exist "target\x86_64-pc-windows-msvc\debug\notes_ai.exe" (
    echo Skia desktop build is missing. Run cargo build --target x86_64-pc-windows-msvc first.
    pause
    exit /b 1
)
"target\x86_64-pc-windows-msvc\debug\notes_ai.exe"
