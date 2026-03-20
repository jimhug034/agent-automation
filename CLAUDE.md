# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

这是一个基于 AI Agent 的 Electron 应用自动化测试系统。核心功能是：
1. 通过 POST API 接收测试任务（包含应用包 URL 和自然语言测试目标）
2. 下载、解压、启动 Electron 应用（通过 Chrome DevTools Protocol 控制）
3. 使用 LLM 解析自然语言测试目标，生成并执行测试步骤
4. 生成测试报告（HTML/JSON）并可选推送到飞书

## 常用命令

```bash
# 使用 just（推荐）
just install      # 安装所有依赖（包括 agent-browser）
just run          # 开发模式运行
just test         # 运行测试
just check-all    # 完整检查（fmt + clippy）
just build        # 构建 release
just clean        # 清理构建产物
just status       # 检查服务状态

# 或直接使用 cargo
cargo run
cargo test
cargo build --release
cargo fmt
cargo clippy
```

## Lefthook（Git Hooks）

```bash
# pre-commit: cargo fmt --check + cargo clippy
# pre-push: cargo test + cargo build --release
# commit-msg: Conventional Commits 格式验证
```

## 配置

配置文件位于 `config/settings.toml`，支持环境变量覆盖：

```bash
export OPENAI_API_KEY="sk-..."        # OpenAI API
export ANTHROPIC_API_KEY="sk-ant-..."  # Claude API
export FEISHU_WEBHOOK="https://..."   # 飞书通知（可选）
```

## 架构概览

```
src/
├── api/              # Axum HTTP API 层
├── engine/           # 测试引擎核心
│   ├── agent.rs      # TestAgent：结合 LLM + 浏览器控制
│   └── browser.rs    # AgentBrowser CLI 封装
├── orchestrator/     # 任务编排
│   ├── executor.rs   # TaskExecutor：完整任务执行流程
│   └── store.rs      # TaskStore：Arc<RwLock<HashMap>> 任务存储
├── llm/              # LLM 客户端抽象
│   ├── client.rs     # LlmClient trait
│   ├── openai.rs
│   └── claude.rs
├── installer/        # Electron 应用安装管理
│   ├── download.rs   # ZIP 下载
│   ├── extract.rs    # ZIP 解压
│   └── launch.rs     # Electron 进程启动/终止
├── reporter/         # 报告生成
│   ├── html.rs       # Tera 模板渲染
│   └── feishu.rs     # 飞书 Webhook 推送
├── models.rs         # 核心数据模型（TestTask, TestStep, TestAction, TestReport）
├── error.rs          # AppError 枚举 + Axum IntoResponse
└── config.rs         # Settings 配置加载
```

## 关键设计模式

### 错误处理
`AppError` 枚举定义所有错误类型，实现了 `IntoResponse` trait 用于 Axum HTTP 响应。

### LLM 抽象
`LlmClient` trait 定义了统一的聊天接口，支持 OpenAI 和 Claude。TestAgent 使用此 trait 解析自然语言测试目标为 `TestAction` 序列。

### 任务执行流程
1. API 接收请求 → 创建 `TestTask` → 存入 `TaskStore`
2. `TaskExecutor::execute()` 执行：下载 → 解压 → 启动 Electron → 执行测试 → 生成报告
3. 测试执行（当前为占位实现，待完成）：
   - 连接 CDP（通过 agent-browser CLI）
   - 对每个 test_goal 调用 `TestAgent::execute_goal()`
   - LLM 生成 TestAction 序列（Click/Input/Wait/Assert/Skip）
   - 执行并收集结果

### agent-browser 集成
外部 CLI 工具（需单独 `npm install -g agent-browser`），用于通过 CDP 控制 Electron 应用。`AgentBrowser` 封装了 CLI 调用。

## 待实现的关键功能

`orchestrator/executor.rs:134` 标记了 TODO：实际的测试执行逻辑。需要：
1. 连接到 agent-browser（已启动的 Electron CDP 端口）
2. 为每个 test_goal 创建 TestAgent 并调用 `execute_goal()`
3. 收集截图、测试结果
4. 处理硬件相关功能的智能跳过（LLM 判断）

## API 端点

```
GET  /health                    # 健康检查
POST /api/test                  # 提交测试任务
GET  /api/test/:id              # 查询任务状态
GET  /api/test/:id/report       # JSON 报告
GET  /api/test/:id/report/html  # HTML 报告
DELETE /api/test/:id            # 删除任务
```

## Git Hooks

使用 lefthook 管理：
- `pre-commit`: `cargo fmt --check` + `cargo clippy`
- `pre-push`: `cargo test` + `cargo build --release`
- `commit-msg`: Conventional Commits 格式验证
