@echo off
cd /d "%~dp0"
where cargo >nul 2>&1 || (
  echo Rust is not on this PC's PATH yet. If you just installed it, close this window, then run build.cmd again.
  echo If that doesn't help, install it from https://rustup.rs and accept the defaults ^(it may also install Visual Studio Build Tools; let it^).
  pause
  exit /b 1
)
echo Testing and building, this takes a minute or two the first time...
cargo test
if not errorlevel 1 cargo build --release
if errorlevel 1 (
  echo.
  echo BUILD FAILED - copy the red text above and paste it to Claude.
  pause
  exit /b 1
)
copy /y target\release\roam.exe roam.exe >nul
echo.
echo Done: roam.exe is in this folder. Copy it to the other PC too.
pause
