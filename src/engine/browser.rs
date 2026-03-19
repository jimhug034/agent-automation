//! Agent Browser 封装模块
//!
//! 通过 subprocess 调用 agent-browser CLI，提供浏览器控制功能

use serde::{Deserialize, Serialize};
use std::process::{Command, Output};
use std::time::Duration;
use tracing::{debug, error, info};

use crate::error::AppError;

/// Agent Browser CLI 封装
///
/// 通过调用 agent-browser 二进制文件来控制浏览器
pub struct AgentBrowser {
    /// agent-browser 二进制文件路径
    bin_path: String,
    /// Chrome DevTools Protocol 端口
    cdp_port: u16,
    /// 连接状态
    connected: bool,
}

impl AgentBrowser {
    /// 创建新的 AgentBrowser 实例
    ///
    /// # Arguments
    ///
    /// * `bin_path` - agent-browser 二进制文件的路径
    /// * `cdp_port` - Chrome DevTools Protocol 端口
    pub fn new<S: Into<String>>(bin_path: S, cdp_port: u16) -> Self {
        Self {
            bin_path: bin_path.into(),
            cdp_port,
            connected: false,
        }
    }

    /// 连接到浏览器
    ///
    /// 通过 CDP 端口建立与浏览器的连接
    ///
    /// # Returns
    ///
    /// 返回连接成功或失败
    pub fn connect(&mut self) -> Result<(), AppError> {
        info!("尝试连接浏览器, CDP 端口: {}", self.cdp_port);

        let port = self.cdp_port.to_string();
        let output = self
            .execute_command(&["--port", &port, "connect"])
            .map_err(|e| {
                error!("浏览器连接命令执行失败: {}", e);
                AppError::CdpConnectionFailed(format!("连接命令执行失败: {}", e))
            })?;

        if output.status.success() {
            self.connected = true;
            info!("浏览器连接成功");
            Ok(())
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("浏览器连接失败: {}", error_msg);
            Err(AppError::CdpConnectionFailed(error_msg.to_string()))
        }
    }

    /// 获取浏览器快照
    ///
    /// 捕获当前页面状态，包括 DOM 结构和元素信息
    ///
    /// # Returns
    ///
    /// 返回浏览器快照
    pub fn snapshot(&self) -> Result<BrowserSnapshot, AppError> {
        debug!("获取浏览器快照");

        let port = self.cdp_port.to_string();
        let output = self
            .execute_command(&["--port", &port, "snapshot", "--format", "json"])
            .map_err(|e| {
                error!("获取快照命令执行失败: {}", e);
                AppError::BrowserCommandFailed(format!("snapshot 命令失败: {}", e))
            })?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("获取快照失败: {}", error_msg);
            return Err(AppError::BrowserCommandFailed(format!(
                "snapshot 失败: {}",
                error_msg
            )));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let snapshot: BrowserSnapshot = serde_json::from_str(&json_str).map_err(|e| {
            error!("解析快照 JSON 失败: {}, 输出: {}", e, json_str);
            AppError::BrowserCommandFailed(format!("解析快照失败: {}", e))
        })?;

        debug!("成功获取快照, URL: {}", snapshot.url);
        Ok(snapshot)
    }

    /// 点击元素
    ///
    /// 通过引用 ID 点击页面元素
    ///
    /// # Arguments
    ///
    /// * `ref_id` - 元素的引用 ID
    pub fn click(&self, ref_id: &str) -> Result<(), AppError> {
        info!("点击元素: {}", ref_id);

        let port = self.cdp_port.to_string();
        let output = self
            .execute_command(&["--port", &port, "click", "--ref-id", ref_id])
            .map_err(|e| {
                error!("点击命令执行失败: {}", e);
                AppError::BrowserCommandFailed(format!("click 命令失败: {}", e))
            })?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("点击元素失败: {}", error_msg);
            return Err(AppError::StepExecutionFailed(format!(
                "点击失败 (ref_id={}): {}",
                ref_id, error_msg
            )));
        }

