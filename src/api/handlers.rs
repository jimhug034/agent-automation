//! HTTP 请求处理器
//!
//! 包含所有 API 端点的处理逻辑

use crate::error::AppError;
use crate::models::{TaskStatus, TestRequest, TestTask};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// 应用状态，在所有处理器之间共享
#[derive(Clone)]
pub struct AppState {
    /// 任务管理器，存储所有测试任务
    pub task_manager: Arc<TaskManager>,
    /// 当前活跃任务计数器
    pub active_tasks: Arc<AtomicUsize>,
}

/// 任务管理器，负责存储和管理测试任务
#[derive(Debug, Default)]
pub struct TaskManager {
    /// 任务存储，使用 HashMap 以任务 ID 为键
    tasks: RwLock<HashMap<String, TestTask>>,
}

impl TaskManager {
    /// 创建新的任务管理器
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加新任务
    pub async fn add_task(&self, task: TestTask) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
    }

    /// 获取任务
    pub async fn get_task(&self, id: &str) -> Option<TestTask> {
        let tasks = self.tasks.read().await;
        tasks.get(id).cloned()
    }

    /// 更新任务状态
    pub async fn update_task_status<F>(&self, id: &str, f: F) -> Result<(), AppError>
    where
        F: FnOnce(&mut TaskStatus),
    {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(id)
            .ok_or_else(|| AppError::NotFound(id.to_string()))?;
        f(&mut task.status);
        Ok(())
    }

    /// 删除任务
    pub async fn remove_task(&self, id: &str) -> Result<TestTask, AppError> {
        let mut tasks = self.tasks.write().await;
        tasks
            .remove(id)
            .ok_or_else(|| AppError::NotFound(id.to_string()))
    }

    /// 获取所有任务
    pub async fn list_tasks(&self) -> Vec<TestTask> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }
}

/// 健康检查响应
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// 服务状态
    pub status: String,
    /// 当前活跃任务数
    pub active_tasks: usize,
    /// 服务版本
    pub version: &'static str,
}

/// 测试提交响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TestSubmitResponse {
    /// 任务 ID
    pub task_id: String,
    /// 任务状态
    pub status: TaskStatus,
}

/// 任务状态响应
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatusResponse {
    /// 任务 ID
    pub id: String,
    /// 任务状态
    pub status: TaskStatus,
    /// 包 URL
    pub package_url: String,
    /// 测试目标
    pub test_goals: Vec<String>,
}

/// JSON 报告响应
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonReportResponse {
    /// 任务 ID
    pub task_id: String,
    /// 报告状态
    pub status: String,
    /// 报告 URL（如果可用）
    pub report_url: Option<String>,
    /// 错误信息（如果失败）
    pub error: Option<String>,
}

/// HTML 报告响应
#[derive(Debug, Serialize)]
pub struct HtmlReportResponse {
    /// HTML 内容
    pub html: String,
}

/// 健康检查处理器
///
/// 返回服务健康状态和当前活跃任务数
pub async fn health_check(State(state): State<AppState>) -> Result<Json<HealthResponse>, AppError> {
    let active_count = state.active_tasks.load(Ordering::Relaxed);

    Ok(Json(HealthResponse {
        status: "healthy".to_string(),
        active_tasks: active_count,
        version: env!("CARGO_PKG_VERSION"),
    }))
}

