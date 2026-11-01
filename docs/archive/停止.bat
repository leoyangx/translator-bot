@echo off
chcp 65001 >nul
cd /d "%~dp0"

if exist "data\supervisor.pid" (
    set /p PID=<"data\supervisor.pid"
    taskkill /PID %PID% /T /F >nul 2>&1
)

powershell -NoProfile -Command "Get-Process python,pythonw -ErrorAction SilentlyContinue | Where-Object { $_.Path -like '*\.venv\*' } | Stop-Process -Force -ErrorAction SilentlyContinue" >nul 2>&1

del "data\supervisor.pid" >nul 2>&1
echo 已停止。
timeout /t 2 >nul
