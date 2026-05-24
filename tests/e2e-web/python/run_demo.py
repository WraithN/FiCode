#!/usr/bin/env python3
"""
fi-code E2E 测试快速演示脚本
展示 Mock AI 开关的用法
"""
import os
import sys
from pathlib import Path

# 添加当前目录到路径
sys.path.insert(0, str(Path(__file__).parent))

import constants


def print_banner(title: str):
    """打印标题横幅"""
    print(f"\n{'='*60}")
    print(f"  {title}")
    print(f"{'='*60}\n")


def show_config():
    """显示当前配置"""
    print_banner("当前配置")
    
    print(f"📡 USE_MOCK_AI = {constants.USE_MOCK_AI}")
    print(f"    - {constants.COLOR_GREEN}使用 Mock AI{constants.COLOR_RESET}" if constants.USE_MOCK_AI 
          else f"    - {constants.COLOR_YELLOW}连接真实 API{constants.COLOR_RESET}")
    
    print(f"\n🔌 服务器配置:")
    print(f"   - SERVER_PORT = {constants.SERVER_PORT}")
    print(f"   - FRONTEND_PORT = {constants.FRONTEND_PORT}")
    print(f"   - MOCK_SERVER_PORT = {constants.MOCK_SERVER_PORT}")
    
    print(f"\n📁 测试项目:")
    print(f"   - {constants.TEST_PROJECT_DIR}")
    
    print()


def show_usage():
    """显示使用说明"""
    print_banner("使用说明")
    
    print("""
运行模式:

1️⃣  Mock 模式（默认）
    └─ 快速测试，无需 API Key
    
    命令:
        python run_demo.py mock
        # 或
        USE_MOCK_AI=true pytest -v

2️⃣  真实模式
    └─ 连接真实 AI API
    
    命令:
        python run_demo.py real
        # 或
        USE_MOCK_AI=false pytest -v

3️⃣  运行特定测试
    └─ 仅运行演示测试
    
    命令:
        pytest -v test_web_01_simple_demo.py
        pytest -v test_web_02_tool_tests.py
""")


def run_tests(mode: str):
    """运行测试"""
    print_banner(f"运行测试 - {mode.upper()} 模式")
    
    # 设置环境变量
    if mode == "mock":
        os.environ["USE_MOCK_AI"] = "true"
        print(f"{constants.COLOR_GREEN}✅ 已启用 Mock AI 模式{constants.COLOR_RESET}")
    else:
        os.environ["USE_MOCK_AI"] = "false"
        print(f"{constants.COLOR_YELLOW}⚠️  已启用真实 API 模式{constants.COLOR_RESET}")
    
    print(f"\n正在运行演示测试...")
    print(f"{constants.COLOR_YELLOW}注意: 这会尝试运行完整的 pytest 测试{constants.COLOR_RESET}")
    print(f"{constants.COLOR_YELLOW}如果没有安装依赖，先运行: pip install -r requirements.txt{constants.COLOR_RESET}\n")
    
    # 尝试运行
    try:
        args = ["pytest", "-v", "test_web_01_simple_demo.py"]
        os.execvp("pytest", args)
    except Exception as e:
        print(f"{constants.COLOR_RED}错误: {e}{constants.COLOR_RESET}")
        print(f"\n提示: pytest 可能没有安装")
        print(f"请运行: pip install -r requirements.txt")


def main():
    """主函数"""
    print(f"\n{constants.COLOR_GREEN}")
    print("╔═══════════════════════════════════════════════════════════════╗")
    print("║       fi-code Web E2E 测试 - 快速演示                      ║")
    print("╚═══════════════════════════════════════════════════════════════╝")
    print(f"{constants.COLOR_RESET}")
    
    # 显示配置
    show_config()
    
    # 解析参数
    if len(sys.argv) > 1:
        mode = sys.argv[1].lower()
        if mode in ["mock", "real"]:
            run_tests(mode)
            return
    
    # 默认显示帮助
    show_usage()


if __name__ == "__main__":
    main()
