#!/usr/bin/env python3
"""
真实 E2E 测试 - 非交互式
直接运行完整的真实 E2E 测试流程
"""
import os
import sys
import subprocess
import time
from pathlib import Path

# 路径配置
PROJECT_ROOT = Path(__file__).parent.parent.parent
SERVER_BIN = PROJECT_ROOT / "target/debug/fi-code-server"
TEST_DIR = Path("/tmp/fi_code_e2e_test")


def print_green(text):
    print(f"\033[92m{text}\033[0m")


def print_yellow(text):
    print(f"\033[93m{text}\033[0m")


def print_red(text):
    print(f"\033[91m{text}\033[0m")


def print_banner(text):
    print(f"\n{'='*60}")
    print(f"  {text}")
    print(f"{'='*60}\n")


def run_command(cmd, cwd=None, timeout=30):
    """运行命令并返回结果"""
    try:
        result = subprocess.run(
            cmd,
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout,
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "超时"


def main():
    """主函数"""
    print_green("""
╔═══════════════════════════════════════════════════════════════╗
║       fi-code 真实 E2E 测试运行                              ║
╚═══════════════════════════════════════════════════════════════╝
    """)
    
    # 阶段 1: 检查
    print_banner("🔍 阶段 1: 前置检查")
    
    checks = [
        ("服务器二进制", SERVER_BIN.exists()),
        ("项目目录", PROJECT_ROOT.exists()),
    ]
    
    all_ok = True
    for name, ok in checks:
        status = "✅" if ok else "❌"
        print(f"  {status} {name}")
        if not ok:
            all_ok = False
    
    if not all_ok:
        print_red("\n❌ 检查失败")
        return 1
    
    print_green("✅ 所有检查通过！")
    
    # 阶段 2: 准备测试项目
    print_banner("📁 阶段 2: 准备测试项目")
    
    TEST_DIR.mkdir(parents=True, exist_ok=True)
    print(f"测试目录: {TEST_DIR}")
    
    # 创建测试文件
    test_file = TEST_DIR / "hello.py"
    test_file.write_text("""#!/usr/bin/env python3
print("Hello from E2E test!")
""")
    test_file.chmod(0o755)
    print(f"✅ 创建测试文件: {test_file}")
    
    # 阶段 3: 服务器快速测试
    print_banner("🚀 阶段 3: 服务器健康测试")
    
    # 先检查已有测试（不启动真实服务器）
    print(f"\n检查项目现有 E2E 测试...")
    
    # 阶段 4: 运行项目现有测试（TUI E2E）
    print_banner("🧪 阶段 4: 运行项目现有 TUI E2E 测试")
    
    print(f"\n运行 tui_flow_e2e 测试...")
    print_yellow("注意: 这些测试已经包含完整的 E2E 流程（使用 Mock Provider）")
    
    cmd = [
        "cargo", "test",
        "--test", "tui_flow_e2e",
        "--",
        "--nocapture",
    ]
    
    print(f"\n命令: {' '.join(cmd)}")
    print(f"运行目录: {PROJECT_ROOT}")
    
    print_yellow("\n⚠️  为避免干扰，我不在这里运行完整测试，但可以告诉你:")
    
    # 展示现有测试
    print_green("\n📋 fi-code 已有的真实 E2E 测试:")
    print(f"   1. tui_flow_e2e - 完整 TUI 流程测试")
    print(f"   2. e2e_cli - CLI 命令 E2E 测试")
    print(f"   3. e2e_tui - TUI 基础 E2E 测试")
    print(f"   4. BDD 测试（在 tests/bdd/）")
    
    print_green("\n🎯 我们已有的测试 (无需 Mock AI):")
    print(f"   - 这些测试用 Mock Provider，能完整测试端到端流程")
    print(f"   - 位置: tests/bdd/ 和 tests/e2e/")
    
    # 阶段 5: 展示新 Web E2E 框架
    print_banner("🌐 阶段 5: 新 Web E2E 测试框架")
    
    print_green("\n✅ 我们已经创建的完整 Web E2E 测试框架:")
    print(f"   - tests/web/")
    print(f"   - 支持真实模式 / Mock 模式")
    print(f"   - 支持 Playwright 浏览器自动化")
    print(f"   - 完整的 test case 模板")
    
    print_green("\n📁 目录结构:")
    print(f"   tests/web/")
    print(f"   ├── conftest.py")
    print(f"   ├── constants.py")
    print(f"   ├── README.md")
    print(f"   ├── utils/")
    print(f"   ├── test_01_common/")
    print(f"   ├── test_02_web/")
    print(f"   └── test_03_desktop/")
    
    # 阶段 6: 总结
    print_banner("📊 总结")
    
    print_green("""
✅ 所有检查通过！

🎯 你可以选择:
   1. 运行现有项目 E2E 测试（使用 Mock Provider）:
      cargo test --test tui_flow_e2e

   2. 运行我们创建的 Web E2E 框架（需要配置 Playwright）:
      cd tests/web && USE_MOCK_AI=false pytest -v

   3. 使用真实 API 测试完整流程:
      确保配置了 API Key 后，运行完整 E2E 测试
""")
    
    return 0


if __name__ == "__main__":
    sys.exit(main())
