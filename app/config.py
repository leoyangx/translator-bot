"""配置：models / languages / features / prompts / update。"""
from __future__ import annotations

import shutil
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import yaml

from .core.languages import LANGUAGES

APP_VERSION = "0.1.0-rc.4"
DEFAULT_CONFIG_PATH = "config.yaml"


class ConfigError(RuntimeError):
    pass


@dataclass(slots=True)
class TelegramConfig:
    token: str = ""
    admin_ids: list[int] = field(default_factory=list)


@dataclass(slots=True)
class ModelConfig:
    id: str = ""
    name: str = ""
    base_url: str = "https://api.deepseek.com/v1"
    api_key: str = ""
    model: str = "deepseek-v4-pro"
    kind: str = "text"
    default: bool = False


@dataclass(slots=True)
class LanguagesConfig:
    mine: str = "zh"
    partners: list[str] = field(default_factory=lambda: ["ko", "ja", "en"])


@dataclass(slots=True)
class FeaturesConfig:
    bilingual: bool = True
    copy_block: bool = True
    tone_buttons: bool = True
    explain_button: bool = True
    glossary: bool = True
    context: bool = True
    cache: bool = True
    auto_detect: bool = True
    group_mode: str = "reply_or_mention"
    max_input_chars: int = 6000
    history_size: int = 3


@dataclass(slots=True)
class PromptsConfig:
    translation: str = ""
    explain: str = ""


@dataclass(slots=True)
class UpdateConfig:
    mode: str = "auto"        # auto | direct | custom
    custom_proxy: str = ""


@dataclass(slots=True)
class AppConfig:
    """程序自身的行为设置。"""

    close_to_tray: bool = True
    autostart_bot: bool = True
    check_update_on_start: bool = True


@dataclass(slots=True)
class Config:
    version: str = APP_VERSION
    telegram: TelegramConfig = field(default_factory=TelegramConfig)
    models: list[ModelConfig] = field(default_factory=list)
    languages: LanguagesConfig = field(default_factory=LanguagesConfig)
    features: FeaturesConfig = field(default_factory=FeaturesConfig)
    prompts: PromptsConfig = field(default_factory=PromptsConfig)
    update: UpdateConfig = field(default_factory=UpdateConfig)
    app: AppConfig = field(default_factory=AppConfig)
    proxy: str = ""

    @property
    def default_model(self) -> ModelConfig | None:
        for model in self.models:
            if model.default and model.api_key:
                return model
        for model in self.models:
            if model.api_key:
                return model
        return None

    def validate(self) -> list[str]:
        issues: list[str] = []
        if not self.telegram.token:
            issues.append("还没填 Telegram Bot Token")
        if not self.models:
            issues.append("还没有添加任何模型")
        elif not self.default_model:
            issues.append("模型列表里没有可用的（需要填 API Key）")
        if self.languages.mine not in LANGUAGES:
            issues.append(f"我的语言无效：{self.languages.mine}")
        for code in self.languages.partners:
            if code not in LANGUAGES:
                issues.append(f"对方语言无效：{code}")
        if not self.languages.partners:
            issues.append("至少选一个对方语言")
        if self.features.max_input_chars < 100:
            issues.append("单次字数上限太小")
        return issues


def _model_from_dict(raw: dict[str, Any], index: int) -> ModelConfig:
    return ModelConfig(
        id=str(raw.get("id") or f"model-{index + 1}"),
        name=str(raw.get("name") or raw.get("model") or f"模型 {index + 1}"),
        base_url=str(raw.get("base_url") or "https://api.deepseek.com/v1"),
        api_key=str(raw.get("api_key") or ""),
        model=str(raw.get("model") or "deepseek-v4-pro"),
        kind=str(raw.get("kind") or "text"),
        default=bool(raw.get("default", index == 0)),
    )


