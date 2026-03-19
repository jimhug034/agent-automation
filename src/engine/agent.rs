//! 测试 Agent 模块
//!
//! 负责解析自然语言测试目标并执行测试步骤

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, info};

use crate::engine::browser::{AgentBrowser, BrowserSnapshot};
use crate::error::AppError;
use crate::llm::client::{ChatMessage, LlmClient};
use crate::models::{TestAction, TestStep, TestTask};

/// 测试 Agent
///
/// 结合 LLM 和浏览器控制，实现智能测试执行
pub struct TestAgent {
    /// 浏览器控制器
    browser: AgentBrowser,
    /// LLM 客户端
    llm_client: Box<dyn LlmClient>,
    /// 当前测试任务
    current_task: Option<TestTask>,
}

impl TestAgent {
    /// 创建新的测试 Agent
    ///
    /// # Arguments
    ///
    /// * `browser` - 浏览器控制器
    /// * `llm_client` - LLM 客户端
    pub fn new(browser: AgentBrowser, llm_client: Box<dyn LlmClient>) -> Self {
        Self {
            browser,
            llm_client,
            current_task: None,
        }
    }

    /// 设置当前测试任务
    ///
    /// # Arguments
    ///
    /// * `task` - 测试任务
    pub fn set_task(&mut self, task: TestTask) {
        self.current_task = Some(task);
    }

    /// 解析自然语言测试目标为动作序列
    ///
    /// 使用 LLM 分析测试目标并生成可执行的测试步骤
    ///
    /// # Arguments
    ///
    /// * `goal` - 自然语言描述的测试目标
    /// * `snapshot` - 当前页面快照（可选）
    ///
    /// # Returns
    ///
    /// 返回解析后的测试动作计划
    pub async fn parse_goal(
        &self,
        goal: &str,
        snapshot: Option<&BrowserSnapshot>,
    ) -> Result<TestActionPlan, AppError> {
        info!("解析测试目标: {}", goal);

        let system_prompt = self.build_system_prompt();
        let user_prompt = self.build_user_prompt(goal, snapshot);

        let messages = vec![
            ChatMessage::system(system_prompt),
            ChatMessage::user(user_prompt),
        ];

        debug!("发送 LLM 请求以解析测试目标");
        let response = self.llm_client.chat(messages).await.map_err(|e| {
            error!("LLM 请求失败: {}", e);
            AppError::LlmApiError(format!("解析测试目标失败: {}", e))
        })?;

        debug!("收到 LLM 响应: {}", response);

        // 解析 LLM 返回的 JSON
        let plan: TestActionPlan = match serde_json::from_str(&response) {
            Ok(p) => p,
            Err(e) => {
                // 尝试从响应中提取 JSON
                let extracted = self.extract_json_from_response(&response);
                match extracted {
                    Some(json_str) => serde_json::from_str(&json_str).map_err(|e2| {
                        AppError::LlmApiError(format!("解析测试计划失败: {} (原始: {})", e2, e))
                    })?,
                    None => {
                        return Err(AppError::LlmApiError(format!(
                            "解析测试计划失败: {}",
                            e
                        )))
                    }
                }
            }
        };

        info!("成功解析测试计划, 包含 {} 个动作", plan.actions.len());
        Ok(plan)
    }

    /// 执行单个测试动作
    ///
    /// # Arguments
    ///
    /// * `action` - 要执行的测试动作
    ///
    /// # Returns
    ///
    /// 返回执行结果和可能的错误信息
    pub async fn execute_step(
        &mut self,
        action: &TestAction,
    ) -> Result<ExecutionResult, AppError> {
        debug!("执行动作: {:?}", action);

        let result = match action {
            TestAction::Click { ref_id } => {
                self.browser.click(ref_id)?;
                ExecutionResult {
                    success: true,
                    message: format!("点击元素: {}", ref_id),
                    screenshot: None,
                }
            }
            TestAction::Input { ref_id, text } => {
                self.browser.input(ref_id, text)?;
                ExecutionResult {
                    success: true,
                    message: format!("输入文本到 {}: {}", ref_id, text),
                    screenshot: None,
                }
            }
            TestAction::Wait { duration_ms } => {
                self.browser.wait(*duration_ms)?;
                ExecutionResult {
                    success: true,
                    message: format!("等待 {}ms", duration_ms),
                    screenshot: None,
                }
            }
            TestAction::Navigate { url } => {
                self.browser.navigate(url)?;
                // 等待页面加载
                self.browser.wait(1000)?;
                ExecutionResult {
                    success: true,
                    message: format!("导航到: {}", url),
                    screenshot: None,
                }
            }
            TestAction::Assert { condition } => {
                // 断言需要 LLM 分析页面状态
                let snapshot = self.browser.snapshot()?;
                let assertion_result = self.evaluate_assertion(condition, &snapshot).await?;
                ExecutionResult {
                    success: assertion_result,
                    message: if assertion_result {
                        format!("断言通过: {}", condition)
                    } else {
                        format!("断言失败: {}", condition)
                    },
                    screenshot: None,
                }
            }
            TestAction::Skip { reason } => {
                info!("跳过动作: {}", reason);
                ExecutionResult {
                    success: true,
                    message: format!("跳过: {}", reason),
                    screenshot: None,
                }
            }
        };

        Ok(result)
    }

