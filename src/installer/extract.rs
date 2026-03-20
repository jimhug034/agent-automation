use crate::error::AppError;
use std::path::Path;
use tokio::fs;
use zip::ZipArchive;

/// 解压 ZIP 文件到目标目录
///
/// # 参数
/// * `zip_path` - ZIP 文件的路径
/// * `destination` - 解压的目标目录
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回相应的错误
pub async fn extract_zip(zip_path: &Path, destination: &Path) -> Result<(), AppError> {
    tracing::info!("开始解压: {:?} -> {:?}", zip_path, destination);

    // 检查 ZIP 文件是否存在
    if !zip_path.exists() {
        return Err(AppError::ExtractFailed(format!(
            "ZIP 文件不存在: {:?}",
            zip_path
        )));
    }

    // 创建目标目录
    fs::create_dir_all(destination).await?;
    tracing::debug!("创建目标目录: {:?}", destination);

    // 在 spawn_blocking 中执行解压，因为 ZipArchive 不是 Send
    let zip_path = zip_path.to_path_buf();
    let destination = destination.to_path_buf();
    tokio::task::spawn_blocking(move || {
        extract_zip_sync(&zip_path, &destination)
    }).await
    .map_err(|e| AppError::ExtractFailed(format!("spawn_blocking 错误: {}", e)))?
}

/// 解压 ZIP 文件到目标目录（同步版本，用于某些特殊场景）
///
/// # 参数
/// * `zip_path` - ZIP 文件的路径
/// * `destination` - 解压的目标目录
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回相应的错误
pub fn extract_zip_sync(zip_path: &Path, destination: &Path) -> Result<(), AppError> {
    tracing::info!("开始解压 (同步): {:?} -> {:?}", zip_path, destination);

    // 检查 ZIP 文件是否存在
    if !zip_path.exists() {
        return Err(AppError::ExtractFailed(format!(
            "ZIP 文件不存在: {:?}",
            zip_path
        )));
    }

    // 创建目标目录
    std::fs::create_dir_all(destination)?;

    // 打开 ZIP 文件
    let file = std::fs::File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let file_count = archive.len();
    tracing::info!("ZIP 文件包含 {} 个文件", file_count);

    // 解压所有文件
    for i in 0..file_count {
        let mut file = archive.by_index(i)?;
        let file_path = destination.join(file.name());

        // 跳过目录
        if file.name().ends_with('/') {
            std::fs::create_dir_all(&file_path)?;
            continue;
        }

        // 创建父目录
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 创建文件并写入内容
        let mut outfile = std::fs::File::create(&file_path)?;
        std::io::copy(&mut file, &mut outfile)?;

        tracing::debug!("解压文件: {} ({} bytes)", file.name(), file.size());
    }

    tracing::info!("解压完成: {:?} (共 {} 个文件)", destination, file_count);

    // 检查是否有嵌套的 .zip 文件（常见于更新包）
    // 只处理第一层嵌套，避免无限循环
    let nested_zips: Vec<_> = std::fs::read_dir(destination)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            // 只处理不以 .temp_ 开头的 zip 文件
            entry.path().extension().map(|e| e == "zip").unwrap_or(false) &&
            !entry.file_name().to_string_lossy().starts_with(".temp_")
        })
        .map(|entry| entry.path())
        .collect();

    if !nested_zips.is_empty() {
        tracing::info!("发现 {} 个嵌套 ZIP 文件，继续解压...", nested_zips.len());
        for nested_zip in &nested_zips {
            tracing::info!("解压嵌套 ZIP: {:?}", nested_zip);

            // 先将 zip 移动到临时位置，避免解压后检测到自己
            let temp_zip = destination.join(format!(".temp_{}.zip",
                nested_zip.file_stem().and_then(|s| s.to_str()).unwrap_or("nested")
            ));
            std::fs::rename(nested_zip, &temp_zip)?;

            // 解压到目标目录
            let result = extract_zip_sync(&temp_zip, destination);

            // 删除临时 zip
            let _ = std::fs::remove_file(&temp_zip);

            if let Err(e) = result {
                tracing::warn!("解压嵌套 ZIP 失败: {}", e);
            } else {
                tracing::debug!("已解压并删除嵌套 ZIP");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
    use zip::write::FileOptions;

    fn create_test_zip(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // 添加一些测试文件
        zip.start_file("test.txt", options)?;
        zip.write_all(b"Hello, World!")?;

        zip.start_file("subdir/nested.txt", options)?;
        zip.write_all(b"Nested content")?;

        zip.finish()?;
        Ok(())
    }

    #[tokio::test]
    async fn test_extract_zip() {
        let temp_dir = TempDir::new().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        let extract_dir = temp_dir.path().join("extract");

        // 创建测试 ZIP 文件
        create_test_zip(&zip_path).unwrap();

        // 解压
        let result = extract_zip(&zip_path, &extract_dir).await;
        assert!(result.is_ok());

        // 验证文件已解压
        assert!(extract_dir.join("test.txt").exists());
        assert!(extract_dir.join("subdir/nested.txt").exists());

        // 验证内容
        let content = fs::read_to_string(extract_dir.join("test.txt"))
            .await
            .unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_extract_zip_sync() {
        let temp_dir = TempDir::new().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        let extract_dir = temp_dir.path().join("extract");

        // 创建测试 ZIP 文件
        create_test_zip(&zip_path).unwrap();

        // 解压
        let result = extract_zip_sync(&zip_path, &extract_dir);
        assert!(result.is_ok());

        // 验证文件已解压
        assert!(extract_dir.join("test.txt").exists());
        assert!(extract_dir.join("subdir/nested.txt").exists());

        // 验证内容
        let content = std::fs::read_to_string(extract_dir.join("test.txt")).unwrap();
        assert_eq!(content, "Hello, World!");
    }
}