def config_from_dict(raw: dict[str, Any]) -> Config:
    tg_raw = raw.get("telegram") or {}
    models_raw = raw.get("models") or []

    # 兼容 0.0.1 之前的 llm 单模型配置
    if not models_raw and isinstance(raw.get("llm"), dict):
        llm = raw["llm"]
        models_raw = [
            {
                "id": "default",
                "name": llm.get("model") or "默认模型",
                "base_url": llm.get("base_url"),
                "api_key": llm.get("api_key"),
                "model": llm.get("model"),
                "default": True,
            }
        ]

    lang_raw = raw.get("languages") or {}
    feat_raw = raw.get("features") or {}
    legacy_tr = raw.get("translation") or {}
    prompt_raw = raw.get("prompts") or {}
    update_raw = raw.get("update") or {}
    app_raw = raw.get("app") or {}

    cfg = Config(
        telegram=TelegramConfig(
            token=str(tg_raw.get("token") or ""),
            admin_ids=[int(v) for v in (tg_raw.get("admin_ids") or [])],
        ),
        models=[_model_from_dict(m, i) for i, m in enumerate(models_raw)],
        languages=LanguagesConfig(
            mine=str(lang_raw.get("mine") or "zh"),
            partners=[str(v) for v in (lang_raw.get("partners") or ["ko", "ja", "en"])],
        ),
        features=FeaturesConfig(
            bilingual=bool(feat_raw.get("bilingual", legacy_tr.get("bilingual", True))),
            copy_block=bool(feat_raw.get("copy_block", True)),
            tone_buttons=bool(feat_raw.get("tone_buttons", True)),
            explain_button=bool(feat_raw.get("explain_button", True)),
            glossary=bool(feat_raw.get("glossary", True)),
            context=bool(feat_raw.get("context", True)),
            cache=bool(feat_raw.get("cache", True)),
            auto_detect=bool(feat_raw.get("auto_detect", True)),
            group_mode=str(feat_raw.get("group_mode") or "reply_or_mention"),
            max_input_chars=int(
                feat_raw.get("max_input_chars", legacy_tr.get("max_input_chars", 6000))
            ),
            history_size=int(
                feat_raw.get("history_size", legacy_tr.get("history_size", 3))
            ),
        ),
        prompts=PromptsConfig(
            translation=str(prompt_raw.get("translation") or ""),
            explain=str(prompt_raw.get("explain") or ""),
        ),
        update=UpdateConfig(
            mode=str(update_raw.get("mode") or "auto"),
            custom_proxy=str(update_raw.get("custom_proxy") or ""),
        ),
        app=AppConfig(
            close_to_tray=bool(app_raw.get("close_to_tray", True)),
            autostart_bot=bool(app_raw.get("autostart_bot", True)),
            check_update_on_start=bool(app_raw.get("check_update_on_start", True)),
        ),
        proxy=str(raw.get("proxy") or ""),
    )
    return cfg


def config_to_dict(cfg: Config) -> dict[str, Any]:
    data = asdict(cfg)
    data["version"] = cfg.version
    return data


def config_path(path: str | Path | None = None) -> Path:
    from .paths import app_root

    return Path(path) if path else app_root() / DEFAULT_CONFIG_PATH


def load_config(path: str | Path | None = None) -> Config:
    target = config_path(path)
    if not target.exists():
        raise ConfigError(f"找不到配置文件：{target}")
    raw = yaml.safe_load(target.read_text(encoding="utf-8")) or {}
    return config_from_dict(raw)


def save_config(cfg: Config, path: str | Path | None = None) -> Path:
    target = config_path(path)
    if target.exists():
        backup = target.with_suffix(".yaml.bak")
        try:
            shutil.copyfile(target, backup)
        except OSError:
            pass
    text = yaml.safe_dump(
        config_to_dict(cfg), allow_unicode=True, sort_keys=False, default_flow_style=False
    )
    target.write_text(text, encoding="utf-8")
    return target


def ensure_config(path: str | Path | None = None) -> Config:
    """首次启动时生成一份可编辑的空配置。"""
    target = config_path(path)
    if target.exists():
        return load_config(target)
    cfg = Config(
        models=[
            ModelConfig(
                id="deepseek",
                name="DeepSeek",
                base_url="https://api.deepseek.com/v1",
                api_key="",
                model="deepseek-v4-pro",
                default=True,
            )
        ]
    )
    save_config(cfg, target)
    return cfg
