//! 数据模型定义
//!
//! 包含测试请求、任务、步骤和报告相关的数据结构

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// 测试请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRequest {
    /// Electron 应用的包下载 URL
    pub package_url: String,
    /// 测试目标列表
    pub test_goals: Vec<String>,
    /// 可选的测试配置
    pub options: Option<TestOptions>,
}

/// 测试选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOptions {
    /// 使用的 LLM 模型名称
    pub model: Option<String>,
    /// 超时时间（秒）
    pub timeout: Option<u64>,
    /// 重试次数
    pub retries: Option<u32>,
}

/// 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// 等待开始
    Pending,
    /// 下载包中
    Downloading,
    /// 解压中
    Extracting,
    /// 安装依赖中
    Installing,
    /// 运行测试中
    Running,
    /// 已完成
    Completed,
    /// 失败（包含错误信息）
    Failed(String),
}

/// 测试任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTask {
    /// 任务唯一标识
    pub id: String,
    /// Electron 应用的包下载 URL
    pub package_url: String,
    /// 测试目标列表
    pub test_goals: Vec<String>,
    /// 当前任务状态
    pub status: TaskStatus,
    /// 工作目录路径
    pub workspace: PathBuf,
    /// Electron 可执行文件路径
    pub electron_path: PathBuf,
    /// Chrome DevTools Protocol 端口
    pub cdp_port: u16,
}

impl TestTask {
    /// 创建新的测试任务
    ///
    /// # Arguments
    ///
    /// * `package_url` - Electron 应用的包下载 URL
    /// * `test_goals` - 测试目标列表
    /// * `workspace` - 工作目录路径
    /// * `electron_path` - Electron 可执行文件路径
    /// * `cdp_port` - Chrome DevTools Protocol 端口
    ///
    /// # Returns
    ///
    /// 返回一个新的 TestTask 实例，状态为 Pending
    pub fn new(
        package_url: String,
        test_goals: Vec<String>,
        workspace: PathBuf,
        electron_path: PathBuf,
        cdp_port: u16,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            package_url,
            test_goals,
            status: TaskStatus::Pending,
            workspace,
            electron_path,
            cdp_port,
        }
    }
}

/// 步骤状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StepStatus {
    /// 等待执行
    Pending,
    /// 执行中
    Running,
    /// 通过
    Passed,
    /// 失败
    Failed,
    /// 跳过
    Skipped,
}

/// 测试动作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TestAction {
    /// 点击元素
    Click { ref_id: String },
    /// 输入文本
    Input { ref_id: String, text: String },
    /// 等待
    Wait { duration_ms: u32 },
    /// 导航到 URL
    Navigate { url: String },
    /// 断言
    Assert { condition: String },
    /// 跳过
    Skip { reason: String },
}

/// 测试步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    /// 步骤唯一标识
    pub id: String,
    /// 步骤描述
    pub description: String,
    /// 测试动作
    pub action: TestAction,
    /// 步骤状态
    pub status: StepStatus,
    /// 截图文件路径（base64 或文件路径）
    pub screenshot: Option<String>,
    /// 错误信息
    pub error: Option<String>,
    /// 是否为硬件相关步骤
    pub is_hardware_related: bool,
}

impl TestStep {
    /// 创建新的测试步骤
    pub fn new(id: String, description: String, action: TestAction) -> Self {
        Self {
            id,
            description,
            action,
            status: StepStatus::Pending,
            screenshot: None,
            error: None,
            is_hardware_related: false,
        }
    }

    /// 标记为硬件相关步骤
    pub fn with_hardware_related(mut self) -> Self {
        self.is_hardware_related = true;
        self
    }
}

/// 报告摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSummary {
    /// 总步骤数
    pub total: u32,
    /// 通过数
    pub passed: u32,
    /// 失败数
    pub failed: u32,
    /// 跳过数
    pub skipped: u32,
    /// 通过率 (0.0 - 1.0)
    pub pass_rate: f32,
}

impl ReportSummary {
    /// 从测试步骤列表生成报告摘要
    ///
    /// # Arguments
    ///
    /// * `steps` - 测试步骤列表
    ///
    /// # Returns
    ///
    /// 返回包含统计信息的 ReportSummary
    pub fn from_steps(steps: &[TestStep]) -> Self {
        let total = steps.len() as u32;
        let passed = steps
            .iter()
            .filter(|s| s.status == StepStatus::Passed)
            .count() as u32;
        let failed = steps
            .iter()
            .filter(|s| s.status == StepStatus::Failed)
            .count() as u32;
        let skipped = steps
            .iter()
            .filter(|s| s.status == StepStatus::Skipped)
            .count() as u32;

        let pass_rate = if total > 0 {
            passed as f32 / total as f32
        } else {
            0.0
        };

        Self {
            total,
            passed,
            failed,
            skipped,
            pass_rate,
        }
    }
}

