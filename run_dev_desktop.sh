#!/bin/bash
# MIT License
# Copyright (c) 2025 fi-code contributors
#
# 开发模式启动 Desktop 端（Tauri + 前端）

set -e

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

echo "🚀 启动 Desktop 开发模式..."
echo "📦 项目根目录: $PROJECT_ROOT"

# 清理可能遗留的进程
pkill -f "fi-code-server" 2>/dev/null || true
sleep 0.5

echo ""
echo "1️⃣  编译 fi-code 二进制（用于 Tauri Sidecar）..."
cd "$PROJECT_ROOT"
cargo build --bin fi-code-cli

echo ""
echo "2️⃣  启动 Tauri 开发模式..."
cd "$PROJECT_ROOT/frontend"
npm run tauri-dev
