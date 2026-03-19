//! HTTP API 模块
//!
//! 提供 RESTful API 接口用于提交测试任务、查询状态和获取报告

pub mod routes;
pub mod handlers;

pub use routes::create_router;
