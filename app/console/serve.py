"""无界面后端：本地 HTTP 服务 + 机器人进程管理，供原生宿主窗口调用。"""
from __future__ import annotations

import atexit
import logging
import threading
import time

from ..logging_setup import cleanup_old_logs, seconds_until_midnight, setup_logging
from ..paths import app_root
from .api import ConsoleApi
from .server import ConsoleServer

log = logging.getLogger(__name__)


def _daily_log_cleanup(root) -> None:
    """每天零点后清空 host.log 并删掉非当天日志，只保留当天。"""
    log_dir = root / "logs"
    while True:
        try:
            time.sleep(seconds_until_midnight())
            host = log_dir / "host.log"
            try:
                if host.exists():
                    host.write_text("", encoding="utf-8")
            except OSError:
                pass
            cleanup_old_logs(log_dir)
            log.info("已清理历史日志，只保留当天")
        except Exception:  # noqa: BLE001
            time.sleep(60)


def run() -> None:
    root = app_root()
    setup_logging(root / "logs", filename="app.log")
    log.info("后端启动")
    threading.Thread(target=_daily_log_cleanup, args=(root,), daemon=True).start()

    api = ConsoleApi()
    server = ConsoleServer(api)
    port = server.start()

    data_dir = root / "data"
    data_dir.mkdir(parents=True, exist_ok=True)
    port_file = data_dir / "ui.port"
    port_file.write_text(str(port), encoding="utf-8")
    atexit.register(lambda: api.bot.stop())

    # 机器人已由 Tauri 宿主（Rust/teloxide）托管，这里不再自动启动，
    # 否则两边会抢同一个 Token 导致 409 Conflict。
    log.info("后端就绪：http://127.0.0.1:%s", port)
    try:
        while True:
            time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    finally:
        try:
            api.bot.stop()
        finally:
            server.stop()
            port_file.unlink(missing_ok=True)
            log.info("后端已退出")
