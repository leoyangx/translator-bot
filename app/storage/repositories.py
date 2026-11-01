"""数据仓库：把 SQL 细节收在这里，上层只调方法。"""
from __future__ import annotations

import hashlib
import sqlite3

from .db import Database


class Repo:
    def __init__(self, db: Database) -> None:
        self.db = db

    # ---------- 全局键值 ----------
    def get_kv(self, key: str, default: str | None = None) -> str | None:
        row = self.db.execute("SELECT value FROM kv WHERE key = ?", (key,)).fetchone()
        return row["value"] if row else default

    def set_kv(self, key: str, value: str) -> None:
        self.db.execute(
            "INSERT INTO kv(key, value) VALUES(?, ?) "
            "ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )
        self.db.commit()

    # ---------- 所有者（首个使用者自动成为主人） ----------
    def get_owner(self) -> int | None:
        value = self.get_kv("owner_id")
        return int(value) if value else None

    def set_owner(self, user_id: int) -> None:
        self.set_kv("owner_id", str(user_id))

    # ---------- 翻译缓存 ----------
    def cache_get(self, key: str) -> str | None:
        row = self.db.execute("SELECT target FROM cache WHERE key = ?", (key,)).fetchone()
        return row["target"] if row else None

    def cache_put(self, key: str, source: str, target: str) -> None:
        self.db.execute(
            "INSERT OR REPLACE INTO cache(key, source, target) VALUES(?, ?, ?)",
            (key, source, target),
        )
        self.db.commit()

    def cache_count(self) -> int:
        row = self.db.execute("SELECT COUNT(*) AS c FROM cache").fetchone()
        return int(row["c"])

    def cache_trim(self, keep: int = 5000) -> None:
        """只保留最近的 N 条，防止数据库无限增长。"""
        self.db.execute(
            "DELETE FROM cache WHERE rowid NOT IN "
            "(SELECT rowid FROM cache ORDER BY created_at DESC, rowid DESC LIMIT ?)",
            (keep,),
        )
        self.db.commit()

    # ---------- 术语表 ----------
    def glossary_list(self) -> list[tuple[str, str]]:
        rows = self.db.execute(
            "SELECT source, target FROM glossary ORDER BY id"
        ).fetchall()
        return [(r["source"], r["target"]) for r in rows]

    def glossary_add(self, source: str, target: str) -> None:
        self.db.execute(
            "INSERT INTO glossary(source, target) VALUES(?, ?) "
            "ON CONFLICT(source) DO UPDATE SET target = excluded.target",
            (source.strip(), target.strip()),
        )
        self.db.commit()

    def glossary_delete(self, source: str) -> bool:
        cur = self.db.execute("DELETE FROM glossary WHERE source = ?", (source.strip(),))
        self.db.commit()
        return cur.rowcount > 0

    def glossary_signature(self) -> str:
        pairs = self.glossary_list()
        if not pairs:
            return "none"
        raw = "|".join(f"{s}={t}" for s, t in pairs)
        return hashlib.sha256(raw.encode("utf-8")).hexdigest()[:12]

    # ---------- 用量 ----------
    def usage_add(self, user_id: int, model: str, prompt: int, completion: int) -> None:
        self.db.execute(
            "INSERT INTO usage(user_id, model, prompt_tokens, completion_tokens) "
            "VALUES(?, ?, ?, ?)",
            (user_id, model, prompt, completion),
        )
        self.db.commit()

    def usage_today(self) -> dict:
        row = self.db.execute(
            "SELECT COUNT(*) AS calls, COALESCE(SUM(prompt_tokens),0) AS pt, "
            "COALESCE(SUM(completion_tokens),0) AS ct FROM usage "
            "WHERE date(created_at, 'localtime') = date('now', 'localtime')"
        ).fetchone()
        return {"calls": row["calls"], "prompt_tokens": row["pt"], "completion_tokens": row["ct"]}

    # ---------- 待处理译文（支撑按钮重新翻译） ----------
    def pending_add(self, user_id: int, text: str, src: str, tgt: str, tone: str) -> int:
        cur = self.db.execute(
            "INSERT INTO pending(user_id, text, src, tgt, tone) VALUES(?, ?, ?, ?, ?)",
            (user_id, text, src, tgt, tone),
        )
        self.db.commit()
        return int(cur.lastrowid)

    def pending_get(self, pending_id: int) -> sqlite3.Row | None:
        return self.db.execute("SELECT * FROM pending WHERE id = ?", (pending_id,)).fetchone()

    def pending_cleanup(self, keep_days: int = 7) -> None:
        self.db.execute(
            "DELETE FROM pending WHERE created_at < datetime('now', ?)",
            (f"-{keep_days} days",),
        )
        self.db.commit()
