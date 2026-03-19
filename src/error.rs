use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    // 下载相关
    DownloadFailed(String),
    ExtractFailed(String),

    // 安装相关
    InstallFailed(String),
    LaunchFailed(String),
    CdpConnectionFailed(String),

    // 测试相关
    LlmApiError(String),
    BrowserCommandFailed(String),
    StepExecutionFailed(String),

    // 报告相关
    ReportGenerationFailed(String),
    FeishuPushFailed(String),

    // 通用
    NotFound(String),
    InvalidRequest(String),
    InternalError(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::DownloadFailed(url) => write!(f, "下载失败: {}", url),
            AppError::ExtractFailed(e) => write!(f, "解压失败: {}", e),
            AppError::InstallFailed(e) => write!(f, "安装失败: {}", e),
            AppError::LaunchFailed(e) => write!(f, "启动失败: {}", e),
            AppError::CdpConnectionFailed(e) => write!(f, "CDP连接失败: {}", e),
            AppError::LlmApiError(e) => write!(f, "LLM API错误: {}", e),
            AppError::BrowserCommandFailed(e) => write!(f, "浏览器命令失败: {}", e),
            AppError::StepExecutionFailed(e) => write!(f, "步骤执行失败: {}", e),
            AppError::ReportGenerationFailed(e) => write!(f, "报告生成失败: {}", e),
            AppError::FeishuPushFailed(e) => write!(f, "飞书推送失败: {}", e),
            AppError::NotFound(id) => write!(f, "任务不存在: {}", id),
            AppError::InvalidRequest(msg) => write!(f, "请求无效: {}", msg),
            AppError::InternalError(e) => write!(f, "内部错误: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = json!({
            "error": error_message,
            "type": std::any::type_name::<AppError>()
        });

        (status, Json(body)).into_response()
    }
}

// From impls
impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::DownloadFailed(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::InternalError(err.to_string())
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        AppError::ExtractFailed(err.to_string())
    }
}
