@echo off
chcp 65001 >nul
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\enable_autostart.ps1"
if errorlevel 1 (
    echo.
    echo [!] 设置失败：请先双击「安装依赖.bat」。
    pause
    exit /b 1
)
echo.
echo 已设置开机自启，机器人已在后台运行（没有窗口是正常的）。
echo 停止：双击「停止.bat」；取消自启：双击「取消开机自启.bat」。
timeout /t 4 >nul
