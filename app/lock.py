"""单实例锁：防止重复启动导致 Telegram 抢消息（409 冲突）。"""
from __future__ import annotations

import socket

DEFAULT_PORT = 47653


class SingleInstance:
    def __init__(self, port: int = DEFAULT_PORT) -> None:
        self._port = port
        self._sock: socket.socket | None = None

    def acquire(self) -> bool:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        try:
            sock.bind(("127.0.0.1", self._port))
            sock.listen(1)
        except OSError:
            sock.close()
            return False
        self._sock = sock
        return True

    def release(self) -> None:
        if self._sock is not None:
            self._sock.close()
            self._sock = None
