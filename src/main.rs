#![allow(dead_code)]

mod api;
mod config;
mod engine;
mod error;
mod installer;
mod llm;
mod logging;
mod models;
mod orchestrator;
mod reporter;

use api::create_router;
use config::Settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 加载 .env 文件（优先级：.env.local > .env）
    dotenv::dotenv().ok();

    // 加载配置
    let settings = Settings::load()?;

    // 初始化日志
    logging::init_logging(&settings)?;
    tracing::info!("Agent Automation 服务启动中...");
    tracing::info!("配置: {}:{}", settings.server.host, settings.server.port);

    // 创建工作目录
    std::fs::create_dir_all(settings.workspace_path())?;
    std::fs::create_dir_all(settings.reports_path())?;
    std::fs::create_dir_all(settings.logs_path())?;

    // 创建路由
    let app = create_router();

    // 启动服务器
    let addr = format!("{}:{}", settings.server.host, settings.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("服务监听: {}", addr);
    tracing::info!("API 端点:");
    tracing::info!("  GET  /health");
    tracing::info!("  POST /api/test");
    tracing::info!("  GET  /api/test/:id");
    tracing::info!("  GET  /api/test/:id/report");
    tracing::info!("  GET  /api/test/:id/report/html");
    tracing::info!("  DELETE /api/test/:id");

    axum::serve(listener, app).await?;

    Ok(())
}
