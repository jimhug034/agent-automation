//! 任务执行器模块
//!
//! 负责执行测试任务，包括下载、解压、启动应用、执行测试、生成报告等

use crate::error::AppError;
use crate::installer::download::download_package;
use crate::installer::extract::extract_zip;
use crate::installer::launch::{find_electron_executable, kill_process, launch_electron};
use crate::models::{TaskStatus, TestReport, TestStep};
use crate::orchestrator::store::TaskStore;
use crate::reporter::feishu::{send_feishu_notification, TestSummary};
use crate::reporter::html::generate_html_report;
use chrono::Utc;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, instrument, warn};

/// 任务执行器
///
/// 负责执行完整的测试任务流程
pub struct TaskExecutor {
    /// 任务存储
    tasks: TaskStore,
    /// 工作目录
    workspace: PathBuf,
    /// 报告输出目录
    reports_dir: PathBuf,
    /// HTML 报告模板路径
    html_template: PathBuf,
    /// 飞书 Webhook URL（可选）
    feishu_webhook: Option<String>,
}

impl TaskExecutor {
    /// 创建新的任务执行器
    ///
    /// # Arguments
    ///
    /// * `tasks` - 任务存储
    /// * `workspace` - 工作目录
    /// * `reports_dir` - 报告输出目录
    /// * `html_template` - HTML 报告模板路径
    /// * `feishu_webhook` - 可选的飞书 Webhook URL
    pub fn new(
        tasks: TaskStore,
        workspace: PathBuf,
        reports_dir: PathBuf,
        html_template: PathBuf,
        feishu_webhook: Option<String>,
    ) -> Self {
        Self {
            tasks,
            workspace,
            reports_dir,
            html_template,
            feishu_webhook,
        }
    }

