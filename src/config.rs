use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub server: ServerSettings,
    pub task: TaskSettings,
    pub llm: LlmSettings,
    pub agent_browser: AgentBrowserSettings,
    pub report: ReportSettings,
    pub feishu: FeishuSettings,
    pub logging: LoggingSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub workers: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TaskSettings {
    pub workspace_dir: String,
    pub cdp_port: u16,
    pub default_timeout: u64,
    pub default_retries: u32,
    pub max_concurrent_tasks: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmSettings {
    pub default_model: String,
    pub timeout: u64,
    pub max_retries: u32,
    pub openai: OpenAiSettings,
    pub claude: ClaudeSettings,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OpenAiSettings {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClaudeSettings {
    pub api_base: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentBrowserSettings {
    pub bin_path: String,
    pub snapshot_interval: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ReportSettings {
    pub output_dir: String,
    pub html_template: String,
    pub screenshot_format: String,
    pub keep_screenshots: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeishuSettings {
    pub webhook_url: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingSettings {
    pub level: String,
    pub dir: String,
    pub max_files: usize,
}

impl Settings {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let mut settings = Config::builder()
            .add_source(File::with_name("config/settings"))
            .add_source(Environment::default().separator("__"))
            .build()?;

        // 展开环境变量
        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            settings.set("llm.openai.api_key", api_key)?;
        }
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            settings.set("llm.claude.api_key", api_key)?;
        }
        if let Ok(webhook) = std::env::var("FEISHU_WEBHOOK") {
            settings.set("feishu.webhook_url", webhook)?;
        }

        Ok(settings.try_deserialize()?)
    }

    pub fn workspace_path(&self) -> PathBuf {
        PathBuf::from(&self.task.workspace_dir)
    }

    pub fn reports_path(&self) -> PathBuf {
        PathBuf::from(&self.report.output_dir)
    }

    pub fn logs_path(&self) -> PathBuf {
        PathBuf::from(&self.logging.dir)
    }
}
