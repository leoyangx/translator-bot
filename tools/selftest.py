"""离线自测：语种识别、语向推断、翻译管线、装配。"""
from __future__ import annotations

import asyncio
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

from app.bot.app import build_dispatcher  # noqa: E402
from app.bot.context import ContextStore  # noqa: E402
from app.config import load_config  # noqa: E402
from app.core.languages import detect_language, resolve_direction  # noqa: E402
from app.core.translator import Translator  # noqa: E402
from app.logging_setup import force_utf8_stdio, setup_logging  # noqa: E402
from app.providers.openai_compat import OpenAICompatProvider  # noqa: E402
from app.storage.db import Database  # noqa: E402
from app.storage.repositories import Repo  # noqa: E402

SAMPLES = [
    "내일 오후 3시에 시간 괜찮으세요? 회의실은 5층으로 잡아뒀어요.",
    "明天下午三点方便吗？我在五楼订了会议室。",
    "Could you send me the invoice by Friday?",
    "明日の午後、打ち合わせは可能でしょうか。",
    "이거 진짜 대박이다 ㅋㅋㅋ",
]


async def main() -> None:
    force_utf8_stdio()
    cfg = load_config()
    setup_logging(ROOT / "logs")

    model = cfg.default_model
    assert model is not None, "没有默认模型"

    db = Database(ROOT / "data" / "bot.db")
    db.connect()
    repo = Repo(db)

    provider = OpenAICompatProvider(model.base_url, model.api_key, model.model, timeout=60)
    translator = Translator(
        provider,
        repo,
        model=model.model,
        max_input_chars=cfg.features.max_input_chars,
        prompt_template=cfg.prompts.translation,
        explain_template=cfg.prompts.explain,
    )
    dp = build_dispatcher(repo, cfg.telegram.admin_ids)
    print(f"装配 OK：路由 {len(dp.sub_routers)} 个")

    print("\n== 语种识别 ==")
    for sample in SAMPLES:
        detected = detect_language(sample)
        src, tgt, _ = resolve_direction(
            sample, cfg.languages.mine, cfg.languages.partners
        )
        print(f"  {detected!s:4} {src}->{tgt}  {sample[:28]}")

    print("\n== 翻译 ==")
    for sample in SAMPLES[:3]:
        src, tgt, _ = resolve_direction(
            sample, cfg.languages.mine, cfg.languages.partners
        )
        result = await translator.translate(sample, src=src, tgt=tgt)
        print(f"\n[{result.src_lang}->{result.tgt_lang}] {result.source_text}")
        print(f"  → {result.target_text}")

    await provider.close()
    db.close()


if __name__ == "__main__":
    asyncio.run(main())
