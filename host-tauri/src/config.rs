//! 配置：读写 %LOCALAPPDATA%\TranslatorBot\config.yaml
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn default_base() -> String {
    "https://api.deepseek.com/v1".into()
}

fn default_model_name() -> String {
    "deepseek-chat".into()
}

fn default_mine() -> String {
    "zh".into()
}

fn default_partners() -> Vec<String> {
    vec!["ko".into(), "ja".into(), "en".into()]
}

fn default_group_mode() -> String {
    "reply_or_mention".into()
}

fn default_max_chars() -> usize {
    6000
}

fn default_history() -> usize {
    3
}

fn default_tone() -> String {
    "natural".into()
}

fn default_update_mode() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramConfig {
    pub token: String,
    pub admin_ids: Vec<i64>,
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self { token: String::new(), admin_ids: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub kind: String,
    pub default: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            base_url: default_base(),
            api_key: String::new(),
            model: default_model_name(),
            kind: "text".into(),
            default: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LanguagesConfig {
    pub mine: String,
    pub partners: Vec<String>,
}

impl Default for LanguagesConfig {
    fn default() -> Self {
        Self { mine: default_mine(), partners: default_partners() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FeaturesConfig {
    pub bilingual: bool,
    pub copy_block: bool,
    pub tone_buttons: bool,
    pub explain_button: bool,
    pub glossary: bool,
    pub context: bool,
    pub cache: bool,
    pub auto_detect: bool,
    pub group_mode: String,
    pub max_input_chars: usize,
    pub history_size: usize,
    #[serde(default = "default_tone")]
    pub default_tone: String,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            bilingual: true,
            copy_block: true,
            tone_buttons: true,
            explain_button: true,
            glossary: true,
            context: true,
            cache: true,
            auto_detect: true,
            group_mode: default_group_mode(),
            max_input_chars: default_max_chars(),
            history_size: default_history(),
            default_tone: default_tone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptsConfig {
    pub translation: String,
    pub explain: String,
}

impl Default for PromptsConfig {
    fn default() -> Self {
        Self { translation: String::new(), explain: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    pub mode: String,
    pub custom_proxy: String,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self { mode: default_update_mode(), custom_proxy: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppBehavior {
    pub close_to_tray: bool,
    pub autostart_bot: bool,
    pub check_update_on_start: bool,
}

impl Default for AppBehavior {
    fn default() -> Self {
        Self { close_to_tray: true, autostart_bot: true, check_update_on_start: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: String,
    #[serde(rename = "telegram")]
    pub telegram: TelegramConfig,
    pub models: Vec<ModelConfig>,
    pub languages: LanguagesConfig,
    pub features: FeaturesConfig,
    pub prompts: PromptsConfig,
    pub update: UpdateConfig,
    #[serde(rename = "app")]
    pub app: AppBehavior,
    pub proxy: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: APP_VERSION.to_string(),
            telegram: TelegramConfig::default(),
            models: vec![ModelConfig {
                id: "deepseek".into(),
                name: "DeepSeek".into(),
                ..Default::default()
            }],
            languages: LanguagesConfig::default(),
            features: FeaturesConfig::default(),
            prompts: PromptsConfig::default(),
            update: UpdateConfig::default(),
            app: AppBehavior::default(),
            proxy: String::new(),
        }
    }
}

impl Config {
    pub fn default_model(&self) -> Option<&ModelConfig> {
        self.models
            .iter()
            .find(|m| m.default && !m.api_key.is_empty())
            .or_else(|| self.models.iter().find(|m| !m.api_key.is_empty()))
    }

    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.telegram.token.is_empty() {
            issues.push("还没填 Telegram Bot Token".to_string());
        }
        if self.models.is_empty() {
            issues.push("还没有添加任何模型".to_string());
        } else if self.default_model().is_none() {
            issues.push("模型列表里没有可用的（需要填 API Key）".to_string());
        }
        if !super::languages::LANGUAGES.iter().any(|l| l.code == self.languages.mine) {
            issues.push(format!("我的语言无效：{}", self.languages.mine));
        }
        if self.languages.partners.is_empty() {
            issues.push("至少选一个对方语言".to_string());
        }
        issues
    }
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join("config.yaml")
}

pub fn load_config(root: &Path) -> anyhow::Result<Config> {
    let path = config_path(root);
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = fs::read_to_string(&path)?;
    let cfg: Config = serde_yaml::from_str(&text)?;
    Ok(cfg)
}

pub fn save_config(root: &Path, cfg: &Config) -> anyhow::Result<PathBuf> {
    let path = config_path(root);
    if path.exists() {
        let _ = fs::copy(&path, path.with_extension("yaml.bak"));
    }
    let text = serde_yaml::to_string(cfg)?;
    fs::write(&path, text)?;
    Ok(path)
}
