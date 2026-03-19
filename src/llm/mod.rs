//! LLM 客户端模块
//!
//! 提供与大语言模型交互的抽象接口和具体实现

pub mod client;
pub mod openai;
pub mod claude;

pub use client::{ChatMessage, LlmClient};
pub use claude::ClaudeClient;
pub use openai::OpenAiClient;
