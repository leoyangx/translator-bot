"""程序入口：根据参数启动界面、机器人或更新程序。"""
from __future__ import annotations

import sys
import traceback
from datetime import datetime
from pathlib import Path


def _log_dir() -> Path:
    base = Path(sys.executable).resolve().parent if getattr(sys, "frozen", False) else Path(__file__).resolve().parent
    return base / "logs"


def _log_crash(exc: BaseException) -> None:
    """打包版没有控制台，崩溃信息落到 logs/crash.log。"""
    try:
        log_dir = _log_dir()
        log_dir.mkdir(parents=True, exist_ok=True)
        with (log_dir / "crash.log").open("a", encoding="utf-8") as handle:
            handle.write(f"\n=== {datetime.now():%Y-%m-%d %H:%M:%S} ===\n")
            handle.write("".join(traceback.format_exception(type(exc), exc, exc.__traceback__)))
    except Exception:  # noqa: BLE001
        pass


def main() -> None:
    argv = sys.argv[1:]

    if "--run-bot" in argv:
        from app.main import main as bot_main

        bot_main()
        return

    if "--serve" in argv:
        from app.console.serve import run as serve

        serve()
        return

    # 默认也是后端模式：宿主窗口通过 --serve 启动它
    from app.console.serve import run as serve

    serve()


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except BaseException as exc:  # noqa: BLE001
        _log_crash(exc)
        raise
