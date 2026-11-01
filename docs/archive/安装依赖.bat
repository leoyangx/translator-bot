@echo off
chcp 65001 >nul
cd /d "%~dp0"
echo ============================================
echo   韩语翻译机器人 - 安装依赖
echo ============================================
echo.

if not exist ".venv\Scripts\python.exe" (
    echo [1/3] 创建运行环境...
    python -m venv .venv
    if errorlevel 1 (
        echo.
        echo [!] 没找到 Python，或者创建失败。
        echo     请先安装 Python 3.12：https://www.python.org/downloads/
        echo     安装时务必勾选 "Add python.exe to PATH"。
        pause
        exit /b 1
    )
) else (
    echo [1/3] 运行环境已存在，跳过。
)

echo [2/3] 升级 pip...
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -q --upgrade pip

echo [3/3] 安装依赖（使用国内镜像，快一些）...
".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -r requirements.txt -i https://pypi.tuna.tsinghua.edu.cn/simple
if errorlevel 1 (
    echo 镜像源失败，换官方源再试一次...
    ".venv\Scripts\python.exe" -m pip install --disable-pip-version-check -r requirements.txt
)

echo.
echo 完成。接下来双击「启动.bat」。
pause
