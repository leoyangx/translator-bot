"""本地 HTTP 服务：静态页面 + JSON 接口，只监听 127.0.0.1。"""
from __future__ import annotations

import json
import mimetypes
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from ..paths import app_root, resource_path


class Handler(BaseHTTPRequestHandler):
    api = None
    web_dir: Path = Path(".")

    def log_message(self, fmt: str, *args) -> None:  # 静默访问日志
        pass

    def _send(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def _json(self, payload: dict, status: int = 200) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self._send(status, body, "application/json; charset=utf-8")

    def do_GET(self) -> None:  # noqa: N802
        path = self.path.split("?")[0]
        if path in ("/", "/index.html"):
            target = self.web_dir / "index.html"
        elif path.startswith("/assets/"):
            target = app_root() / path.lstrip("/")
            if not target.exists():
                target = resource_path(*path.lstrip("/").split("/"))
        else:
            target = self.web_dir / path.lstrip("/")

        if not target.exists() or not target.is_file():
            self._send(404, b"not found", "text/plain; charset=utf-8")
            return

        content_type = mimetypes.guess_type(str(target))[0] or "application/octet-stream"
        if content_type.startswith("text/") or content_type in (
            "application/javascript",
            "application/json",
        ):
            content_type += "; charset=utf-8"
        self._send(200, target.read_bytes(), content_type)

    def do_POST(self) -> None:  # noqa: N802
        if not self.path.startswith("/api/"):
            self._send(404, b"not found", "text/plain; charset=utf-8")
            return
        if self.headers.get("X-Console") != "1":
            self._json({"ok": False, "message": "非法请求"}, 403)
            return

        method = self.path[len("/api/") :]
        length = int(self.headers.get("Content-Length") or 0)
        raw = self.rfile.read(length) if length else b"[]"
        try:
            args = json.loads(raw.decode("utf-8")) or []
        except json.JSONDecodeError:
            self._json({"ok": False, "message": "参数格式错误"}, 400)
            return

        func = getattr(self.api, method, None)
        if func is None or method.startswith("_"):
            self._json({"ok": False, "message": f"未知接口：{method}"}, 404)
            return

        try:
            result = func(*args) if isinstance(args, list) else func(**args)
        except Exception as exc:  # noqa: BLE001
            self._json({"ok": False, "message": f"{type(exc).__name__}: {exc}"}, 500)
            return

        if not isinstance(result, (dict, list)):
            result = {"ok": True, "result": result}
        self._json(result)


class ConsoleServer:
    def __init__(self, api) -> None:
        self.api = api
        self.httpd: ThreadingHTTPServer | None = None
        self.port = 0

    def start(self) -> int:
        handler = type("BoundHandler", (Handler,), {"api": self.api, "web_dir": resource_path("app", "console", "web")})
        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        self.port = self.httpd.server_address[1]
        threading.Thread(target=self.httpd.serve_forever, daemon=True).start()
        return self.port

    def stop(self) -> None:
        if self.httpd is not None:
            self.httpd.shutdown()
            self.httpd.server_close()
            self.httpd = None
