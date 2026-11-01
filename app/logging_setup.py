"""日志：控制台 + 滚动文件，统一 UTF-8，避免 Windows 控制台乱码。"""
from __future__ import annotations

import logging
import sys
from logging.handlers import RotatingFileHandler
from pathlib import Path

_FORMAT = "%(asctime)s %(levelname)-7s %(name)-18s %(message)s"


def force_utf8_stdio() -> None:
    for stream in (sys.stdout, sys.stderr):
        if stream is None:  # pythonw 下没有控制台
            continue
        try:
            if (stream.encoding or "").lower() not in ("utf-8", "utf8"):
                stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:  # 某些环境下 stream 不可重配置，忽略
            pass


def setup_logging(log_dir: Path, level: int = logging.INFO, filename: str = "bot.log") -> None:
    log_dir.mkdir(parents=True, exist_ok=True)

    root = logging.getLogger()
    root.setLevel(level)
    root.handlers.clear()

    formatter = logging.Formatter(_FORMAT)

    file_handler = RotatingFileHandler(
        log_dir / filename, maxBytes=5 * 1024 * 1024, backupCount=5, encoding="utf-8"
    )
    file_handler.setFormatter(formatter)
    root.addHandler(file_handler)

    console = None
    if sys.stdout is not None:
        console = logging.StreamHandler(sys.stdout)
        console.setFormatter(formatter)
        root.addHandler(console)

    logging.getLogger("aiogram").setLevel(logging.WARNING)
    logging.getLogger("aiohttp").setLevel(logging.WARNING)
