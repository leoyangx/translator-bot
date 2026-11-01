"""更新检查与下载（GitHub Releases）。"""
from __future__ import annotations

import hashlib
import json
import re
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

UA = {"User-Agent": "translator-bot", "Accept": "application/vnd.github+json"}


def _parse_version(text: str) -> tuple[int, ...]:
    nums = re.findall(r"\d+", text or "")
    return tuple(int(n) for n in nums[:3]) if nums else (0,)


def is_newer(candidate: str, current: str) -> bool:
    return _parse_version(candidate) > _parse_version(current)


def check(repo: str, current: str, enabled: bool = True) -> dict:
    if not enabled:
        return {"available": False, "reason": "已关闭更新检查"}
    repo = (repo or "").strip()
    if "/" not in repo:
        return {"available": False, "reason": "未配置 GitHub 仓库（格式 owner/repo）"}

    url = f"https://api.github.com/repos/{repo}/releases/latest"
    try:
        with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=12) as resp:
            data = json.loads(resp.read())
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return {"available": False, "reason": "仓库或 Release 不存在"}
        return {"available": False, "reason": f"GitHub 返回 {exc.code}"}
    except Exception as exc:  # noqa: BLE001
        return {"available": False, "reason": f"检查失败：{exc}"}

    tag = str(data.get("tag_name") or "").lstrip("vV")
    if not tag:
        return {"available": False, "reason": "Release 没有版本号"}

    assets = data.get("assets") or []
    zip_asset = next((a for a in assets if str(a.get("name", "")).endswith(".zip")), None)
    sum_asset = next(
        (a for a in assets if "checksum" in str(a.get("name", "")).lower()), None
    )

    return {
        "available": is_newer(tag, current),
        "current": current,
        "version": tag,
        "notes": (data.get("body") or "")[:2000],
        "html_url": data.get("html_url"),
        "asset_url": zip_asset.get("browser_download_url") if zip_asset else None,
        "asset_name": zip_asset.get("name") if zip_asset else None,
        "checksums_url": sum_asset.get("browser_download_url") if sum_asset else None,
        "reason": "" if zip_asset else "Release 里没有 zip 包",
    }


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 256), b""):
            digest.update(chunk)
    return digest.hexdigest()


def download(info: dict, root: Path) -> Path:
    url = info.get("asset_url")
    if not url:
        raise RuntimeError("这个 Release 没有可下载的 zip")

    version = info.get("version") or "new"
    work = root / "data" / "update" / version
    work.mkdir(parents=True, exist_ok=True)

    zip_path = work / (info.get("asset_name") or "update.zip")
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=120) as resp:
        zip_path.write_bytes(resp.read())

    checksums_url = info.get("checksums_url")
    if checksums_url:
        with urllib.request.urlopen(
            urllib.request.Request(checksums_url, headers=UA), timeout=30
        ) as resp:
            text = resp.read().decode("utf-8", "replace")
        expected = None
        for line in text.splitlines():
            if zip_path.name in line:
                expected = line.split()[0].strip().lower()
                break
        if expected and expected != _sha256(zip_path):
            raise RuntimeError("校验失败：下载的文件不完整或被篡改")

    target = work / "files"
    if target.exists():
        import shutil

        shutil.rmtree(target, ignore_errors=True)
    with zipfile.ZipFile(zip_path) as zf:
        zf.extractall(target)
    return target
