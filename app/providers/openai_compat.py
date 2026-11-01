"""OpenAI 兼容实现（DeepSeek / 通义 / 智谱 / Ollama / OpenRouter 通用）。"""
from __future__ import annotations

import asyncio
import logging

import aiohttp

from .base import ChatResult

log = logging.getLogger(__name__)

_RETRY_STATUS = {408, 409, 429, 500, 502, 503, 504}


class ProviderError(RuntimeError):
    """模型调用失败，message 直接展示给用户。"""


class OpenAICompatProvider:
    def __init__(
        self,
        base_url: str,
        api_key: str,
        default_model: str,
        *,
        timeout: int = 60,
        proxy: str | None = None,
        max_retries: int = 2,
    ) -> None:
        self._base = base_url.rstrip("/")
        self._api_key = api_key
        self._default_model = default_model
        self._timeout = timeout
        self._proxy = proxy or None
        self._max_retries = max_retries
        self._session: aiohttp.ClientSession | None = None

    async def _get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=self._timeout)
            )
        return self._session

    async def close(self) -> None:
        if self._session and not self._session.closed:
            await self._session.close()

    async def models(self) -> list[str]:
        session = await self._get_session()
        async with session.get(
            f"{self._base}/models", headers=self._headers()
        ) as resp:
            if resp.status != 200:
                raise ProviderError(f"读取模型列表失败（HTTP {resp.status}）")
            data = await resp.json(content_type=None)
        return [item.get("id", "") for item in data.get("data", [])]

    def _headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self._api_key}",
            "Content-Type": "application/json",
        }

    async def chat(
        self,
        messages: list[dict],
        *,
        model: str | None = None,
        temperature: float = 0.3,
        max_tokens: int | None = None,
    ) -> ChatResult:
        payload: dict = {
            "model": model or self._default_model,
            "messages": messages,
            "temperature": temperature,
            "stream": False,
        }
        if max_tokens:
            payload["max_tokens"] = max_tokens

        last_error: Exception | None = None
        for attempt in range(self._max_retries + 1):
            try:
                return await self._post_chat(payload)
            except ProviderError as exc:
                if not getattr(exc, "retryable", False) or attempt == self._max_retries:
                    raise
                last_error = exc
            except (aiohttp.ClientError, asyncio.TimeoutError) as exc:
                last_error = exc
                if attempt == self._max_retries:
                    raise ProviderError("模型服务连接失败，请检查网络或代理设置") from exc
            await asyncio.sleep(1.5 * (attempt + 1))

        raise ProviderError(f"模型调用失败：{last_error}")

    async def _post_chat(self, payload: dict) -> ChatResult:
        session = await self._get_session()
        async with session.post(
            f"{self._base}/chat/completions",
            headers=self._headers(),
            json=payload,
            proxy=self._proxy,
        ) as resp:
            if resp.status != 200:
                body = (await resp.text())[:300]
                err = ProviderError(_humanize(resp.status, body))
                err.retryable = resp.status in _RETRY_STATUS  # type: ignore[attr-defined]
                raise err
            data = await resp.json(content_type=None)

        try:
            content = data["choices"][0]["message"]["content"] or ""
        except (KeyError, IndexError) as exc:
            raise ProviderError("模型返回格式异常") from exc

        usage = data.get("usage") or {}
        return ChatResult(
            text=content.strip(),
            prompt_tokens=int(usage.get("prompt_tokens") or 0),
            completion_tokens=int(usage.get("completion_tokens") or 0),
        )


def _humanize(status: int, body: str) -> str:
    if status == 401:
        return "模型 Key 无效或已过期（HTTP 401）"
    if status == 402:
        return "模型账户余额不足（HTTP 402），请充值"
    if status == 429:
        return "请求太频繁，已被限流（HTTP 429），稍后再试"
    if status in (500, 502, 503, 504):
        return f"模型服务暂时不可用（HTTP {status}），稍后再试"
    return f"模型调用失败（HTTP {status}）：{body}"
