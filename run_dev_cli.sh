#!/bin/bash
# MIT License
# Copyright (c) 2025 fi-code contributors
#
# 开发模式启动 CLI 端

set -e

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

echo "🚀 启动 CLI 开发模式..."
echo "📦 项目根目录: $PROJECT_ROOT"
echo ""
echo "提示: 你可以添加参数，例如:"
echo "  ./run_dev_cli.sh --help       查看帮助"
echo "  ./run_dev_cli.sh -i           进入交互模式"
echo "  ./run_dev_cli.sh server       启动 server 模式"
echo ""

cd "$PROJECT_ROOT"
cargo run --bin fi-code-cli -- "$@"
