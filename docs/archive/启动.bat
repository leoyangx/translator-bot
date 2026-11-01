@echo off
chcp 65001 >nul
cd /d "%~dp0"

if not exist ".venv\Scripts\pythonw.exe" (
    echo [!] 还没装依赖，请先双击「安装依赖.bat」
    pause
    exit /b 1
)

start "" ".venv\Scripts\pythonw.exe" -m app.supervisor
echo 机器人已在后台启动（没有窗口是正常的）。
echo.
echo 到 Telegram 里给它发 /doctor 就能确认在线。
timeout /t 3 >nul
