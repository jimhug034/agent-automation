//! 飞书 Webhook 推送模块
//!
//! 用于将测试报告推送到飞书群聊

use crate::error::AppError;
use reqwest::Client;
use serde::Serialize;
use tracing::{debug, error, info};

/// 飞书消息类型
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum MsgType {
    Post,
    Interactive,
    Text,
}

/// 飞书消息内容
#[derive(Debug, Clone, Serialize)]
struct FeishuContent {
    post: FeishuPost,
}

/// 飞书 Post 类型消息
#[derive(Debug, Clone, Serialize)]
struct FeishuPost {
    #[serde(rename = "zh_cn")]
    zh_cn: FeishuPostContent,
}

/// 飞书 Post 内容（中文）
#[derive(Debug, Clone, Serialize)]
struct FeishuPostContent {
    title: String,
    content: Vec<Vec<FeishuTextElement>>,
}

/// 飞书文本元素
#[derive(Debug, Clone, Serialize)]
struct FeishuTextElement {
    tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    href: Option<String>,
}

impl FeishuTextElement {
    /// 创建纯文本元素
    fn text(text: impl Into<String>) -> Self {
        Self {
            tag: "text".to_string(),
            text: Some(text.into()),
            href: None,
        }
    }

    /// 创建链接元素
    fn link(text: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            tag: "a".to_string(),
            text: Some(text.into()),
            href: Some(href.into()),
        }
    }

    /// 创建换行元素（纯文本换行）
    fn newline() -> Self {
        Self {
            tag: "text".to_string(),
            text: Some("\n".to_string()),
            href: None,
        }
    }
}

/// 飞书 Webhook 消息
#[derive(Debug, Clone, Serialize)]
struct FeishuMessage {
    msg_type: MsgType,
    content: FeishuContent,
}

impl FeishuMessage {
    /// 从测试报告创建飞书消息
    fn from_test_report(
        task_id: &str,
        summary: &TestSummary,
        duration_secs: u64,
        report_url: Option<&str>,
    ) -> Self {
        let title = format!("🤖 自动化测试报告 - {}", task_id);

        let mut content = Vec::new();

        // 测试摘要标题
        content.push(vec![FeishuTextElement::text("📊 测试摘要")]);
        content.push(vec![FeishuTextElement::newline()]);

        // 统计信息
        content.push(vec![FeishuTextElement::text(format!(
            "总计: {} | ",
            summary.total
        ))]);
        content.push(vec![FeishuTextElement::text(format!(
            "通过: {} | ",
            summary.passed
        ))]);
        content.push(vec![
            FeishuTextElement::text(format!("失败: {}", summary.failed)),
            FeishuTextElement::newline(),
        ]);

        // 通过率
        content.push(vec![
            FeishuTextElement::text(format!("通过率: {:.1}%", summary.pass_rate * 100.0)),
            FeishuTextElement::newline(),
        ]);

        // 耗时
        content.push(vec![
            FeishuTextElement::text(format!("⏱️ 耗时: {}秒", duration_secs)),
            FeishuTextElement::newline(),
        ]);

        // 如果有报告链接，添加链接
        if let Some(url) = report_url {
            content.push(vec![FeishuTextElement::newline()]);
            content.push(vec![
                FeishuTextElement::text("🔗 查看详细报告: "),
                FeishuTextElement::link("点击打开", url),
            ]);
        }

        let post = FeishuPost {
            zh_cn: FeishuPostContent { title, content },
        };

        Self {
            msg_type: MsgType::Post,
            content: FeishuContent { post },
        }
    }
}

/// 测试摘要（简化版，用于飞书推送）
#[derive(Debug, Clone)]
pub struct TestSummary {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub pass_rate: f32,
}

/// 测试报告 trait，定义推送所需的接口
pub trait TestReport {
    fn task_id(&self) -> &str;
    fn total_steps(&self) -> u32;
    fn passed_steps(&self) -> u32;
    fn failed_steps(&self) -> u32;
    fn duration_secs(&self) -> u64;
}

