use crate::error::AppError;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Electron 进程信息
#[derive(Debug, Clone)]
pub struct ElectronProcess {
    /// 进程 ID
    pub pid: u32,
    /// Chrome DevTools Protocol 端口
    pub cdp_port: u16,
}

/// 在指定目录中查找 Electron 可执行文件
///
/// # 参数
/// * `dir` - 要搜索的目录路径
///
/// # 返回
/// 找到的可执行文件路径，如果未找到则返回 None
///
/// # 平台差异
/// - **Windows**: 查找 `.exe` 文件（通常在根目录）
/// - **macOS**: 查找 `.app` 包内的可执行文件
pub fn find_electron_executable(dir: &Path) -> Option<PathBuf> {
    tracing::debug!("查找 Electron 可执行文件: {:?}", dir);

    if cfg!(target_os = "windows") {
        // Windows: 查找 .exe 文件
        // 常见位置: app.exe, electron.exe, <app-name>.exe
        let exe_names = [
            "electron.exe",
            "app.exe",
            "AgentDesktop.exe",
            "ClaudeDesktop.exe",
        ];

        for name in &exe_names {
            let exe_path = dir.join(name);
            if exe_path.exists() {
                tracing::info!("找到 Electron 可执行文件: {:?}", exe_path);
                return Some(exe_path);
            }
        }

        // 递归搜索子目录（深度 1）
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    for name in &exe_names {
                        let exe_path = path.join(name);
                        if exe_path.exists() {
                            tracing::info!("找到 Electron 可执行文件: {:?}", exe_path);
                            return Some(exe_path);
                        }
                    }
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        // macOS: 查找 .app 包
        // 结构: AppName.app/Contents/MacOS/app-name
        let app_names = [
            "Electron.app",
            "app.app",
            "Agent Desktop.app",
            "Claude Desktop.app",
        ];

        // 首先在当前目录查找 .app
        for app_name in &app_names {
            let app_path = dir.join(app_name);
            if app_path.exists() {
                let exe_path = app_path.join("Contents/MacOS/");
                if let Some(exe_name) = app_name.strip_suffix(".app") {
                    // 转换为小写并处理空格
                    let exe_name = exe_name.to_lowercase().replace(' ', "-");
                    let full_exe = exe_path.join(&exe_name);
                    if full_exe.exists() {
                        tracing::info!("找到 Electron 可执行文件: {:?}", full_exe);
                        return Some(full_exe);
                    }
                }
            }
        }

        // 搜索所有 .app 目录
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("app") {
                    // 这是 .app 包
                    let contents_path = path.join("Contents/MacOS");
                    if contents_path.exists() {
                        // 查找包内的可执行文件
                        if let Ok(mac_entries) = std::fs::read_dir(&contents_path) {
                            for mac_entry in mac_entries.flatten() {
                                let exe_path = mac_entry.path();
                                if exe_path.file_name().and_then(|s| s.to_str()).map(|s| !s.starts_with('.')) == Some(true) {
                                    tracing::info!("找到 Electron 可执行文件: {:?}", exe_path);
                                    return Some(exe_path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 检查是否直接指向 .app 包内的可执行位置
        if dir.ends_with("MacOS") {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let exe_path = entry.path();
                    if exe_path.file_name().and_then(|s| s.to_str()).map(|s| !s.starts_with('.')) == Some(true) {
                        tracing::info!("找到 Electron 可执行文件: {:?}", exe_path);
                        return Some(exe_path);
                    }
                }
            }
        }
    } else {
        // Linux: 查找无扩展名的可执行文件
        let exe_names = [
            "electron",
            "app",
            "agent-desktop",
            "claude-desktop",
        ];

        for name in &exe_names {
            let exe_path = dir.join(name);
            if exe_path.exists() {
                tracing::info!("找到 Electron 可执行文件: {:?}", exe_path);
                return Some(exe_path);
            }
        }
    }

    tracing::warn!("未找到 Electron 可执行文件: {:?}", dir);
    None
}

/// 启动 Electron 应用并返回进程信息
///
/// # 参数
/// * `app_dir` - 应用目录
/// * `cdp_port` - Chrome DevTools Protocol 端口
///
/// # 返回
/// 成功时返回 `ElectronProcess`，包含 PID 和 CDP 端口
///
/// # 注意
/// 此函数会以 `--remote-debugging-port` 参数启动 Electron，
/// 允许后续通过 CDP 协议控制应用
pub async fn launch_electron(
    app_dir: &Path,
    cdp_port: u16,
) -> Result<ElectronProcess, AppError> {
    tracing::info!("启动 Electron 应用: {:?}", app_dir);

    // 查找可执行文件
    let exe_path = find_electron_executable(app_dir).ok_or_else(|| {
        AppError::LaunchFailed(format!("未找到 Electron 可执行文件: {:?}", app_dir))
    })?;

    tracing::debug!("可执行文件: {:?}", exe_path);

    // 准备启动参数
    let mut cmd = Command::new(&exe_path);

    // 设置远程调试端口（用于 CDP 控制）
    cmd.arg(format!("--remote-debugging-port={}", cdp_port));

    // 其他有用的调试参数
    cmd.arg("--no-first-run");
    cmd.arg("--no-default-browser-check");

    // macOS .app 包需要特殊处理
    if cfg!(target_os = "macos") {
        if exe_path.to_string_lossy().contains(".app/Contents/MacOS/") {
            // 已经是完整路径，不需要 open 命令
        }
    }

    // Windows 和 Linux 直接启动
    tracing::debug!("启动命令: {:?}", cmd);

    // 启动进程
    let child = cmd.spawn()?;

    // 获取进程 ID（tokio::process::Child::id() 返回值在不同版本可能不同）
    // 在稳定版中返回 u32，在 nightly 中可能返回 Option<u32>
    let pid = match child.id() {
        Some(id) => id,
        None => {
            return Err(AppError::LaunchFailed(
                "无法获取进程 ID".to_string()
            ));
        }
    };

    tracing::info!("Electron 进程已启动 (PID: {}), CDP 端口: {}", pid, cdp_port);

    // 注意: child 在这里离开作用域，不会等待进程结束
    // 如果需要跟踪进程，应该将 child 存储在其他地方

    Ok(ElectronProcess { pid, cdp_port })
}

/// 终止指定 PID 的进程
///
/// # 参数
/// * `pid` - 要终止的进程 ID
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回相应的错误
///
/// # 平台差异
/// - **Windows**: 使用 `taskkill` 命令
/// - **Unix/macOS**: 发送 SIGTERM 信号
pub fn kill_process(pid: u32) -> Result<(), AppError> {
    tracing::info!("终止进程: PID {}", pid);

    if cfg!(target_os = "windows") {
        // Windows: 使用 taskkill 命令
        let output = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            tracing::error!("终止进程失败: {}", error_msg);
            return Err(AppError::LaunchFailed(format!(
                "无法终止进程 {}: {}",
                pid, error_msg
            )));
        }
    } else {
        // Unix/macOS: 使用 kill 命令
        let output = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("终止进程警告: {}", error_msg);
            // 进程可能已经不存在，不算严重错误
            if !error_msg.contains("No such process") {
                return Err(AppError::LaunchFailed(format!(
                    "无法终止进程 {}: {}",
                    pid, error_msg
                )));
            }
        }
    }

    tracing::info!("进程已终止: PID {}", pid);
    Ok(())
}

/// 检查指定 PID 的进程是否正在运行
///
/// # 参数
/// * `pid` - 要检查的进程 ID
///
/// # 返回
/// 如果进程正在运行返回 `true`，否则返回 `false`
pub fn is_process_running(pid: u32) -> bool {
    if cfg!(target_os = "windows") {
        // Windows: 使用 tasklist 命令
        let output = match std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
        {
            Ok(o) => o,
            Err(_) => return false,
        };

        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str.contains(&pid.to_string())
    } else {
        // Unix/macOS: 使用 kill -0 (仅检查进程是否存在)
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

/// 获取绑定到指定端口的进程 ID
///
/// # 参数
/// * `port` - 要检查的端口号
///
/// # 返回
/// 如果找到则返回进程 ID，否则返回 None
pub fn find_process_by_port(port: u16) -> Option<u32> {
    tracing::debug!("查找占用端口 {} 的进程", port);

    if cfg!(target_os = "windows") {
        // Windows: 使用 netstat
        if let Ok(output) = std::process::Command::new("netstat")
            .args(["-ano"])
            .output()
        {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(pid_str) = parts.last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            tracing::debug!("端口 {} 的进程: {}", port, pid);
                            return Some(pid);
                        }
                    }
                }
            }
        }
    } else {
        // Unix/macOS: 使用 lsof
        if let Ok(output) = std::process::Command::new("lsof")
            .args(["-ti", &format!(":{}", port)])
            .output()
        {
            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !pid_str.is_empty() {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    tracing::debug!("端口 {} 的进程: {}", port, pid);
                    return Some(pid);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_process_running() {
        // 测试检查自己的进程
        let own_pid = std::process::id();
        assert!(is_process_running(own_pid));

        // 测试一个不太可能存在的 PID
        assert!(!is_process_running(999999));
    }

    #[test]
    fn test_find_electron_executable_in_nonexistent_dir() {
        let nonexistent = PathBuf::from("/this/path/does/not/exist");
        assert!(find_electron_executable(&nonexistent).is_none());
    }
}
