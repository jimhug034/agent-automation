//! HTTP 路由配置
//!
//! 使用 Axum 定义 API 路由和中间件

use crate::api::handlers::{
    delete_task, get_report_html, get_report_json, get_task_status, health_check, submit_test_task,
    AppState,
};
use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

/// 创建 API 路由器
///
/// 配置所有 API 端点和中间件
///
/// # Returns
///
/// 返回配置好的 Axum Router
pub fn create_router() -> Router {
    // 创建应用状态
    let state = AppState {
        task_manager: Arc::new(crate::api::handlers::TaskManager::new()),
        active_tasks: Arc::new(AtomicUsize::new(0)),
    };

    // 配置 CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // 构建路由器
    Router::new()
        // 健康检查端点
        .route("/health", get(health_check))
        // 测试任务相关端点
        .route("/api/test", post(submit_test_task))
        .route("/api/test/:id", get(get_task_status))
        .route("/api/test/:id/report", get(get_report_json))
        .route("/api/test/:id/report/html", get(get_report_html))
        .route("/api/test/:id", delete(delete_task))
        // 静态文件服务（报告文件）
        .nest_service("/reports", tower_http::services::ServeDir::new("reports"))
        // 中间件
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(100 * 1024 * 1024)) // 100MB 限制
        .layer(TraceLayer::new_for_http())
        // 应用状态
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_router() {
        let router = create_router();
        // 路由器应该能够成功创建
        // 实际的端点测试需要集成测试环境
        assert_eq!(true, true);
    }
}
