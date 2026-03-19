# AI Agent 自动化测试 Electron 应用 - 设计文档

**日期**: 2025-03-19
**版本**: v0.1.0
**作者**: Claude

---

## 1. 项目概述

### 1.1 目标

构建一个基于 AI Agent 的自动化测试系统，用于测试包含编辑器和硬件连接功能的 Electron 桌面应用。

### 1.2 核心能力

| 能力 | 描述 |
|------|------|
| **CI 集成** | 通过 POST API 接收安装包 URL，自动下载、解压、安装、测试 |
| **自然语言驱动** | 支持自由文本描述测试目标，LLM 解析并执行 |
| **UI 视觉测试** | AI 语义对比验证 UI 元素状态，非像素级对比 |
| **智能跳过** | 自动识别硬件相关功能，标记跳过不阻塞测试 |
| **多模型支持** | 可配置 OpenAI/Claude API |
| **报告推送** | 飞书推送摘要 + 本地 HTML/JSON 报告 |

### 1.3 部署环境

- **固定部署**: Windows / macOS 物理机
- **需要**: 真实桌面环境运行 Electron

---

## 2. 系统架构

### 2.1 整体架构图

```
┌───────────────────────────────────────────────────────────────────┐
│                      测试服务器 (Rust)                             │
├───────────────────────────────────────────────────────────────────┤
│                                                                    │
│   ┌─────────────────────────────────────────────────────────┐     │
│   │                    HTTP API 层 (Axum)                   │     │
│   │              POST /api/test { url: "..." }              │     │
│   └───────────────────────────┬─────────────────────────────┘     │
│                               │                                   │
│   ┌───────────────────────────▼─────────────────────────────┐     │
│   │                    核心编排层                             │     │
│   │              ┌─────────────────────────────┐            │     │
│   │              │      TaskOrchestrator       │            │     │
│   │              │  - 接收测试请求              │            │     │
│   │              │  - 协调各模块执行            │            │     │
│   │              │  - 管理测试状态              │            │     │
│   │              └──────────┬──────────┬───────┘            │     │
│   └─────────────────────────┼──────────┼───────────────────┘     │
│                             │          │                         │
│   ┌────────────┐  ┌─────────▼──┐  ┌──▼──────────┐              │
│   │ 安装管理器  │  │  测试引擎  │  │  报告生成器  │              │
│   │            │  │            │  │             │              │
│   │- 下载zip   │  │- 启动Electr│  │- 汇总结果   │              │
│   │- 解压      │  │- 连接CDP   │  │- 生成HTML   │              │
│   │- 安装/启动 │  │- Agent执行 │  │- 生成JSON   │              │
│   │            │  │- 视觉验证  │  │             │              │
│   └────────────┘  └─────┬──────┘  └──────┬──────┘              │
│                          │                 │                    │
│   ┌────────────┐        │          ┌──────▼──────┐            │
│   │ 模型配置器  │        │          │  飞书推送器  │            │
│   │            │        │          │             │            │
│   │- OpenAI    │        │          │- Webhook    │            │
│   │- Claude    │        │          │- 摘要推送   │            │
│   │- 切换      │        │          │             │            │
│   └────────────┘        │          └─────────────┘            │
│                          │                                       │
│                   ┌──────▼───────────┐                          │
│                   │  agent-browser   │                          │
│                   │  (Rust CLI)      │                          │
│                   └──────┬───────────┘                          │
│                          │ CDP                                  │
│                   ┌──────▼───────────┐                          │
│                   │   Electron App   │                          │
│                   │  (--remote-debug)│                          │
│                   └──────────────────┘                          │
└───────────────────────────────────────────────────────────────────┘
```

### 2.2 技术栈

| 模块 | 技术选择 | 理由 |
|------|----------|------|
| HTTP API | **Axum** | Tokio生态、类型安全、性能高 |
| 异步运行时 | **Tokio** | 成熟稳定、生态丰富 |
| agent-browser | **直接调用/嵌入** | 同语言，可直接集成 |
| LLM调用 | **reqwest + serde** | HTTP客户端，灵活调用各API |
| 报告生成 | **Tera / Askama** | 模板引擎，生成HTML |
| 配置管理 | **serde + config** | TOML/YAML反序列化 |
| 日志 | **tracing** | 结构化日志 |

---

## 3. 数据模型

### 3.1 核心数据结构

```rust
// 测试请求
#[derive(Debug, Deserialize)]
pub struct TestRequest {
    pub package_url: String,      // zip安装包URL
    pub test_goals: Vec<String>,  // 测试目标（自然语言）
    pub options: Option<TestOptions>,
}

// 测试任务
#[derive(Debug, Clone)]
pub struct TestTask {
    pub id: String,               // 任务ID
    pub package_url: String,
    pub test_goals: Vec<String>,
    pub status: TaskStatus,
    pub workspace: PathBuf,
    pub electron_path: PathBuf,
    pub cdp_port: u16,
}

#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    Downloading,
    Extracting,
    Installing,
    Running,
    Completed,
    Failed(String),
}

// 测试步骤
#[derive(Debug, Serialize, Deserialize)]
pub struct TestStep {
    pub id: String,
    pub description: String,
    pub action: TestAction,
    pub status: StepStatus,
    pub screenshot: Option<String>,
    pub error: Option<String>,
    pub is_hardware_related: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TestAction {
    Click { ref_id: String },
    Input { ref_id: String, text: String },
    Wait { duration_ms: u32 },
    Navigate { url: String },
    Assert { condition: String },
    Skip { reason: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
}

// 测试报告
#[derive(Debug, Serialize)]
pub struct TestReport {
    pub task_id: String,
    pub package_url: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration_secs: u64,
    pub steps: Vec<TestStep>,
    pub summary: ReportSummary,
}

#[derive(Debug, Serialize)]
pub struct ReportSummary {
    pub total: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub pass_rate: f32,
}
```