    /// 执行完整的测试目标
    ///
    /// # Arguments
    ///
    /// * `goal` - 自然语言描述的测试目标
    ///
    /// # Returns
    ///
    /// 返回执行的测试步骤列表
    pub async fn execute_goal(&mut self, goal: &str) -> Result<Vec<TestStep>, AppError> {
        info!("开始执行测试目标: {}", goal);

        // 获取初始快照
        let initial_snapshot = self.browser.snapshot()?;

        // 解析测试目标
        let plan = self.parse_goal(goal, Some(&initial_snapshot)).await?;

        let mut steps = Vec::new();
        let mut step_id = 1;

        for action_plan in &plan.actions {
            let step = TestStep::new(
                format!("step-{}", step_id),
                action_plan.description.clone(),
                action_plan.action.clone(),
            );

            info!("执行步骤 {}: {}", step_id, action_plan.description);

            // 执行动作
            let execution_result: Result<ExecutionResult, AppError> =
                self.execute_step(&action_plan.action).await;

            let mut completed_step = step;

            match execution_result {
                Ok(result) => {
                    if result.success {
                        completed_step.status = crate::models::StepStatus::Passed;
                        info!("步骤 {} 执行成功", step_id);
                    } else {
                        completed_step.status = crate::models::StepStatus::Failed;
                        completed_step.error = Some(result.message);
                        info!("步骤 {} 执行失败", step_id);
                    }
                }
                Err(e) => {
                    completed_step.status = crate::models::StepStatus::Failed;
                    completed_step.error = Some(e.to_string());
                    error!("步骤 {} 执行出错: {}", step_id, e);

                    // 捕获错误截图
                    if let Ok(screenshot) = self.browser.screenshot(None) {
                        completed_step.screenshot = Some(screenshot);
                    }

                    steps.push(completed_step);

                    // 遇到错误停止执行
                    break;
                }
            }

            steps.push(completed_step);
            step_id += 1;

            // 步骤间短暂等待
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        info!(
            "测试目标执行完成, 共执行 {} 个步骤",
            steps.len()
        );
        Ok(steps)
    }

    /// 评估断言条件
    ///
    /// 使用 LLM 分析页面快照，评估断言是否成立
    async fn evaluate_assertion(
        &self,
        condition: &str,
        snapshot: &BrowserSnapshot,
    ) -> Result<bool, AppError> {
        debug!("评估断言: {}", condition);

        let prompt = format!(
            "请分析以下页面信息，判断断言是否成立：\n\n\
             断言: {}\n\n\
             页面信息:\n\
             URL: {}\n\
             标题: {}\n\
             元素: {}\n\n\
             请只返回 true 或 false。",
            condition,
            snapshot.url,
            snapshot.title,
            serde_json::to_string(&snapshot.elements).unwrap_or_default()
        );

        let messages = vec![
            ChatMessage::system(
                "你是一个测试验证助手。请根据页面信息判断断言是否成立，只返回 true 或 false。"
                    .to_string(),
            ),
            ChatMessage::user(prompt),
        ];

        let response = self.llm_client.chat(messages).await.map_err(|e| {
            error!("评估断言时 LLM 请求失败: {}", e);
            AppError::LlmApiError(format!("评估断言失败: {}", e))
        })?;

        let result = response
            .to_lowercase()
            .contains("true")
            || response.contains("成立")
            || response.contains("通过");

        debug!("断言评估结果: {}", result);
        Ok(result)
    }

    /// 构建系统提示词
    fn build_system_prompt(&self) -> String {
        r#"你是一个自动化测试规划助手。你的任务是将自然语言描述的测试目标转换为可执行的测试步骤。

响应必须是有效的 JSON 格式，包含以下结构：
{
  "description": "测试计划的简要描述",
  "estimated_steps": 3,
  "actions": [
    {
      "description": "第一步的描述",
      "action": {
        "type": "navigate",
        "url": "https://example.com"
      }
    },
    {
      "description": "点击登录按钮",
      "action": {
        "type": "click",
        "ref_id": "login-btn"
      }
    },
    {
      "description": "输入用户名",
      "action": {
        "type": "input",
        "ref_id": "username-input",
        "text": "testuser"
      }
    }
  ]
}

支持的动作类型：
- navigate: 导航到指定 URL
- click: 点击元素（需要 ref_id）
- input: 输入文本（需要 ref_id 和 text）
- wait: 等待指定时间（需要 duration_ms）
- assert: 断言（需要 condition）
- skip: 跳过（需要 reason）

注意：
1. 确保返回的是纯 JSON，不要包含任何其他文本
2. ref_id 应该从页面快照的 elements 中选择
3. 如果目标涉及硬件功能（如摄像头、麦克风），应该使用 skip 动作
4. 保持步骤简洁明了"#
            .to_string()
    }

    /// 构建用户提示词
    fn build_user_prompt(&self, goal: &str, snapshot: Option<&BrowserSnapshot>) -> String {
        let mut prompt = format!("测试目标: {}\n", goal);

        if let Some(snapt) = snapshot {
            prompt.push_str(&format!(
                "\n当前页面状态:\nURL: {}\n标题: {}\n",
                snapt.url, snapt.title
            ));

            if !snapt.elements.is_empty() {
                prompt.push_str("\n可用的页面元素:\n");
                for elem in &snapt.elements {
                    prompt.push_str(&format!(
                        "- [{}] {}: {} (type={}, tag={})\n",
                        elem.ref_id, elem.tag_name, elem.text, elem.element_type, elem.tag_name
                    ));
                }
            }
        }

        prompt
    }

    /// 从响应中提取 JSON
    fn extract_json_from_response(&self, response: &str) -> Option<String> {
        // 尝试找到 JSON 代码块
        if let Some(start) = response.find("```json") {
            let start = start + 7;
            if let Some(end) = response[start..].find("```") {
                return Some(response[start..start + end].trim().to_string());
            }
        }

        // 尝试查找 { ... }
        if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                if end > start {
                    return Some(response[start..=end].to_string());
                }
            }
        }

