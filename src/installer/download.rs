use crate::error::AppError;
use futures_util::StreamExt;
use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// 下载指定 URL 的包到目标路径
///
/// # 参数
/// * `url` - 要下载的包的 URL
/// * `destination` - 下载文件保存的目标路径
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回相应的错误
pub async fn download_package(url: &str, destination: &Path) -> Result<(), AppError> {
    tracing::info!("开始下载包: {} -> {:?}", url, destination);

    // 创建父目录
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
        tracing::debug!("创建目录: {:?}", parent);
    }

    // 创建 HTTP 客户端
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300)) // 5 分钟超时
        .build()?;

    // 发起下载请求
    tracing::debug!("发送 HTTP 请求: {}", url);
    let response = client.get(url).send().await?;

    // 检查响应状态
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "无法读取错误响应".to_string());
        tracing::error!("下载失败: {} - {}", status, error_text);
        return Err(AppError::DownloadFailed(format!(
            "HTTP {}: {}",
            status, error_text
        )));
    }

    // 获取文件总大小（如果有的话）
    let total_size = response.content_length();
    if let Some(size) = total_size {
        tracing::info!("文件大小: {} bytes", size);
    }

    // 创建目标文件
    let mut file = File::create(destination).await?;
    tracing::debug!("创建文件: {:?}", destination);

    // 获取响应的字节流
    let mut bytes_stream = response.bytes_stream();

    // 下载并写入文件，带进度跟踪
    let mut downloaded: u64 = 0;
    let mut last_log_time = std::time::Instant::now();

    while let Some(chunk_result) = bytes_stream.next().await {
        let chunk: bytes::Bytes = chunk_result?;
        downloaded += chunk.len() as u64;

        // 写入文件
        file.write_all(&chunk).await?;

        // 每秒记录一次进度（避免日志过多）
        if last_log_time.elapsed().as_secs() >= 1 {
            if let Some(total) = total_size {
                let percent = (downloaded as f64 / total as f64) * 100.0;
                tracing::debug!("下载进度: {:.1}% ({}/{})", percent, downloaded, total);
            } else {
                tracing::debug!("已下载: {} bytes", downloaded);
            }
            last_log_time = std::time::Instant::now();
        }
    }

    // 确保所有数据都写入磁盘
    file.flush().await?;

    tracing::info!("下载完成: {:?}", destination);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_download_small_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let destination = temp_file.path();

        // 使用一个小文件进行测试
        let result = download_package("https://httpbin.org/bytes/1024", destination).await;

        assert!(result.is_ok());
        assert!(destination.exists());
        let metadata = std::fs::metadata(destination).unwrap();
        assert!(metadata.len() > 0);
    }
}
