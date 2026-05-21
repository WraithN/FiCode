#!/bin/bash
# MIT License
# Copyright (c) 2025 fi-code contributors
#
# 开发模式启动 TUI 端

set -e

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

echo "🚀 启动 TUI 开发模式..."
echo "📦 项目根目录: $PROJECT_ROOT"

cd "$PROJECT_ROOT"
cargo run --bin fi-code-tui
