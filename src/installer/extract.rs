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
pub async fn extract_zip(
    zip_path: &Path,
    destination: &Path,
) -> Result<(), AppError> {
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

    // 读取 ZIP 文件
    let file_bytes = fs::read(zip_path).await?;
    tracing::debug!("读取 ZIP 文件: {} bytes", file_bytes.len());

    // 创建 ZIP 归档
    let mut archive = ZipArchive::new(std::io::Cursor::new(file_bytes))?;

    let file_count = archive.len();
    tracing::info!("ZIP 文件包含 {} 个文件", file_count);

    // 解压所有文件
    for i in 0..file_count {
        let mut file = archive.by_index(i)?;
        let file_path = destination.join(file.name());

        // 跳过目录（已在创建目录时处理）
        if file.name().ends_with('/') {
            fs::create_dir_all(&file_path).await?;
            continue;
        }

        // 创建父目录
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 使用缓冲区复制文件内容
        let mut buffer = Vec::with_capacity(file.size() as usize);
        std::io::copy(&mut file, &mut buffer)?;

        // 直接写入文件（比先创建再写入更高效）
        fs::write(&file_path, buffer).await?;

        tracing::debug!("解压文件: {} ({} bytes)", file.name(), file.size());
    }

    tracing::info!("解压完成: {:?} (共 {} 个文件)", destination, file_count);
    Ok(())
}

/// 解压 ZIP 文件到目标目录（同步版本，用于某些特殊场景）
///
/// # 参数
/// * `zip_path` - ZIP 文件的路径
/// * `destination` - 解压的目标目录
///
/// # 返回
/// 成功时返回 `Ok(())`，失败时返回相应的错误
pub fn extract_zip_sync(
    zip_path: &Path,
    destination: &Path,
) -> Result<(), AppError> {
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use zip::write::FileOptions;
    use std::io::Write;

    fn create_test_zip(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let file = std::fs::File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

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
        let content = fs::read_to_string(extract_dir.join("test.txt")).await.unwrap();
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
