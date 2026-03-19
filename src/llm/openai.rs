//! OpenAI API 客户端
//!
//! 与 OpenAI API 交互的客户端实现

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info};

use crate::error::AppError;
use crate::llm::client::{ChatMessage, LlmClient};

/// OpenAI API 客户端
pub struct OpenAiClient {
    /// API 密钥
    api_key: String,
    /// 使用的模型名称
    model: String,
    /// API 基础 URL
    api_base: String,
    /// HTTP 客户端
    client: Client,
    /// 请求超时时间
    timeout: Duration,
    /// 最大重试次数
    max_retries: u32,
}

impl OpenAiClient {
    /// 创建新的 OpenAI 客户端
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API 密钥
    /// * `model` - 使用的模型名称（如 gpt-4, gpt-3.5-turbo）
    /// * `api_base` - API 基础 URL
    /// * `timeout_secs` - 请求超时时间（秒）
    /// * `max_retries` - 最大重试次数
    pub fn new<S: Into<String>>(
        api_key: S,
        model: S,
        api_base: S,
        timeout_secs: u64,
        max_retries: u32,
    ) -> Self {
        let timeout = Duration::from_secs(timeout_secs);

        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            api_key: api_key.into(),
            model: model.into(),
            api_base: api_base.into(),
            client,
            timeout,
            max_retries,
        }
    }

    /// 构建聊天 API URL
    fn chat_url(&self) -> String {
        let base = self.api_base.trim_end_matches('/');
        format!("{}/v1/chat/completions", base)
    }

    /// 执行带重试的 API 请求
    async fn chat_with_retry(
        &self,
        messages: Vec<ChatMessage>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut last_error: Option<AppError> = None;

        for attempt in 0..=self.max_retries {
            if attempt > 0 {
                debug!(
                    "OpenAI API 请求重试 {}/{}, 错误: {:?}",
                    attempt, self.max_retries, last_error
                );
                tokio::time::sleep(Duration::from_millis(1000 * attempt as u64)).await;
            }

            match self.do_chat_request(&messages).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = Some(e);
                    // 对于认证错误或无效请求，不需要重试
                    if let Some(AppError::LlmApiError(msg)) = &last_error {
                        if msg.contains("401") || msg.contains("403") || msg.contains("400") {
                            break;
                        }
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| AppError::LlmApiError("Unknown error".to_string()))
            .into())
    }

    /// 执行单次聊天 API 请求
    async fn do_chat_request(
        &self,
        messages: &[ChatMessage],
    ) -> Result<String, AppError> {
        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            temperature: 0.7,
            max_tokens: 2000,
        };

        info!("发送 OpenAI API 请求, 模型: {}", self.model);
        debug!("请求数据: {:?}", serde_json::to_string(&request).unwrap());

        let response = self
            .client
            .post(&self.chat_url())
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("OpenAI API 请求失败: {}", e);
                AppError::LlmApiError(format!("请求失败: {}", e))
            })?;

        let status = response.status();
        let body: String = response.text().await.map_err(|e| {
            error!("读取响应体失败: {}", e);
            AppError::LlmApiError(format!("读取响应失败: {}", e))
        })?;

        if !status.is_success() {
            error!(
                "OpenAI API 返回错误: status={}, body={}",
                status, body
            );
            return Err(AppError::LlmApiError(format!(
                "API 返回错误: status={}, body={}",
                status, body
            )));
        }

        let chat_response: ChatCompletionResponse =
            serde_json::from_str(&body).map_err(|e| {
                error!("解析 OpenAI API 响应失败: {}, body: {}", e, body);
                AppError::LlmApiError(format!("解析响应失败: {}", e))
            })?;

        let content = chat_response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| {
                error!("OpenAI API 响应中没有消息内容");
                AppError::LlmApiError("响应中没有消息内容".to_string())
            })?;

        debug!("收到 OpenAI API 响应: {}", content);
        Ok(content)
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, Box<dyn std::error::Error>> {
        self.chat_with_retry(messages).await
    }
}

/// OpenAI 聊天完成请求
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

/// OpenAI 聊天完成响应
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

/// OpenAI 响应中的选择项
#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

/// OpenAI 响应中的消息
#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_client_new() {
        let client = OpenAiClient::new("test-key", "gpt-4", "https://api.openai.com", 30, 3);

        assert_eq!(client.api_key, "test-key");
        assert_eq!(client.model, "gpt-4");
        assert_eq!(client.api_base, "https://api.openai.com");
        assert_eq!(client.timeout, Duration::from_secs(30));
        assert_eq!(client.max_retries, 3);
    }

    #[test]
    fn test_openai_client_chat_url() {
        let client = OpenAiClient::new("test-key", "gpt-4", "https://api.openai.com", 30, 3);
        assert_eq!(
            client.chat_url(),
            "https://api.openai.com/v1/chat/completions"
        );

        let client = OpenAiClient::new("test-key", "gpt-4", "https://api.openai.com/", 30, 3);
        assert_eq!(
            client.chat_url(),
            "https://api.openai.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_chat_completion_request_serialization() {
        let request = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage::user("Hello")],
            temperature: 0.7,
            max_tokens: 2000,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(json.contains("\"max_tokens\":2000"));
    }
}
