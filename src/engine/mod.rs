//! 测试引擎模块
//!
//! 提供浏览器控制和测试代理的核心功能

pub mod agent;
pub mod browser;

pub use agent::{TestActionPlan, TestAgent};
pub use browser::{BrowserSnapshot, AgentBrowser};
