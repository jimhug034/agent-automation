# Agent Automation - Just 命令集

default:
    @just --list

# 安装 agent-browser
install-agent-browser:
    #!/bin/bash
    if command -v agent-browser &> /dev/null; then
        echo "agent-browser already installed: $(agent-browser --version 2>&1 || echo 'unknown')"
    else
        echo "Installing agent-browser..."
        npm install -g agent-browser
    fi

# 安装所有依赖
install: install-agent-browser
    @echo "All dependencies installed"

# 开发模式运行
run:
    cargo run

# 构建 release
build:
    cargo build --release

# 测试
test:
    cargo test

# 检查代码
check:
    cargo check

# 格式化
fmt:
    cargo fmt

# Lint
clippy:
    cargo clippy -- -D warnings
# 自动修复 clippy 问题
clippy-fix:
    cargo clippy --fix --allow-dirty -- -D warnings

# 格式化检查
fmt-check:
    cargo fmt -- --check

# 完整检查（pre-commit hook）
check-all: fmt-check clippy

# 清理构建产物
clean:
    cargo clean

# 创建配置文件（如果不存在）
config:
    #!/bin/bash
    if [ ! -f "config/settings.toml" ]; then
        echo "Creating config/settings.toml from template..."
        cp config/settings.toml.example config/settings.toml 2>/dev/null || \
        echo "# Please create config/settings.toml manually"
    else
        echo "config/settings.toml already exists"
    fi
    if [ ! -f ".env.local" ]; then
        echo "Creating .env.local from .env.example..."
        cp .env.example .env.local
        echo "# Please edit .env.local with your API keys"
    else
        echo ".env.local already exists"
    fi

# 显示服务状态
status:
    @curl -s http://localhost:8080/health || echo "Service not running"
