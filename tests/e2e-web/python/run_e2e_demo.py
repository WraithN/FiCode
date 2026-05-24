#!/usr/bin/env python3
"""
真实 E2E 测试演示
直接与 fi-code 服务器交互的完整测试
"""
import os
import sys
import subprocess
import time
from pathlib import Path

# 添加常量模块
sys.path.insert(0, str(Path(__file__).parent))

import constants


def print_banner(text):
    """打印横幅"""
    print(f"\n{constants.COLOR_GREEN}{'='*60}{constants.COLOR_RESET}")
    print(f"  {text}")
    print(f"{constants.COLOR_GREEN}{'='*60}{constants.COLOR_RESET}\n")


def check_prerequisites():
    """检查前置条件"""
    print_banner("🔍 E2E 测试前置检查")
    
    checks = [
        ("服务器二进制", constants.SERVER_BIN.exists()),
        ("项目目录", constants.PROJECT_ROOT.exists()),
        ("测试目录", True),
    ]
    
    all_ok = True
    for name, ok in checks:
        status = "✅" if ok else "❌"
        print(f"  {status} {name}")
        if not ok:
            all_ok = False
    
    if not all_ok:
        print(f"\n{constants.COLOR_RED}❌ 部分检查失败，请先解决问题{constants.COLOR_RESET}")
        return False
    
    print(f"\n✅ 所有检查通过！")
    return True


def run_server_demo():
    """演示启动服务器"""
    print_banner("🚀 fi-code 服务器 E2E 测试")
    
    # 创建测试目录
    test_dir = Path("/tmp/fi_code_e2e_test")
    test_dir.mkdir(parents=True, exist_ok=True)
    print(f"📁 测试目录: {test_dir}")
    
    # 构建服务器启动命令
    server_cmd = [
        str(constants.SERVER_BIN),
        "--port", str(constants.SERVER_PORT),
        "--project-dir", str(test_dir),
        "--no-open",
    ]
    
    print(f"\n🏃 启动命令:")
    print(f"   {' '.join(server_cmd)}")
    
    print(f"\n{constants.COLOR_YELLOW}⚠️  这个脚本会在后台启动服务器，但不会自动停止它{constants.COLOR_RESET}")
    print(f"   之后你可以用 curl 或浏览器测试 API")
    
    choice = input(f"\n{constants.COLOR_YELLOW}是否要尝试启动服务器？(y/N): {constants.COLOR_RESET}")
    
    if choice.lower().strip() != 'y':
        print(f"取消启动服务器")
        return
    
    # 启动服务器
    print(f"\n🚀 正在启动 fi-code 服务器...")
    print(f"   端口: {constants.SERVER_PORT}")
    print(f"   项目目录: {test_dir}")
    
    try:
        env = os.environ.copy()
        env["RUST_LOG"] = "info"
        
        proc = subprocess.Popen(
            server_cmd,
            cwd=test_dir,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env=env,
        )
        
        # 等待一会儿
        print(f"⏳ 等待服务器启动...")
        time.sleep(3)
        
        # 检查进程
        if proc.poll() is not None:
            stdout, _ = proc.communicate()
            print(f"{constants.COLOR_RED}服务器启动失败！{constants.COLOR_RESET}")
            print(f"输出:\n{stdout}")
            return
        
        print(f"{constants.COLOR_GREEN}✅ 服务器启动成功！{constants.COLOR_RESET}")
        print(f"\n服务器 PID: {proc.pid}")
        print(f"\n📡 现在可以测试 API:")
        print(f"   curl http://localhost:{constants.SERVER_PORT}/api/health")
        print(f"\n")
        print(f"要停止服务器，按 Ctrl+C 或:")
        print(f"   kill {proc.pid}")
        
        # 保持运行，让用户测试
        try:
            while True:
                time.sleep(1)
        except KeyboardInterrupt:
            print(f"\n{constants.COLOR_YELLOW}停止服务器...{constants.COLOR_RESET}")
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except:
                proc.kill()
            print(f"{constants.COLOR_GREEN}✅ 服务器已停止{constants.COLOR_RESET}")
            
    except Exception as e:
        print(f"{constants.COLOR_RED}错误: {e}{constants.COLOR_RESET}")


def show_test_commands():
    """显示可运行的测试命令"""
    print_banner("📋 可用的测试命令")
    
    print(f"\n1️⃣  Python E2E 检查:")
    print(f"   cd tests/web && python3 test_web_e2e_real.py")
    
    print(f"\n2️⃣  pytest 测试 (真实模式):")
    print(f"   cd tests/web")
    print(f"   USE_MOCK_AI=false python3 -m pytest test_web_e2e_real.py -v")
    
    print(f"\n3️⃣  测试项目管理:")
    print(f"   查看和使用 /tmp/fi_code_test/test_project")
    
    print(f"\n4️⃣  完整服务器测试 (已有 TUI 测试):")
    print(f"   cargo test --test tui_flow_e2e -- --nocapture")


def main():
    """主函数"""
    print(f"\n{constants.COLOR_GREEN}")
    print("╔═══════════════════════════════════════════════════════════════╗")
    print("║       fi-code E2E 测试 - 真实模式                          ║")
    print("╚═══════════════════════════════════════════════════════════════╝")
    print(f"{constants.COLOR_RESET}")
    
    # 检查前置条件
    if not check_prerequisites():
        return
    
    # 显示菜单
    while True:
        print_banner("🎯 E2E 测试菜单")
        
        print(f"1. 🚀 服务器启动演示")
        print(f"2. 📋 查看测试命令")
        print(f"3. ❌ 退出")
        
        choice = input(f"\n{constants.COLOR_YELLOW}请选择 (1-3): {constants.COLOR_RESET}").strip()
        
        if choice == '1':
            run_server_demo()
        elif choice == '2':
            show_test_commands()
        elif choice == '3':
            print(f"\n👋 再见！")
            break
        else:
            print(f"\n{constants.COLOR_RED}无效选择！{constants.COLOR_RESET}")


if __name__ == "__main__":
    main()
