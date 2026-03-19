# Agent Automation

基于 AI Agent 的 Electron 应用自动化测试系统。

## 功能特性

- 🚀 **CI 集成** - 通过 POST API 触发测试
- 🗣️ **自然语言驱动** - 用自然语言描述测试目标
- 👁️ **UI 视觉测试** - AI 语义对比验证
- 🤖 **智能跳过** - 自动识别硬件相关功能
- 🧩 **多模型支持** - OpenAI / Claude
- 📊 **报告推送** - 飞书 + HTML/JSON

## 快速开始

### 安装依赖

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 agent-browser
npm install -g agent-browser
# 或
brew install agent-browser
```

### 配置

```bash
# 复制配置文件
cp config/settings.toml.example config/settings.toml

# 设置环境变量
export OPENAI_API_KEY="sk-..."        # 或 ANTHROPIC_API_KEY
export FEISHU_WEBHOOK="https://..."   # 可选
```

### 运行

```bash
# 开发模式
cargo run

# 构建 release
cargo build --release
./target/release/agent-automation
```

## API 使用

### 提交测试任务

```bash
curl -X POST http://localhost:8080/api/test \
  -H "Content-Type: application/json" \
  -d '{
    "package_url": "https://example.com/app.zip",
    "test_goals": [
      "测试登录流程：点击登录按钮，输入用户名密码，验证登录成功",
      "测试编辑器新建文件功能"
    ],
    "options": {
      "model": "claude",
      "timeout": 300
    }
  }'
```

### 查询任务状态

```bash
curl http://localhost:8080/api/test/{task_id}
```

### 获取报告

```bash
# JSON 报告
curl http://localhost:8080/api/test/{task_id}/report

# HTML 报告
curl http://localhost:8080/api/test/{task_id}/report/html
```

## 项目结构

```
src/
├── api/           # HTTP API
├── config.rs      # 配置管理
├── domain.rs      # 数据模型 (TestTask, TestStep, etc.)
├── engine/        # 测试引擎 (Agent + Browser)
│   ├── agent.rs   # AI 测试代理
│   └── browser.rs # agent-browser 封装
├── error.rs       # 错误处理
├── installer/     # 安装管理器
│   ├── download.rs
│   ├── extract.rs
│   └── launch.rs
├── llm/           # LLM 客户端
│   ├── client.rs  # LLM trait
│   ├── openai.rs  # OpenAI API
│   └── claude.rs  # Claude API
├── logging.rs     # 日志系统
├── orchestrator/  # 任务编排
│   ├── executor.rs
│   └── store.rs
└── reporter/      # 报告生成
    ├── feishu.rs  # 飞书推送
    └── html.rs    # HTML 报告
```

## 测试

```bash
cargo test
```

## 开发

```bash
# 检查代码
cargo check

# 格式化
cargo fmt

# Lint
cargo clippy
```

## License

MIT
