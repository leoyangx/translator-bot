"""独立更新程序：等主程序退出 → 备份 → 替换文件 → 重启。"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import time
from pathlib import Path

SKIP = {"data", "logs", "config.yaml", "config.yaml.bak", ".venv", "__pycache__"}


def wait_for_exit(pid: int, timeout: float = 90.0) -> None:
    import ctypes

    kernel32 = ctypes.windll.kernel32  # type: ignore[attr-defined]
    handle = kernel32.OpenProcess(0x00100000, False, pid)  # SYNCHRONIZE
    if not handle:
        time.sleep(2)
        return
    try:
        kernel32.WaitForSingleObject(handle, int(timeout * 1000))
    finally:
        kernel32.CloseHandle(handle)


def copy_tree(source: Path, target: Path) -> None:
    for item in source.iterdir():
        if item.name in SKIP:
            continue
        dest = target / item.name
        if item.is_dir():
            if dest.exists():
                shutil.rmtree(dest, ignore_errors=True)
            shutil.copytree(item, dest)
        else:
            shutil.copy2(item, dest)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--exe", required=True)
    args = parser.parse_args()

    source = Path(args.source)
    target = Path(args.target)
    if not source.exists() or not target.exists():
        return 1

    wait_for_exit(args.pid)

    backup = target.parent / f"{target.name}.backup-{time.strftime('%Y%m%d-%H%M%S')}"
    try:
        shutil.copytree(target, backup, ignore=shutil.ignore_patterns("data", "logs", ".venv"))
    except Exception:  # noqa: BLE001
        backup = None

    try:
        copy_tree(source, target)
    except Exception:  # noqa: BLE001
        if backup and backup.exists():
            copy_tree(backup, target)
        return 1

    subprocess.Popen([args.exe], cwd=str(target))
    return 0


if __name__ == "__main__":
    sys.exit(main())