    /// 执行指定 ID 的任务
    ///
    /// # Arguments
    ///
    /// * `task_id` - 要执行的任务 ID
    ///
    /// # Returns
    ///
    /// 成功时返回 Ok(())，失败时返回相应的错误
    #[instrument(skip(self), fields(task_id))]
    pub async fn execute(&self, task_id: &str) -> Result<(), AppError> {
        info!("开始执行任务: {}", task_id);

        // 1. 获取任务
        let task = self.get_task(task_id).await?;
        let package_url = task.package_url.clone();
        let cdp_port = task.cdp_port;
        let task_workspace = task.workspace.clone();
        let test_goals = task.test_goals.clone();

        // 记录开始时间
        let start_time = Utc::now();
        let mut electron_pid: Option<u32> = None;

        // 准备工作目录
        let package_dir = task_workspace.join("package");
        let zip_path = task_workspace.join("app.zip");

        // 2. 下载 ZIP
        info!("下载包: {}", package_url);
        self.update_task_status(task_id, TaskStatus::Downloading)
            .await;
        if let Err(e) = download_package(&package_url, &zip_path).await {
            error!("下载失败: {}", e);
            return self.fail_task(task_id, format!("下载失败: {}", e)).await;
        }
        debug!("下载完成: {:?}", zip_path);

        // 3. 解压 ZIP
        info!("解压包到: {:?}", package_dir);
        self.update_task_status(task_id, TaskStatus::Extracting)
            .await;
        if let Err(e) = extract_zip(&zip_path, &package_dir).await {
            error!("解压失败: {}", e);
            return self.fail_task(task_id, format!("解压失败: {}", e)).await;
        }
        debug!("解压完成");

        // 4. 查找 Electron 可执行文件
        info!("查找 Electron 可执行文件");
        let electron_path = find_electron_executable(&package_dir).ok_or_else(|| {
            AppError::LaunchFailed(format!("未找到 Electron 可执行文件: {:?}", package_dir))
        })?;
        debug!("找到 Electron: {:?}", electron_path);

        // 5. 启动 Electron
        info!("启动 Electron 应用，CDP 端口: {}", cdp_port);
        self.update_task_status(task_id, TaskStatus::Running).await;

        let electron_process = match launch_electron(&package_dir, cdp_port).await {
            Ok(process) => process,
            Err(e) => {
                error!("启动 Electron 失败: {}", e);
                return self
                    .fail_task(task_id, format!("启动应用失败: {}", e))
                    .await;
            }
        };

        electron_pid = Some(electron_process.pid);
        info!("Electron 进程已启动 (PID: {:?})", electron_pid);

        // 等待应用启动
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // 6. TODO: 执行测试 (暂跳过)
        // 这里是测试执行的核心逻辑，需要：
        // - 连接 CDP
        // - 调用 LLM 生成测试步骤
        // - 执行测试步骤
        // - 收集截图和结果
        info!("执行测试 (当前为占位实现)");

        // 创建占位测试步骤
        let steps = self.create_placeholder_steps(&test_goals);

        // 7. 终止进程
        if let Some(pid) = electron_pid {
            info!("终止 Electron 进程: {}", pid);
            if let Err(e) = kill_process(pid) {
                warn!("终止进程失败: {}", e);
                // 不返回错误，继续生成报告
            } else {
                debug!("进程已终止");
            }
        }

        // 8. 生成报告
        info!("生成测试报告");
        let end_time = Utc::now();
        let report = TestReport::new(
            task_id.to_string(),
            package_url,
            start_time,
            end_time,
            steps,
        );

        let report_path = self.reports_dir.join(format!("report-{}.html", task_id));
        if let Err(e) = generate_html_report(&report, &self.html_template, &report_path) {
            warn!("生成 HTML 报告失败: {}", e);
            // 不返回错误，继续执行
        } else {
            info!("报告已生成: {:?}", report_path);
        }

        // 9. 发送飞书通知
        if let Some(webhook_url) = &self.feishu_webhook {
            info!("发送飞书通知");
            let summary = TestSummary {
                total: report.summary.total,
                passed: report.summary.passed,
                failed: report.summary.failed,
                pass_rate: report.summary.pass_rate,
            };

            if let Err(e) = send_feishu_notification(
                webhook_url,
                task_id,
                &summary,
                report.duration_secs,
                None, // TODO: 添加报告 URL
            )
            .await
            {
                warn!("飞书通知发送失败: {}", e);
                // 不返回错误，继续执行
            } else {
                info!("飞书通知已发送");
            }
        }

        // 10. 更新状态为 Completed
        self.update_task_status(task_id, TaskStatus::Completed)
            .await;
        info!("任务执行完成: {}", task_id);

        Ok(())
    }

    /// 获取任务
    async fn get_task(&self, task_id: &str) -> Result<crate::models::TestTask, AppError> {
        let tasks = self.tasks.read().await;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(task_id.to_string()))
    }

    /// 更新任务状态
    async fn update_task_status(&self, task_id: &str, status: TaskStatus) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            debug!("更新任务状态: {} -> {:?}", task_id, status);
            task.status = status;
        }
    }

    /// 标记任务失败
    async fn fail_task(&self, task_id: &str, error_msg: String) -> Result<(), AppError> {
        self.update_task_status(task_id, TaskStatus::Failed(error_msg.clone()))
            .await;
        Err(AppError::InternalError(error_msg))
    }

    /// 创建占位测试步骤
    ///
    /// TODO: 替换为实际的测试执行逻辑
    fn create_placeholder_steps(&self, test_goals: &[String]) -> Vec<TestStep> {
        let mut steps = Vec::new();

        // 为每个测试目标创建占位步骤
        for (index, goal) in test_goals.iter().enumerate() {
            let step = TestStep::new(
                format!("step-{}", index),
                format!("测试目标: {}", goal),
                crate::models::TestAction::Skip {
                    reason: "测试执行器待实现".to_string(),
                },
            );
            steps.push(step);
        }

        // 添加一个启动应用的步骤
        let launch_step = TestStep::new(
            "launch".to_string(),
            "启动 Electron 应用".to_string(),
            crate::models::TestAction::Skip {
                reason: "应用已启动".to_string(),
            },
        );
        steps.insert(0, launch_step);

        steps
    }
}

