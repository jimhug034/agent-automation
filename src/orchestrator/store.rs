//! 任务存储模块
//!
//! 提供任务的内存存储和管理功能

use crate::models::{TestTask, TaskStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 任务存储类型
///
/// 使用 Arc<RwLock<HashMap>> 实现线程安全的内存存储
pub type TaskStore = Arc<RwLock<HashMap<String, TestTask>>>;

/// 任务管理器
///
/// 负责任务的创建、查询、更新和统计
pub struct TaskManager {
    /// 内部任务存储
    tasks: TaskStore,
}

impl TaskManager {
    /// 创建新的任务管理器
    ///
    /// # Returns
    ///
    /// 返回一个新的 TaskManager 实例
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取任务存储的引用
    ///
    /// # Returns
    ///
    /// 返回 TaskStore 的引用，用于共享给其他组件
    pub fn store(&self) -> TaskStore {
        Arc::clone(&self.tasks)
    }

    /// 创建新任务
    ///
    /// # Arguments
    ///
    /// * `task` - 要创建的任务
    ///
    /// # Returns
    ///
    /// 成功时返回任务 ID，失败时返回错误
    pub async fn create(&self, task: TestTask) -> Result<String, crate::error::AppError> {
        let task_id = task.id.clone();
        tracing::info!("创建新任务: {}", task_id);

        let mut tasks = self.tasks.write().await;
        tasks.insert(task_id.clone(), task);

        tracing::debug!("任务已创建，当前任务总数: {}", tasks.len());
        Ok(task_id)
    }

    /// 获取指定 ID 的任务
    ///
    /// # Arguments
    ///
    /// * `task_id` - 任务 ID
    ///
    /// # Returns
    ///
    /// 成功时返回任务的克隆，失败时返回 NotFound 错误
    pub async fn get(&self, task_id: &str) -> Result<TestTask, crate::error::AppError> {
        let tasks = self.tasks.read().await;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| crate::error::AppError::NotFound(task_id.to_string()))
    }

    /// 检查任务是否存在
    ///
    /// # Arguments
    ///
    /// * `task_id` - 任务 ID
    ///
    /// # Returns
    ///
    /// 如果任务存在返回 true，否则返回 false
    pub async fn exists(&self, task_id: &str) -> bool {
        let tasks = self.tasks.read().await;
        tasks.contains_key(task_id)
    }

    /// 更新任务状态
    ///
    /// # Arguments
    ///
    /// * `task_id` - 任务 ID
    /// * `status` - 新的状态
    ///
    /// # Returns
    ///
    /// 成功时返回 Ok(())，失败时返回 NotFound 错误
    pub async fn update_status(
        &self,
        task_id: &str,
        status: TaskStatus,
    ) -> Result<(), crate::error::AppError> {
        let mut tasks = self.tasks.write().await;

        if let Some(task) = tasks.get_mut(task_id) {
            tracing::debug!("更新任务状态: {} -> {:?}", task_id, status);
            task.status = status;
            Ok(())
        } else {
            Err(crate::error::AppError::NotFound(task_id.to_string()))
        }
    }

    /// 更新任务状态（带错误信息）
    ///
    /// # Arguments
    ///
    /// * `task_id` - 任务 ID
    /// * `error_msg` - 错误信息
    ///
    /// # Returns
    ///
    /// 成功时返回 Ok(())，失败时返回 NotFound 错误
    pub async fn fail_task(
        &self,
        task_id: &str,
        error_msg: String,
    ) -> Result<(), crate::error::AppError> {
        self.update_status(task_id, TaskStatus::Failed(error_msg))
            .await
    }

    /// 列出所有任务
    ///
    /// # Returns
    ///
    /// 返回所有任务的列表
    pub async fn list(&self) -> Vec<TestTask> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    /// 列出指定状态的任务
    ///
    /// # Arguments
    ///
    /// * `status` - 要筛选的状态
    ///
    /// # Returns
    ///
    /// 返回匹配状态的任务列表
    pub async fn list_by_status(&self, status: &TaskStatus) -> Vec<TestTask> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|task| &task.status == status)
            .cloned()
            .collect()
    }

    /// 统计各状态的任务数量
    ///
    /// # Returns
    ///
    /// 返回一个 HashMap，键为状态，值为该状态的任务数量
    pub async fn count_by_status(&self) -> HashMap<TaskStatus, usize> {
        let tasks = self.tasks.read().await;
        let mut counts = HashMap::new();

        for task in tasks.values() {
            // 对于 Failed 状态，我们统一计数
            let status_key = match &task.status {
                TaskStatus::Failed(_) => TaskStatus::Failed(String::new()),
                other => other.clone(),
            };
            *counts.entry(status_key).or_insert(0) += 1;
        }

        counts
    }

    /// 获取任务总数
    ///
    /// # Returns
    ///
    /// 返回当前存储的任务总数
    pub async fn count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.len()
    }

    /// 删除指定任务
    ///
    /// # Arguments
    ///
    /// * `task_id` - 任务 ID
    ///
    /// # Returns
    ///
    /// 成功时返回被删除的任务，失败时返回 NotFound 错误
    pub async fn delete(&self, task_id: &str) -> Result<TestTask, crate::error::AppError> {
        let mut tasks = self.tasks.write().await;

        tasks
            .remove(task_id)
            .ok_or_else(|| crate::error::AppError::NotFound(task_id.to_string()))
    }

    /// 清空所有任务
    ///
    /// # Returns
    ///
    /// 返回被清空的任务数量
    pub async fn clear(&self) -> usize {
        let mut tasks = self.tasks.write().await;
        let count = tasks.len();
        tasks.clear();
        tracing::info!("已清空所有任务，共 {} 个", count);
        count
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TestTask;
    use std::path::PathBuf;

    fn create_test_task() -> TestTask {
        TestTask::new(
            "https://example.com/app.zip".to_string(),
            vec!["Test login".to_string()],
            PathBuf::from("/tmp/workspace"),
            PathBuf::from("/tmp/app/electron"),
            9222,
        )
    }

    #[tokio::test]
    async fn test_create_task() {
        let manager = TaskManager::new();
        let task = create_test_task();
        let task_id = task.id.clone();

        let result = manager.create(task).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), task_id);
    }

    #[tokio::test]
    async fn test_get_task() {
        let manager = TaskManager::new();
        let task = create_test_task();
        let task_id = task.id.clone();

        manager.create(task).await;

        let result = manager.get(&task_id).await;
        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.id, task_id);
    }

    #[tokio::test]
    async fn test_get_nonexistent_task() {
        let manager = TaskManager::new();
        let result = manager.get("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_status() {
        let manager = TaskManager::new();
        let task = create_test_task();
        let task_id = task.id.clone();

        manager.create(task).await;

        let result = manager
            .update_status(&task_id, TaskStatus::Downloading)
            .await;
        assert!(result.is_ok());

        let updated = manager.get(&task_id).await.unwrap();
        assert_eq!(updated.status, TaskStatus::Downloading);
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let manager = TaskManager::new();

        let task1 = create_test_task();
        let task2 = create_test_task();

        manager.create(task1).await;
        manager.create(task2).await;

        let tasks = manager.list().await;
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn test_count_by_status() {
        let manager = TaskManager::new();

        let mut task1 = create_test_task();
        task1.status = TaskStatus::Pending;
        let task1_id = task1.id.clone();

        let mut task2 = create_test_task();
        task2.status = TaskStatus::Completed;

        manager.create(task1).await;
        manager.create(task2).await;

        let counts = manager.count_by_status().await;
        assert_eq!(*counts.get(&TaskStatus::Pending).unwrap_or(&0), 1);
        assert_eq!(*counts.get(&TaskStatus::Completed).unwrap_or(&0), 1);
    }

    #[tokio::test]
    async fn test_delete_task() {
        let manager = TaskManager::new();
        let task = create_test_task();
        let task_id = task.id.clone();

        manager.create(task).await;

        let result = manager.delete(&task_id).await;
        assert!(result.is_ok());

        assert!(!manager.exists(&task_id).await);
    }

    #[tokio::test]
    async fn test_clear_tasks() {
        let manager = TaskManager::new();

        manager.create(create_test_task()).await;
        manager.create(create_test_task()).await;

        let count = manager.clear().await;
        assert_eq!(count, 2);
        assert_eq!(manager.count().await, 0);
    }

    #[tokio::test]
    async fn test_fail_task() {
        let manager = TaskManager::new();
        let task = create_test_task();
        let task_id = task.id.clone();

        manager.create(task).await;

        let result = manager.fail_task(&task_id, "Test error".to_string()).await;
        assert!(result.is_ok());

        let failed = manager.get(&task_id).await.unwrap();
        assert!(matches!(failed.status, TaskStatus::Failed(_)));
    }
}