        debug!("成功点击元素: {}", ref_id);
        Ok(())
    }

    /// 输入文本
    ///
    /// 向元素输入文本内容
    ///
    /// # Arguments
    ///
    /// * `ref_id` - 目标元素的引用 ID
    /// * `text` - 要输入的文本
    pub fn input(&self, ref_id: &str, text: &str) -> Result<(), AppError> {
        info!("向元素输入文本: ref_id={}, text={}", ref_id, text);

        let port = self.cdp_port.to_string();
        let output = self
            .execute_command(&["--port", &port, "input", "--ref-id", ref_id, "--text", text])
            .map_err(|e| {
                error!("输入命令执行失败: {}", e);
                AppError::BrowserCommandFailed(format!("input 命令失败: {}", e))
            })?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("输入文本失败: {}", error_msg);
            return Err(AppError::StepExecutionFailed(format!(
                "输入失败 (ref_id={}): {}",
                ref_id, error_msg
            )));
        }

        debug!("成功输入文本到元素: {}", ref_id);
        Ok(())
    }

    /// 截图
    ///
    /// 捕获当前页面的屏幕截图
    ///
    /// # Arguments
    ///
    /// * `output_path` - 截图保存路径（可选）
    ///
    /// # Returns
    ///
    /// 返回截图的 base64 编码数据
    pub fn screenshot(&self, output_path: Option<&str>) -> Result<String, AppError> {
        debug!("捕获截图");

        let port_str = self.cdp_port.to_string();
        let mut args = vec!["--port", &port_str, "screenshot", "--format", "base64"];

        if let Some(path) = output_path {
            args.extend_from_slice(&["--output", path]);
        }

        let output = self.execute_command(&args).map_err(|e| {
            error!("截图命令执行失败: {}", e);
            AppError::BrowserCommandFailed(format!("screenshot 命令失败: {}", e))
        })?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("截图失败: {}", error_msg);
            return Err(AppError::BrowserCommandFailed(format!(
                "截图失败: {}",
                error_msg
            )));
        }

        let base64_data = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!("成功捕获截图, 数据长度: {}", base64_data.len());
        Ok(base64_data)
    }

    /// 等待
    ///
    /// 等待指定的时间
    ///
    /// # Arguments
    ///
    /// * `duration_ms` - 等待时间（毫秒）
    pub fn wait(&self, duration_ms: u32) -> Result<(), AppError> {
        debug!("等待 {}ms", duration_ms);
        std::thread::sleep(Duration::from_millis(duration_ms as u64));
        debug!("等待完成");
        Ok(())
    }

    /// 导航到 URL
    ///
    /// 让浏览器导航到指定的 URL
    ///
    /// # Arguments
    ///
    /// * `url` - 目标 URL
    pub fn navigate(&self, url: &str) -> Result<(), AppError> {
        info!("导航到 URL: {}", url);

        let port = self.cdp_port.to_string();
        let output = self
            .execute_command(&["--port", &port, "navigate", "--url", url])
            .map_err(|e| {
                error!("导航命令执行失败: {}", e);
                AppError::BrowserCommandFailed(format!("navigate 命令失败: {}", e))
            })?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("导航失败: {}", error_msg);
            return Err(AppError::StepExecutionFailed(format!(
                "导航失败: {}",
                error_msg
            )));
        }

        debug!("成功导航到: {}", url);
        Ok(())
    }

    /// 检查连接状态
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// 执行 agent-browser 命令
    ///
    /// # Arguments
    ///
    /// * `args` - 命令参数
    ///
    /// # Returns
    ///
    /// 返回命令输出
    fn execute_command(&self, args: &[&str]) -> Result<Output, std::io::Error> {
        debug!(
            "执行 agent-browser 命令: {} {}",
            self.bin_path,
            args.join(" ")
        );

        let output = Command::new(&self.bin_path).args(args).output()?;

        Ok(output)
    }

    /// 获取 CDP 端口
    pub fn cdp_port(&self) -> u16 {
        self.cdp_port
    }
}

/// 浏览器快照
///
/// 包含页面的当前状态信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSnapshot {
    /// 页面 URL
    pub url: String,
    /// 页面标题
    pub title: String,
    /// 页面元素列表
    pub elements: Vec<ElementInfo>,
    /// 页面文本内容
    pub text_content: String,
}

/// 元素信息
///
/// 描述页面中的可交互元素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInfo {
    /// 元素引用 ID（用于后续操作）
    pub ref_id: String,
    /// 元素类型（button, input, link 等）
    pub element_type: String,
    /// 元素标签名
    pub tag_name: String,
    /// 元素文本内容
    pub text: String,
    /// 元素位置信息
    pub position: ElementPosition,
    /// 元素属性
    pub attributes: Vec<Attribute>,
}

/// 元素位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementPosition {
    /// X 坐标
    pub x: f64,
    /// Y 坐标
    pub y: f64,
    /// 宽度
    pub width: f64,
    /// 高度
    pub height: f64,
}

/// 元素属性
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    /// 属性名
    pub name: String,
    /// 属性值
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_browser_new() {
        let browser = AgentBrowser::new("/path/to/agent-browser", 9222);
        assert_eq!(browser.bin_path, "/path/to/agent-browser");
        assert_eq!(browser.cdp_port, 9222);
        assert!(!browser.is_connected());
    }

    #[test]
    fn test_element_info_serialization() {
        let element = ElementInfo {
            ref_id: "btn-1".to_string(),
            element_type: "button".to_string(),
            tag_name: "button".to_string(),
            text: "Click me".to_string(),
            position: ElementPosition {
                x: 100.0,
                y: 200.0,
                width: 80.0,
                height: 30.0,
            },
            attributes: vec![Attribute {
                name: "id".to_string(),
                value: "submit-btn".to_string(),
            }],
        };

        let json = serde_json::to_string(&element).unwrap();
        assert!(json.contains("\"ref_id\":\"btn-1\""));
        assert!(json.contains("\"text\":\"Click me\""));
    }

    #[test]
    fn test_browser_snapshot_serialization() {
        let snapshot = BrowserSnapshot {
            url: "https://example.com".to_string(),
            title: "Example Page".to_string(),
            elements: vec![],
            text_content: "Hello world".to_string(),
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"url\":\"https://example.com\""));
        assert!(json.contains("\"title\":\"Example Page\""));
    }

    #[test]
    fn test_element_position() {
        let pos = ElementPosition {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 50.0,
        };

        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
        assert_eq!(pos.width, 100.0);
        assert_eq!(pos.height, 50.0);
    }
}
