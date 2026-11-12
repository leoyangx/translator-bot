//! 翻译编排：清洗输出、缓存、拼提示词。
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::db::Db;
use crate::prompts::{build_explain_messages, build_messages};
use crate::provider::Provider;

const MAX_GLOSSARY: usize = 80;

pub struct Translator<'a> {
    pub provider: &'a Provider,
    pub db: &'a Db,
    pub cfg: &'a Config,
    pub model: String,
}

pub fn clean_output(text: &str) -> String {
    let mut out = text.trim().to_string();

    // 去掉 ``` 代码块包裹
    if out.starts_with("```") {
        out = out.trim_start_matches("```").to_string();
        if let Some(pos) = out.find('\n') {
            out = out[pos + 1..].to_string();
        }
        if out.ends_with("```") {
            out = out[..out.len() - 3].to_string();
        }
        out = out.trim().to_string();
    }

    // 去掉 ** 加粗与反引号
    out = out.replace("**", "").replace('`', "");

    // 去掉「译文：」之类前缀
    for prefix in ["译文：", "译文:", "翻译结果：", "翻译：", "翻译:"] {
        if let Some(rest) = out.strip_prefix(prefix) {
            out = rest.trim().to_string();
        }
    }

    // 去掉整体包裹的引号
    for (left, right) in [("\"", "\""), ("“", "”"), ("「", "」"), ("『", "』")] {
        if out.len() > 1 && out.starts_with(left) && out.ends_with(right) {
            out = out[left.len()..out.len() - right.len()].trim().to_string();
        }
    }
    out
}

impl<'a> Translator<'a> {
    fn cache_key(&self, text: &str, src: &str, tgt: &str, tone: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(
            format!(
                "v3|{}|{}|{}>{}|{}|{}",
                self.model,
                tone,
                src,
                tgt,
                self.cfg.prompts.translation,
                text
            )
            .as_bytes(),
        );
        format!("{:x}", hasher.finalize())
    }

    pub async fn translate(
        &self,
        text: &str,
        src: &str,
        tgt: &str,
        tone: &str,
    ) -> anyhow::Result<(String, bool)> {
        let text = text.trim();
        if text.is_empty() {
            anyhow::bail!("内容为空");
        }
        if text.chars().count() > self.cfg.features.max_input_chars {
            anyhow::bail!(
                "文本太长（{} 字符），请分段发送，每段不超过 {} 字符",
                text.chars().count(),
                self.cfg.features.max_input_chars
            );
        }

        let key = self.cache_key(text, src, tgt, tone);
        if self.cfg.features.cache {
            if let Some(hit) = self.db.cache_get(&key) {
                return Ok((hit, true));
            }
        }

        let glossary = if self.cfg.features.glossary {
            self.db.glossary()
        } else {
            Vec::new()
        };
        let messages = build_messages(
            text,
            src,
            tgt,
            tone,
            &glossary[..glossary.len().min(MAX_GLOSSARY)],
            &[],
            &self.cfg.prompts.translation,
        );
        let result = self
            .provider
            .chat(&messages, &self.model, 0.3, None)
            .await?;
        let output = clean_output(&result.text);
        if output.is_empty() {
            anyhow::bail!("模型返回了空译文，请重试");
        }
        if self.cfg.features.cache {
            self.db.cache_put(&key, &output);
        }
        Ok((output, false))
    }

    pub async fn explain(&self, text: &str) -> anyhow::Result<String> {
        let messages = build_explain_messages(text.trim(), &self.cfg.prompts.explain);
        let result = self.provider.chat(&messages, &self.model, 0.4, None).await?;
        Ok(result.text)
    }
}

impl<'a> Translator<'a> {
    /// 流式翻译：缓存命中时一次性回调；否则边生成边回调
    pub async fn translate_stream<F>(
        &self,
        text: &str,
        src: &str,
        tgt: &str,
        tone: &str,
        mut on_delta: F,
    ) -> anyhow::Result<(String, bool)>
    where
        F: FnMut(&str),
    {
        let text = text.trim();
        if text.is_empty() {
            anyhow::bail!("内容为空");
        }
        if text.chars().count() > self.cfg.features.max_input_chars {
            anyhow::bail!(
                "文本太长（{} 字符），请分段翻译",
                text.chars().count()
            );
        }
        let key = self.cache_key(text, src, tgt, tone);
        if self.cfg.features.cache {
            if let Some(hit) = self.db.cache_get(&key) {
                on_delta(&hit);
                return Ok((hit, true));
            }
        }
        let glossary = if self.cfg.features.glossary {
            self.db.glossary()
        } else {
            Vec::new()
        };
        let messages = build_messages(
            text,
            src,
            tgt,
            tone,
            &glossary[..glossary.len().min(MAX_GLOSSARY)],
            &[],
            &self.cfg.prompts.translation,
        );
        let raw = self
            .provider
            .chat_stream(&messages, &self.model, 0.3, |delta| on_delta(delta))
            .await?;
        let output = clean_output(&raw);
        if output.is_empty() {
            anyhow::bail!("模型返回了空译文，请重试");
        }
        if self.cfg.features.cache {
            self.db.cache_put(&key, &output);
        }
        Ok((output, false))
    }
}
