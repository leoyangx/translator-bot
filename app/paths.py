"""路径解析：区分源码运行、打包后的 exe，以及宿主指定的根目录。"""
from __future__ import annotations

import os
import sys
from pathlib import Path


def app_root() -> Path:
    """可写目录：宿主可通过 TRANSLATORBOT_HOME 指定，否则取 exe/项目所在目录。"""
    env = os.environ.get("TRANSLATORBOT_HOME")
    if env:
        return Path(env)
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent.parent


def resource_path(*parts: str) -> Path:
    """只读资源：打包后位于 PyInstaller 的解包目录。"""
    bundle = getattr(sys, "_MEIPASS", None)
    base = Path(bundle) if bundle else app_root()
    return base.joinpath(*parts)
