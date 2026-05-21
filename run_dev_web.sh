#!/bin/bash
# MIT License
# Copyright (c) 2025 fi-code contributors
#
# 开发模式启动 Web 端（后端 Server + 前端 Vite）

set -e

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)

echo "🚀 启动 Web 开发模式..."
echo "📦 项目根目录: $PROJECT_ROOT"

# 清理可能遗留的进程
pkill -f "fi-code-server" 2>/dev/null || true
pkill -f "npm run dev" 2>/dev/null || true
sleep 1

echo ""
echo "1️⃣  编译并启动后端 Server..."
cd "$PROJECT_ROOT"
cargo run --bin fi-code-server &
SERVER_PID=$!

# 等待 server 启动
for i in {1..10}; do
  if ss -tuln 2>/dev/null | grep -q ":4040"; then
    echo "✅ 后端 Server 已启动"
    break
  fi
  echo "⏳ 等待后端启动... ($i/10)"
  sleep 1
done

echo ""
echo "2️⃣  启动前端 Vite 开发服务器..."
cd "$PROJECT_ROOT/frontend"
npm run dev &
VITE_PID=$!

echo ""
echo "✅ 服务已启动！"
echo "   后端 Server: http://0.0.0.0:4040"
echo "   前端 Vite:   http://localhost:1420/"
echo ""
echo "   按 Ctrl+C 停止所有服务"

# 捕获退出信号，清理进程
trap "echo ''; echo '🛑 正在停止服务...'; kill $SERVER_PID $VITE_PID 2>/dev/null || true; exit 0" INT TERM

# 等待任一进程退出
wait
