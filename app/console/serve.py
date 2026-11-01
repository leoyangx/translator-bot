"""无界面后端：本地 HTTP 服务 + 机器人进程管理，供原生宿主窗口调用。"""
from __future__ import annotations

import atexit
import logging
import time

from ..logging_setup import setup_logging
from ..paths import app_root
from .api import ConsoleApi
from .server import ConsoleServer

log = logging.getLogger(__name__)


def run() -> None:
    root = app_root()
    setup_logging(root / "logs", filename="app.log")
    log.info("后端启动")

    api = ConsoleApi()
    server = ConsoleServer(api)
    port = server.start()

    data_dir = root / "data"
    data_dir.mkdir(parents=True, exist_ok=True)
    port_file = data_dir / "ui.port"
    port_file.write_text(str(port), encoding="utf-8")
    atexit.register(lambda: api.bot.stop())

    try:
        cfg = api.load_config()
        if cfg.app.autostart_bot and cfg.telegram.token and cfg.default_model:
            api.bot.start()
    except Exception:  # noqa: BLE001
        log.exception("自动启动机器人失败")

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