/// 测试报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReport {
    /// 关联的任务 ID
    pub task_id: String,
    /// Electron 应用的包 URL
    pub package_url: String,
    /// 测试开始时间
    pub start_time: DateTime<Utc>,
    /// 测试结束时间
    pub end_time: DateTime<Utc>,
    /// 测试持续时间（秒）
    pub duration_secs: u64,
    /// 测试步骤列表
    pub steps: Vec<TestStep>,
    /// 报告摘要
    pub summary: ReportSummary,
}

impl TestReport {
    /// 创建新的测试报告
    pub fn new(
        task_id: String,
        package_url: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        steps: Vec<TestStep>,
    ) -> Self {
        let duration_secs = if end_time > start_time {
            (end_time - start_time).num_seconds().max(0) as u64
        } else {
            0
        };

        let summary = ReportSummary::from_steps(&steps);

        Self {
            task_id,
            package_url,
            start_time,
            end_time,
            duration_secs,
            steps,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_equality() {
        assert_eq!(TaskStatus::Pending, TaskStatus::Pending);
        assert_ne!(TaskStatus::Pending, TaskStatus::Downloading);
    }

    #[test]
    fn test_step_status_equality() {
        assert_eq!(StepStatus::Pending, StepStatus::Pending);
        assert_ne!(StepStatus::Passed, StepStatus::Failed);
    }

    #[test]
    fn test_report_summary_from_empty_steps() {
        let summary = ReportSummary::from_steps(&[]);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.pass_rate, 0.0);
    }

    #[test]
    fn test_report_summary_from_steps() {
        let steps = vec![
            TestStep::new(
                "1".to_string(),
                "Step 1".to_string(),
                TestAction::Wait { duration_ms: 100 },
            ),
            TestStep::new(
                "2".to_string(),
                "Step 2".to_string(),
                TestAction::Click {
                    ref_id: "btn".to_string(),
                },
            ),
        ];

        let summary = ReportSummary::from_steps(&steps);
        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 0); // All steps are Pending by default
        assert_eq!(summary.failed, 0);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.pass_rate, 0.0);
    }

    #[test]
    fn test_test_task_new() {
        let task = TestTask::new(
            "https://example.com/app.zip".to_string(),
            vec!["Test login".to_string(), "Test logout".to_string()],
            PathBuf::from("/tmp/workspace"),
            PathBuf::from("/tmp/app/electron"),
            9222,
        );

        assert!(!task.id.is_empty());
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.package_url, "https://example.com/app.zip");
        assert_eq!(task.test_goals.len(), 2);
        assert_eq!(task.cdp_port, 9222);
    }

    #[test]
    fn test_test_step_with_hardware_related() {
        let step = TestStep::new(
            "1".to_string(),
            "Click button".to_string(),
            TestAction::Click {
                ref_id: "btn".to_string(),
            },
        )
        .with_hardware_related();

        assert!(step.is_hardware_related);
    }

    #[test]
    fn test_test_action_serialization() {
        let action = TestAction::Input {
            ref_id: "input-field".to_string(),
            text: "hello".to_string(),
        };

        let serialized = serde_json::to_string(&action).unwrap();
        assert!(serialized.contains("input"));
        assert!(serialized.contains("input-field"));
        assert!(serialized.contains("hello"));
    }

    #[test]
    fn test_report_summary_with_mixed_status() {
        let mut steps = vec![
            TestStep::new(
                "1".to_string(),
                "Step 1".to_string(),
                TestAction::Wait { duration_ms: 100 },
            ),
            TestStep::new(
                "2".to_string(),
                "Step 2".to_string(),
                TestAction::Click {
                    ref_id: "btn".to_string(),
                },
            ),
            TestStep::new(
                "3".to_string(),
                "Step 3".to_string(),
                TestAction::Skip {
                    reason: "Not applicable".to_string(),
                },
            ),
        ];

        steps[0].status = StepStatus::Passed;
        steps[1].status = StepStatus::Failed;
        steps[2].status = StepStatus::Skipped;

        let summary = ReportSummary::from_steps(&steps);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert!((summary.pass_rate - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_test_report_new() {
        let start = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let end = DateTime::parse_from_rfc3339("2024-01-01T00:01:30Z")
            .unwrap()
            .with_timezone(&Utc);

        let steps = vec![TestStep::new(
            "1".to_string(),
            "Step 1".to_string(),
            TestAction::Wait { duration_ms: 100 },
        )];

        let report = TestReport::new(
            "task-123".to_string(),
            "https://example.com/app.zip".to_string(),
            start,
            end,
            steps,
        );

        assert_eq!(report.task_id, "task-123");
        assert_eq!(report.duration_secs, 90);
        assert_eq!(report.summary.total, 1);
    }
}