---

## 4. 测试执行流程

```
POST /api/test
     │
     ▼
┌─────────────┐
│ 创建任务    │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ 下载zip包   │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ 解压安装包  │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ 启动Electron│ → --remote-debugging-port=9222
└──────┬──────┘
       │
       ▼
┌─────────────┐
│ 连接CDP     │ → agent-browser connect
└──────┬──────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│                    测试执行循环                               │
│  1. 获取页面快照 (agent-browser snapshot -i)                │
│  2. LLM分析: 理解目标 → 识别硬件功能 → 决定动作              │
│  3. 执行动作 (agent-browser click @e1)                      │
│  4. 视觉验证 (AI语义对比)                                   │
│  5. 记录步骤 + 截图                                         │
│  6. 重复直到目标完成或失败                                  │
└───────────────────────────┬─────────────────────────────────┘
                          │
                          ▼
                   ┌─────────────┐
                   │ 关闭应用    │
                   └──────┬──────┘
                          │
                          ▼
                   ┌─────────────┐
                   │ 生成报告    │ → HTML + JSON
                   └──────┬──────┘
                          │
                          ▼
                   ┌─────────────┐
                   │ 飞书推送    │
                   └─────────────┘
```

---

## 5. API 接口

| 方法 | 路径 | 描述 |
|------|------|------|
| POST | /api/test | 提交测试任务 |
| GET | /api/test/{id} | 查询任务状态 |
| GET | /api/test/{id}/report | 获取 JSON 报告 |
| GET | /api/test/{id}/report/html | 获取 HTML 报告 |
| DELETE | /api/test/{id} | 取消任务 |
| GET | /health | 健康检查 |

### 5.1 提交测试任务

```json
POST /api/test
{
    "package_url": "https://ci.example.com/builds/app-v1.2.3-win.zip",
    "test_goals": [
        "测试登录流程：点击登录按钮，输入用户名密码，验证登录成功",
        "测试编辑器新建文件功能"
    ],
    "options": {
        "model": "claude",
        "timeout": 300,
        "retries": 2
    }
}
```

---

## 6. 配置管理

```toml
[server]
host = "0.0.0.0"
port = 8080
workers = 4

[task]
workspace_dir = "./workspace"
cdp_port = 9222
default_timeout = 300
default_retries = 2
max_concurrent_tasks = 3

[llm]
default_model = "claude"
timeout = 30
max_retries = 3

[llm.openai]
api_base = "https://api.openai.com/v1"
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o"

[llm.claude]
api_base = "https://api.anthropic.com/v1"
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-3-5-sonnet-20241022"

[agent_browser]
bin_path = "agent-browser"
snapshot_interval = 1000

[report]
output_dir = "./reports"
html_template = "./templates/report.html"
screenshot_format = "png"
keep_screenshots = true

[feishu]
webhook_url = "${FEISHU_WEBHOOK}"
enabled = true

[logging]
level = "info"
dir = "./logs"
max_files = 7
```

---

## 7. 项目结构

```
agent-automation/
├── Cargo.toml
├── config/
│   └── settings.toml
├── docs/
│   └── plans/
│       └── 2025-03-19-electron-test-agent-design.md
├── src/
│   ├── main.rs
│   ├── api/
│   │   ├── mod.rs
│   │   └── routes.rs
│   ├── orchestrator/
│   │   ├── mod.rs
│   │   └── task.rs
│   ├── installer/
│   │   ├── mod.rs
│   │   ├── download.rs
│   │   ├── extract.rs
│   │   └── launch.rs
│   ├── engine/
│   │   ├── mod.rs
│   │   ├── agent.rs
│   │   ├── browser.rs
│   │   └── vision.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── openai.rs
│   │   └── claude.rs
│   ├── reporter/
│   │   ├── mod.rs
│   │   ├── html.rs
│   │   └── feishu.rs
│   ├── error.rs
│   └── config.rs
├── templates/
│   └── report.html
├── workspace/        # 工作目录（动态）
├── reports/          # 报告输出
└── logs/             # 日志文件
```

---

## 8. 飞书推送格式

飞书推送包含：
- 测试摘要（通过/失败/跳过数量）
- 通过率
- 耗时
- 失败截图缩略图
- 详细报告链接

---

## 9. 开发优先级

**并行开发策略**：

| 模块 | 优先级 | 说明 |
|------|--------|------|
| 项目搭建 | P0 | Cargo项目、基础结构 |
| HTTP API | P0 | 提交任务、状态查询 |
| 安装管理器 | P0 | 下载/解压/启动核心流程 |
| 测试引擎 - Agent | P1 | LLM解析、动作执行 |
| 测试引擎 - Browser | P1 | agent-browser封装 |
| 视觉验证 | P1 | AI语义对比 |
| 报告生成 | P1 | HTML/JSON生成 |
| 飞书推送 | P2 | 通知功能 |
