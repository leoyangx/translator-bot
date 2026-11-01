@echo off
chcp 65001 >nul
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\disable_autostart.ps1"
echo.
echo 已取消开机自启（正在运行的机器人不受影响）。
timeout /t 3 >nul