/// 发送飞书通知
///
/// # 参数
/// - `webhook_url`: 飞书机器人 Webhook URL
/// - `task_id`: 任务 ID
/// - `summary`: 测试摘要
/// - `duration_secs`: 测试耗时（秒）
/// - `report_url`: 可选的报告链接
///
/// # 返回
/// - `Ok(())`: 推送成功
/// - `Err(AppError)`: 推送失败
pub async fn send_feishu_notification(
    webhook_url: &str,
    task_id: &str,
    summary: &TestSummary,
    duration_secs: u64,
    report_url: Option<&str>,
) -> Result<(), AppError> {
    info!("准备发送飞书通知，任务: {}", task_id);

    // 构建消息
    let message = FeishuMessage::from_test_report(task_id, summary, duration_secs, report_url);

    debug!("飞书消息内容: {:?}", serde_json::to_string(&message));

    // 创建 HTTP 客户端
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::FeishuPushFailed(format!("创建客户端失败: {}", e)))?;

    // 发送 POST 请求
    let response = client
        .post(webhook_url)
        .json(&message)
        .send()
        .await
        .map_err(|e| AppError::FeishuPushFailed(format!("发送请求失败: {}", e)))?;

    // 检查响应状态
    let status = response.status();
    let response_body = response
        .text()
        .await
        .unwrap_or_else(|_| "无法读取响应体".to_string());

    if !status.is_success() {
        error!("飞书推送失败，状态码: {}, 响应: {}", status, response_body);
        return Err(AppError::FeishuPushFailed(format!(
            "HTTP 错误: {}, 响应: {}",
            status, response_body
        )));
    }

    // 检查飞书 API 返回的 code
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response_body) {
        if let Some(code) = json.get("code").and_then(|c| c.as_i64()) {
            if code != 0 {
                error!("飞书 API 返回错误: {}", json);
                return Err(AppError::FeishuPushFailed(format!("API 错误: {}", json)));
            }
        }
    }

    info!("飞书通知发送成功，任务: {}", task_id);
    Ok(())
}

/// 便捷函数：从完整的测试报告数据创建并发送通知
///
/// # 参数
/// - `webhook_url`: 飞书机器人 Webhook URL
/// - `task_id`: 任务 ID
/// - `total_steps`: 总步骤数
/// - `passed_steps`: 通过步骤数
/// - `failed_steps`: 失败步骤数
/// - `duration_secs`: 测试耗时（秒）
/// - `report_url`: 可选的报告链接
pub async fn send_feishu_notification_simple(
    webhook_url: &str,
    task_id: &str,
    total_steps: u32,
    passed_steps: u32,
    failed_steps: u32,
    duration_secs: u64,
    report_url: Option<&str>,
) -> Result<(), AppError> {
    let pass_rate = if total_steps > 0 {
        passed_steps as f32 / total_steps as f32
    } else {
        0.0
    };

    let summary = TestSummary {
        total: total_steps,
        passed: passed_steps,
        failed: failed_steps,
        pass_rate,
    };

    send_feishu_notification(webhook_url, task_id, &summary, duration_secs, report_url).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feishu_message_creation() {
        let summary = TestSummary {
            total: 10,
            passed: 8,
            failed: 1,
            pass_rate: 0.8,
        };

        let message = FeishuMessage::from_test_report(
            "test-task-123",
            &summary,
            120,
            Some("https://example.com/report"),
        );

        let json = serde_json::to_string_pretty(&message).unwrap();
        println!("{}", json);

        // 验证消息结构
        assert_eq!(message.msg_type, MsgType::Post);
        assert!(message.content.post.zh_cn.title.contains("test-task-123"));
    }

    #[test]
    fn test_text_element() {
        let elem = FeishuTextElement::text("hello");
        assert_eq!(elem.tag, "text");
        assert_eq!(elem.text, Some("hello".to_string()));
    }

    #[test]
    fn test_link_element() {
        let elem = FeishuTextElement::link("点击", "https://example.com");
        assert_eq!(elem.tag, "a");
        assert_eq!(elem.text, Some("点击".to_string()));
        assert_eq!(elem.href, Some("https://example.com".to_string()));
    }
}
