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
    // TODO: 实现完整的任务创建和执行逻辑
    // 目前返回一个模拟响应

    let task_id = uuid::Uuid::new_v4().to_string();

    // 创建新任务（使用默认值，实际应从配置获取）
    let task = TestTask::new(
        request.package_url.clone(),
        request.test_goals.clone(),
        std::path::PathBuf::from("/tmp/workspace"),
        std::path::PathBuf::from("/tmp/electron"),
        9222,
    );

    // 将任务添加到管理器
    state.task_manager.add_task(task).await;

    // 增加活跃任务计数
    state.active_tasks.fetch_add(1, Ordering::Relaxed);

    Ok(Json(TestSubmitResponse {
        task_id,
        status: TaskStatus::Pending,
    }))
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