/// 提交测试任务处理器
///
/// 接收测试请求，创建新任务并异步执行
pub async fn submit_test_task(
    State(state): State<AppState>,
    Json(request): Json<TestRequest>,
) -> Result<Json<TestSubmitResponse>, AppError> {
    tracing::info!("收到测试请求: package_url={}, test_goals={:?}",
        request.package_url, request.test_goals);

    // 创建工作目录路径（使用配置的 workspace）
    let workspace = std::path::PathBuf::from("./workspace");
    let task_workspace = workspace.join(uuid::Uuid::new_v4().to_string());
    if let Err(e) = std::fs::create_dir_all(&task_workspace) {
        tracing::error!("创建工作目录失败: {}", e);
        return Err(AppError::LaunchFailed(format!("创建工作目录失败: {}", e)));
    }

    // 创建新任务
    let task = TestTask::new(
        request.package_url.clone(),
        request.test_goals.clone(),
        task_workspace.clone(),
        std::path::PathBuf::from("/tmp/electron"), // 会在解压后更新
        9222,
    );

    let task_id = task.id.clone();
    tracing::info!("创建任务: {}", task_id);

    // 将任务添加到管理器
    state.task_manager.add_task(task.clone()).await;

    // 增加活跃任务计数
    state.active_tasks.fetch_add(1, Ordering::Relaxed);

    // 启动异步执行任务
    let task_manager = state.task_manager.clone();
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        tracing::info!("开始执行任务: {}", task_id_clone);
        execute_task_internal(task_manager, task).await;
    });

    Ok(Json(TestSubmitResponse {
        task_id,
        status: TaskStatus::Pending,
    }))
}

/// 内部任务执行函数
///
/// 执行完整的测试流程：下载 -> 解压 -> 启动 -> 测试 -> 报告
async fn execute_task_internal(
    task_manager: Arc<TaskManager>,
    task: TestTask,
) {
    let task_id = task.id.clone();
    let workspace = task.workspace.clone();

    // 更新状态为下载中
    update_status(&task_manager, &task_id, TaskStatus::Downloading).await;

    // 下载包
    let zip_path = workspace.join("app.zip");
    match crate::installer::download::download_package(&task.package_url, &zip_path).await {
        Ok(_) => tracing::info!("下载完成: {:?}", zip_path),
        Err(e) => {
            tracing::error!("下载失败: {}", e);
            update_status(&task_manager, &task_id, TaskStatus::Failed(e.to_string())).await;
            return;
        }
    }

    // 更新状态为解压中
    update_status(&task_manager, &task_id, TaskStatus::Extracting).await;

    // 解压包
    let package_dir = workspace.join("package");
    match crate::installer::extract::extract_zip(&zip_path, &package_dir).await {
        Ok(_) => tracing::info!("解压完成: {:?}", package_dir),
        Err(e) => {
            tracing::error!("解压失败: {}", e);
            update_status(&task_manager, &task_id, TaskStatus::Failed(e.to_string())).await;
            return;
        }
    }

    // 查找 Electron 可执行文件
    let electron_path = match crate::installer::launch::find_electron_executable(&package_dir) {
        Some(path) => {
            tracing::info!("找到 Electron: {:?}", path);
            path
        }
        None => {
            let msg = format!("未找到 Electron 可执行文件: {:?}", package_dir);
            tracing::error!("{}", msg);
            update_status(&task_manager, &task_id, TaskStatus::Failed(msg.clone())).await;
            return;
        }
    };

    // 更新状态为运行中
    update_status(&task_manager, &task_id, TaskStatus::Running).await;

    // 启动 Electron 应用
    let process = match crate::installer::launch::launch_electron(&electron_path, task.cdp_port).await {
        Ok(process) => {
            tracing::info!("Electron 启动成功: PID={}, CDP Port={}", process.pid, process.cdp_port);
            Some(process)
        }
        Err(e) => {
            tracing::error!("启动 Electron 失败: {}", e);
            update_status(&task_manager, &task_id, TaskStatus::Failed(e.to_string())).await;
            return;
        }
    };

    // TODO: 实际的测试执行逻辑
    // 这里应该连接 CDP，使用 TestAgent 执行测试目标
    tracing::info!("执行测试目标: {:?}", task.test_goals);

    // 等待一段时间模拟测试
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // 清理：终止 Electron 进程
    if let Some(process) = process {
        if let Err(e) = crate::installer::launch::kill_process(process.pid) {
            tracing::warn!("终止进程失败: {}", e);
        }
    }

    // 更新状态为完成
    update_status(&task_manager, &task_id, TaskStatus::Completed).await;
    tracing::info!("任务完成: {}", task_id);
}

