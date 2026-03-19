//! Claude API 客户端
//!
//! 与 Anthropic Claude API 交互的客户端实现

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info};

use crate::error::AppError;
use crate::llm::client::{ChatMessage, LlmClient};

/// Claude API 客户端
pub struct ClaudeClient {
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

impl ClaudeClient {
    /// 创建新的 Claude 客户端
    ///
    /// # Arguments
    ///
    /// * `api_key` - Anthropic API 密钥
    /// * `model` - 使用的模型名称（如 claude-3-opus, claude-3-sonnet）
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

    /// 构建消息 API URL
    fn messages_url(&self) -> String {
        let base = self.api_base.trim_end_matches('/');
        format!("{}/v1/messages", base)
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
                    "Claude API 请求重试 {}/{}, 错误: {:?}",
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

    /// 将通用 ChatMessage 转换为 Claude 格式
    ///
    /// Claude API 使用不同的消息格式（system 参数 + messages 数组）
    fn convert_messages(&self, messages: &[ChatMessage]) -> (Option<String>, Vec<ClaudeMessage>) {
        let mut system_message: Option<String> = None;
        let mut claude_messages = Vec::new();

        for msg in messages {
            match msg.role.as_str() {
                "system" => {
                    system_message = Some(msg.content.clone());
                }
                "user" => {
                    claude_messages.push(ClaudeMessage {
                        role: "user".to_string(),
                        content: msg.content.clone(),
                    });
                }
                "assistant" => {
                    claude_messages.push(ClaudeMessage {
                        role: "assistant".to_string(),
                        content: msg.content.clone(),
                    });
                }
                _ => {
                    debug!("忽略未知角色的消息: {}", msg.role);
                }
            }
        }

        (system_message, claude_messages)
    }

    /// 执行单次聊天 API 请求
    async fn do_chat_request(&self, messages: &[ChatMessage]) -> Result<String, AppError> {
        let (system, claude_messages) = self.convert_messages(messages);

        if claude_messages.is_empty() {
            return Err(AppError::LlmApiError("消息列表为空".to_string()));
        }

        // 确保最后一条消息是用户消息
        let last_is_user = claude_messages
            .last()
            .map(|m| m.role == "user")
            .unwrap_or(false);

        if !last_is_user {
            return Err(AppError::LlmApiError(
                "最后一条消息必须是用户消息".to_string(),
            ));
        }

        let request = MessageRequest {
            model: self.model.clone(),
            messages: claude_messages,
            system,
            max_tokens: 2000,
            temperature: 0.7,
        };

        info!("发送 Claude API 请求, 模型: {}", self.model);
        debug!("请求数据: {:?}", serde_json::to_string(&request).unwrap());

        let response = self
            .client
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("Claude API 请求失败: {}", e);
                AppError::LlmApiError(format!("请求失败: {}", e))
            })?;

        let status = response.status();
        let body: String = response.text().await.map_err(|e| {
            error!("读取响应体失败: {}", e);
            AppError::LlmApiError(format!("读取响应失败: {}", e))
        })?;

        if !status.is_success() {
            error!("Claude API 返回错误: status={}, body={}", status, body);
            return Err(AppError::LlmApiError(format!(
                "API 返回错误: status={}, body={}",
                status, body
            )));
        }

        let message_response: MessageResponse = serde_json::from_str(&body).map_err(|e| {
            error!("解析 Claude API 响应失败: {}, body: {}", e, body);
            AppError::LlmApiError(format!("解析响应失败: {}", e))
        })?;

        let content = message_response
            .content
            .first()
            .and_then(|c| match c {
                ContentBlock::Text { text } => Some(text),
                _ => None,
            })
            .ok_or_else(|| {
                error!("Claude API 响应中没有文本内容");
                AppError::LlmApiError("响应中没有文本内容".to_string())
            })?;

        debug!("收到 Claude API 响应: {}", content);
        Ok(content.clone())
    }
}

#[async_trait]
impl LlmClient for ClaudeClient {
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, Box<dyn std::error::Error>> {
        self.chat_with_retry(messages).await
    }
}

/// Claude 消息格式
#[derive(Debug, Clone, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

/// Claude API 内容块
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: serde_json::Value },
}

/// Claude 消息 API 请求
#[derive(Debug, Serialize)]
struct MessageRequest {
    model: String,
    messages: Vec<ClaudeMessage>,
    system: Option<String>,
    max_tokens: u32,
    temperature: f32,
}

/// Claude 消息 API 响应
#[derive(Debug, Deserialize)]
struct MessageResponse {
    content: Vec<ContentBlock>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_client_new() {
        let client = ClaudeClient::new(
            "test-key",
            "claude-3-opus",
            "https://api.anthropic.com",
            30,
            3,
        );

        assert_eq!(client.api_key, "test-key");
        assert_eq!(client.model, "claude-3-opus");
        assert_eq!(client.api_base, "https://api.anthropic.com");
        assert_eq!(client.timeout, Duration::from_secs(30));
        assert_eq!(client.max_retries, 3);
    }

    #[test]
    fn test_claude_client_messages_url() {
        let client = ClaudeClient::new(
            "test-key",
            "claude-3-opus",
            "https://api.anthropic.com",
            30,
            3,
        );
        assert_eq!(
            client.messages_url(),
            "https://api.anthropic.com/v1/messages"
        );

        let client = ClaudeClient::new(
            "test-key",
            "claude-3-opus",
            "https://api.anthropic.com/",
            30,
            3,
        );
        assert_eq!(
            client.messages_url(),
            "https://api.anthropic.com/v1/messages"
        );
    }

    #[test]
    fn test_convert_messages() {
        let client = ClaudeClient::new(
            "test-key",
            "claude-3-opus",
            "https://api.anthropic.com",
            30,
            3,
        );

        let messages = vec![
            ChatMessage::system("You are helpful"),
            ChatMessage::user("Hello"),
            ChatMessage::assistant("Hi"),
            ChatMessage::user("How are you?"),
        ];

        let (system, claude_messages) = client.convert_messages(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(claude_messages.len(), 3);
        assert_eq!(claude_messages[0].role, "user");
        assert_eq!(claude_messages[0].content, "Hello");
        assert_eq!(claude_messages[1].role, "assistant");
        assert_eq!(claude_messages[1].content, "Hi");
        assert_eq!(claude_messages[2].role, "user");
        assert_eq!(claude_messages[2].content, "How are you?");
    }

    #[test]
    fn test_convert_messages_empty() {
        let client = ClaudeClient::new(
            "test-key",
            "claude-3-opus",
            "https://api.anthropic.com",
            30,
            3,
        );

        let (system, claude_messages) = client.convert_messages(&[]);
        assert!(system.is_none());
        assert!(claude_messages.is_empty());
    }

    #[test]
    fn test_convert_messages_system_only() {
        let client = ClaudeClient::new(
            "test-key",
            "claude-3-opus",
            "https://api.anthropic.com",
            30,
            3,
        );

        let messages = vec![ChatMessage::system("You are helpful")];
        let (system, claude_messages) = client.convert_messages(&messages);

        assert_eq!(system, Some("You are helpful".to_string()));
        assert!(claude_messages.is_empty());
    }
}
