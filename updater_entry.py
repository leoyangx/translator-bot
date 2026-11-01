"""更新程序入口（单独打包成 updater.exe）。"""
from __future__ import annotations

import sys

from app.console.updater_main import main

if __name__ == "__main__":
    sys.exit(main())
