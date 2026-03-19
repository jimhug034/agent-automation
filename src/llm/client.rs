//! LLM 客户端 trait 定义
//!
//! 定义了与大语言模型交互的统一接口

use async_trait::async_trait;

/// LLM 客户端 trait
///
/// 所有 LLM 实现都需要实现此 trait，提供统一的聊天接口
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// 发送聊天请求并获取响应
    ///
    /// # Arguments
    ///
    /// * `messages` - 聊天消息列表
    ///
    /// # Returns
    ///
    /// 返回 LLM 的响应内容
    ///
    /// # Errors
    ///
    /// 当 API 调用失败时返回错误
    async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, Box<dyn std::error::Error>>;
}

/// 聊天消息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    /// 消息角色 (system, user, assistant)
    pub role: String,
    /// 消息内容
    pub content: String,
}

impl ChatMessage {
    /// 创建用户消息
    ///
    /// # Arguments
    ///
    /// * `content` - 消息内容
    pub fn user<S: Into<String>>(content: S) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// 创建系统消息
    ///
    /// # Arguments
    ///
    /// * `content` - 消息内容
    pub fn system<S: Into<String>>(content: S) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    /// 创建助手消息
    ///
    /// # Arguments
    ///
    /// * `content` - 消息内容
    pub fn assistant<S: Into<String>>(content: S) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_message_user() {
        let msg = ChatMessage::user("Hello");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content, "Hello");
    }

    #[test]
    fn test_chat_message_system() {
        let msg = ChatMessage::system("You are a helpful assistant");
        assert_eq!(msg.role, "system");
        assert_eq!(msg.content, "You are a helpful assistant");
    }

    #[test]
    fn test_chat_message_assistant() {
        let msg = ChatMessage::assistant("Hi there");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Hi there");
    }

    #[test]
    fn test_chat_message_serialization() {
        let msg = ChatMessage::user("test");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"test\""));
    }
}