/// 更新任务状态的辅助函数
async fn update_status(task_manager: &Arc<TaskManager>, task_id: &str, status: TaskStatus) {
    let _ = task_manager.update_task_status(task_id, |s| {
        *s = status.clone();
    }).await;
}

/// 获取任务状态处理器
///
/// 根据任务 ID 查询当前任务状态
pub async fn get_task_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TaskStatusResponse>, AppError> {
    let task = state
        .task_manager
        .get_task(&id)
        .await
        .ok_or_else(|| AppError::NotFound(id.clone()))?;

    Ok(Json(TaskStatusResponse {
        id: task.id,
        status: task.status,
        package_url: task.package_url,
        test_goals: task.test_goals,
    }))
}

/// 获取 JSON 报告处理器
///
/// 返回指定任务的 JSON 格式测试报告
pub async fn get_report_json(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<JsonReportResponse>, AppError> {
    // 检查任务是否存在
    let task = state
        .task_manager
        .get_task(&id)
        .await
        .ok_or_else(|| AppError::NotFound(id.clone()))?;

    // TODO: 实现从存储读取报告的逻辑
    // 目前返回一个响应表示报告尚未生成
    match task.status {
        TaskStatus::Completed => Ok(Json(JsonReportResponse {
            task_id: id.clone(),
            status: "available".to_string(),
            report_url: Some(format!("/api/test/{}/report", id)),
            error: None,
        })),
        TaskStatus::Failed(ref err) => Ok(Json(JsonReportResponse {
            task_id: id.clone(),
            status: "failed".to_string(),
            report_url: None,
            error: Some(err.clone()),
        })),
        _ => Ok(Json(JsonReportResponse {
            task_id: id.clone(),
            status: "pending".to_string(),
            report_url: None,
            error: None,
        })),
    }
}

/// 获取 HTML 报告处理器
///
/// 返回指定任务的 HTML 格式测试报告
pub async fn get_report_html(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<HtmlReportResponse, AppError> {
    // 检查任务是否存在
    let _task = state
        .task_manager
        .get_task(&id)
        .await
        .ok_or_else(|| AppError::NotFound(id.clone()))?;

    // TODO: 实现从存储读取 HTML 报告的逻辑
    // 目前返回一个简单的占位 HTML
    let html = format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <title>Test Report - {}</title>
    <meta charset="utf-8">
    <style>
        body {{ font-family: Arial, sans-serif; margin: 20px; }}
        .header {{ background: #f0f0f0; padding: 20px; border-radius: 5px; }}
        .pending {{ color: #ff9800; }}
        .running {{ color: #2196f3; }}
        .completed {{ color: #4caf50; }}
        .failed {{ color: #f44336; }}
    </style>
</head>
<body>
    <div class="header">
        <h1>Test Report</h1>
        <p>Task ID: {}</p>
        <p class="pending">Report generation in progress...</p>
    </div>
</body>
</html>
"#,
        id, id
    );

    Ok(HtmlReportResponse { html })
}

/// 删除任务处理器
///
/// 删除指定任务及其相关数据
pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    // 删除任务
    state.task_manager.remove_task(&id).await?;

    // 减少活跃任务计数
    state.active_tasks.fetch_sub(1, Ordering::Relaxed);

    Ok(StatusCode::NO_CONTENT)
}

// 为 HtmlReportResponse 实现 IntoResponse
impl IntoResponse for HtmlReportResponse {
    fn into_response(self) -> Response {
        #[derive(Debug, Serialize)]
        struct HtmlWrapper {
            html: String,
        }

        Json(HtmlWrapper { html: self.html }).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "healthy".to_string(),
            active_tasks: 5,
            version: "0.1.0",
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("5"));
    }

    #[test]
    fn test_task_manager_new() {
        let manager = TaskManager::new();
        // 应该能够创建新管理器
        let tasks = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(manager.list_tasks());
        assert_eq!(tasks.len(), 0);
    }
}
