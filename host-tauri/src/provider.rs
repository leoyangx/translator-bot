//! OpenAI 兼容接口：模型列表 + 对话，带重试与中文错误。
use std::time::Duration;

use serde_json::{json, Value};

use crate::prompts::Message;

#[derive(Debug, Clone)]
pub struct ChatResult {
    pub text: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

pub struct Provider {
    base: String,
    api_key: String,
    client: reqwest::Client,
}

impl Provider {
    pub fn new(base_url: &str, api_key: &str, proxy: &str, timeout_secs: u64) -> anyhow::Result<Self> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .user_agent("TranslatorBot/0.1");
        if !proxy.trim().is_empty() {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        }
        Ok(Self {
            base: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: builder.build()?,
        })
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut map = reqwest::header::HeaderMap::new();
        map.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.api_key).parse().unwrap(),
        );
        map.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        map
    }

    pub async fn models(&self) -> anyhow::Result<Vec<String>> {
        let resp = self
            .client
            .get(format!("{}/models", self.base))
            .headers(self.headers())
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("{}", humanize(status.as_u16(), &body));
        }
        let data: Value = resp.json().await?;
        Ok(data
            .get("data")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default())
    }

    pub async fn chat(
        &self,
        messages: &[Message],
        model: &str,
        temperature: f64,
        max_tokens: Option<u32>,
    ) -> anyhow::Result<ChatResult> {
        let mut payload = json!({
            "model": model,
            "messages": messages,
            "temperature": temperature,
            "stream": false,
        });
        if let Some(tokens) = max_tokens {
            payload["max_tokens"] = json!(tokens);
        }

        let mut last_error = String::new();
        for attempt in 0..3 {
            match self.post_chat(&payload).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    last_error = err;
                    if attempt < 2 {
                        tokio::time::sleep(Duration::from_millis(1500 * (attempt as u64 + 1))).await;
                    }
                }
            }
        }
        anyhow::bail!("{last_error}")
    }

    async fn post_chat(&self, payload: &Value) -> Result<ChatResult, String> {
        let resp = self
            .client
            .post(format!("{}/chat/completions", self.base))
            .headers(self.headers())
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("模型服务连接失败：{e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(humanize(status.as_u16(), &body));
        }
        let data: Value = resp.json().await.map_err(|e| format!("模型返回格式异常：{e}"))?;
        let content = data["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let usage = &data["usage"];
        Ok(ChatResult {
            text: content.trim().to_string(),
            prompt_tokens: usage["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage["completion_tokens"].as_u64().unwrap_or(0) as u32,
        })
    }
}

fn humanize(status: u16, body: &str) -> String {
    match status {
        401 => "模型 Key 无效或已过期（HTTP 401）".into(),
        402 => "账户余额不足（HTTP 402）".into(),
        403 => "没有访问权限（HTTP 403）".into(),
        404 => "接口地址不对，或模型不存在（HTTP 404）".into(),
        429 => "请求太频繁被限流（HTTP 429），稍后再试".into(),
        _ => format!("模型调用失败（HTTP {status}）：{}", body.chars().take(160).collect::<String>()),
    }
}