        None
    }

    /// 获取当前浏览器状态
    pub fn browser_state(&self) -> &AgentBrowser {
        &self.browser
    }

    /// 获取当前任务
    pub fn current_task(&self) -> Option<&TestTask> {
        self.current_task.as_ref()
    }
}

/// 测试动作计划
///
/// 由 LLM 生成的测试步骤序列
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestActionPlan {
    /// 计划描述
    pub description: String,
    /// 预估步骤数
    pub estimated_steps: usize,
    /// 动作列表
    pub actions: Vec<ActionPlanItem>,
}

/// 动作计划项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlanItem {
    /// 步骤描述
    pub description: String,
    /// 具体动作
    pub action: TestAction,
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// 是否成功
    pub success: bool,
    /// 结果消息
    pub message: String,
    /// 截图数据（base64）
    pub screenshot: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_test_action_plan_serialization() {
        let plan = TestActionPlan {
            description: "测试登录流程".to_string(),
            estimated_steps: 2,
            actions: vec![
                ActionPlanItem {
                    description: "打开登录页面".to_string(),
                    action: TestAction::Navigate {
                        url: "https://example.com/login".to_string(),
                    },
                },
                ActionPlanItem {
                    description: "点击登录按钮".to_string(),
                    action: TestAction::Click {
                        ref_id: "login-btn".to_string(),
                    },
                },
            ],
        };

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("\"description\":\"测试登录流程\""));
        assert!(json.contains("\"estimated_steps\":2"));
        assert!(json.contains("\"type\":\"navigate\""));
        assert!(json.contains("\"type\":\"click\""));
    }

    #[test]
    fn test_extract_json_from_response() {
        let agent = create_mock_agent();

        // 测试代码块格式
        let response = r#"这是一些文本
```json
{"key": "value"}
```
更多文本"#;
        let extracted = agent.extract_json_from_response(response);
        assert_eq!(extracted, Some(r#"{"key": "value"}"#.to_string()));

        // 测试直接 JSON
        let response = r#"前置文本 {"key": "value"} 后置文本"#;
        let extracted = agent.extract_json_from_response(response);
        assert_eq!(extracted, Some(r#"{"key": "value"}"#.to_string()));

        // 测试无效响应
        let response = "没有 JSON 的文本";
        let extracted = agent.extract_json_from_response(response);
        assert!(extracted.is_none());
    }

    #[test]
    fn test_build_user_prompt() {
        let agent = create_mock_agent();

        let snapshot = BrowserSnapshot {
            url: "https://example.com".to_string(),
            title: "Example".to_string(),
            elements: vec![],
            text_content: "Hello".to_string(),
        };

        let prompt = agent.build_user_prompt("测试目标", Some(&snapshot));
        assert!(prompt.contains("测试目标"));
        assert!(prompt.contains("https://example.com"));
        assert!(prompt.contains("Example"));
    }

    fn create_mock_agent() -> TestAgent {
        use crate::llm::client::ChatMessage;
        use async_trait::async_trait;

        struct MockLlmClient;

        #[async_trait]
        impl LlmClient for MockLlmClient {
            async fn chat(&self, _messages: Vec<ChatMessage>) -> Result<String, Box<dyn std::error::Error>> {
                Ok("{}".to_string())
            }
        }

        let browser = AgentBrowser::new("/fake/path", 9222);
        let llm_client = Box::new(MockLlmClient);
        TestAgent::new(browser, llm_client)
    }

    #[test]
    fn test_execution_result() {
        let result = ExecutionResult {
            success: true,
            message: "操作成功".to_string(),
            screenshot: None,
        };

        assert!(result.success);
        assert_eq!(result.message, "操作成功");
        assert!(result.screenshot.is_none());
    }
}
