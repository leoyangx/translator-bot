"""守护进程：崩溃后自动重启，双击 start_hidden.vbs 时无窗口运行。"""
from __future__ import annotations

import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
LOGS = ROOT / "logs"
PID_FILE = DATA / "supervisor.pid"
LOG_FILE = LOGS / "supervisor.log"

FAST_EXIT_SECONDS = 8
MAX_FAST_FAILURES = 3


def log(message: str) -> None:
    LOGS.mkdir(parents=True, exist_ok=True)
    stamp = time.strftime("%Y-%m-%d %H:%M:%S")
    with LOG_FILE.open("a", encoding="utf-8") as handle:
        handle.write(f"{stamp} {message}\n")


def main() -> None:
    DATA.mkdir(parents=True, exist_ok=True)
    PID_FILE.write_text(str(os.getpid()), encoding="utf-8")
    log("守护进程启动")

    delay = 3
    fast_failures = 0

    while True:
        started = time.monotonic()
        log("启动机器人子进程")
        proc = subprocess.Popen(
            [sys.executable, "-m", "app.main"],
            cwd=str(ROOT),
        )
        code = proc.wait()
        elapsed = time.monotonic() - started
        log(f"子进程退出 code={code} 运行 {elapsed:.0f}s")

        if elapsed < FAST_EXIT_SECONDS:
            fast_failures += 1
            if fast_failures >= MAX_FAST_FAILURES:
                log("连续快速退出，停止重启。请查看 logs/bot.log 排查配置或网络问题。")
                break
        else:
            fast_failures = 0
            delay = 3

        log(f"{delay}s 后重启")
        time.sleep(delay)
        delay = min(delay * 2, 60)

    # 只清理自己写的 PID，避免误删另一个实例的记录
    try:
        if PID_FILE.read_text(encoding="utf-8").strip() == str(os.getpid()):
            PID_FILE.unlink(missing_ok=True)
    except FileNotFoundError:
        pass


if __name__ == "__main__":
    main()