/// 从配置创建任务执行器的便捷函数
///
/// # Arguments
///
/// * `tasks` - 任务存储
/// * `workspace` - 工作目录
/// * `reports_dir` - 报告输出目录
/// * `html_template` - HTML 报告模板路径
/// * `feishu_webhook` - 可选的飞书 Webhook URL
///
/// # Returns
///
/// 返回配置好的 TaskExecutor 实例
pub fn create_executor(
    tasks: TaskStore,
    workspace: &Path,
    reports_dir: &Path,
    html_template: &Path,
    feishu_webhook: Option<String>,
) -> TaskExecutor {
    TaskExecutor::new(
        tasks,
        workspace.to_path_buf(),
        reports_dir.to_path_buf(),
        html_template.to_path_buf(),
        feishu_webhook,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TestTask;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;

    fn create_test_store() -> TaskStore {
        Arc::new(RwLock::new(HashMap::new()))
    }

    fn create_test_executor() -> (TaskExecutor, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        let reports_dir = temp_dir.path().join("reports");
        let html_template = temp_dir.path().join("template.html");

        // 创建目录
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&reports_dir).unwrap();

        // 创建空的 HTML 模板
        std::fs::write(&html_template, "<html><body>{{task_id}}</body></html>").unwrap();

        let tasks = create_test_store();
        let executor = TaskExecutor::new(
            tasks,
            workspace,
            reports_dir,
            html_template,
            None, // 不发送飞书通知
        );

        (executor, temp_dir)
    }

    #[tokio::test]
    async fn test_create_placeholder_steps() {
        let (executor, _temp) = create_test_executor();

        let steps = executor
            .create_placeholder_steps(&vec!["Test login".to_string(), "Test logout".to_string()]);

        // 应该有 3 个步骤：launch + 2 个测试目标
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].description, "启动 Electron 应用");
        assert_eq!(steps[1].description, "测试目标: Test login");
        assert_eq!(steps[2].description, "测试目标: Test logout");
    }

    #[tokio::test]
    async fn test_get_task() {
        let (executor, _temp) = create_test_executor();

        // 添加一个测试任务
        let task = TestTask::new(
            "https://example.com/app.zip".to_string(),
            vec!["Test".to_string()],
            PathBuf::from("/tmp/workspace"),
            PathBuf::from("/tmp/electron"),
            9222,
        );

        let task_id = task.id.clone();
        let mut tasks = executor.tasks.write().await;
        tasks.insert(task_id.clone(), task);
        drop(tasks);

        // 获取任务
        let result = executor.get_task(&task_id).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, task_id);
    }

    #[tokio::test]
    async fn test_update_task_status() {
        let (executor, _temp) = create_test_executor();

        // 添加一个测试任务
        let task = TestTask::new(
            "https://example.com/app.zip".to_string(),
            vec!["Test".to_string()],
            PathBuf::from("/tmp/workspace"),
            PathBuf::from("/tmp/electron"),
            9222,
        );

        let task_id = task.id.clone();
        let mut tasks = executor.tasks.write().await;
        tasks.insert(task_id.clone(), task);
        drop(tasks);

        // 更新状态
        executor
            .update_task_status(&task_id, TaskStatus::Downloading)
            .await;

        // 验证状态
        let tasks = executor.tasks.read().await;
        let updated = tasks.get(&task_id).unwrap();
        assert_eq!(updated.status, TaskStatus::Downloading);
    }

    #[test]
    fn test_create_executor() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("workspace");
        let reports_dir = temp_dir.path().join("reports");
        let html_template = temp_dir.path().join("template.html");

        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(&reports_dir).unwrap();
        std::fs::write(&html_template, "<html></html>").unwrap();

        let executor = create_executor(
            create_test_store(),
            &workspace,
            &reports_dir,
            &html_template,
            Some("https://feishu.webhook".to_string()),
        );

        assert_eq!(executor.workspace, workspace);
        assert_eq!(executor.reports_dir, reports_dir);
        assert!(executor.feishu_webhook.is_some());
    }
}
