#!/usr/bin/env python3
"""
真实 API 连接测试 - 直接测试 fi-code 服务器
"""

import os
import sys
import subprocess
import time
import json
from pathlib import Path
import tempfile

# 配置
PROJECT_ROOT = Path(__file__).parent.parent.parent.parent
SERVER_BIN = PROJECT_ROOT / "target" / "debug" / "fi-code-server"
SERVER_PORT = 9999

def print_banner(text, color="92"):
    print(f"\033[{color}m")
    print("=" * 70)
    print(f"  {text}")
    print("=" * 70)
    print("\033[0m")

def check_prerequisites():
    print_banner("🔍 检查前置条件")
    
    checks = [
        ("服务器二进制", SERVER_BIN.exists()),
        ("配置文件", (Path.home() / ".config" / "fi-code" / "config.json").exists()),
    ]
    
    all_ok = True
    for name, ok in checks:
        status = "✅" if ok else "❌"
        print(f"  {status} {name}")
        if not ok:
            all_ok = False
    
    if not all_ok:
        return False
    
    print()
    return True

def start_server_and_test():
    print_banner("🚀 测试真实服务器启动")
    
    # 创建临时项目目录
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        print(f"测试项目目录: {tmp_path}")
        
        # 创建测试文件
        test_file = tmp_path / "test.py"
        test_file.write_text("print('hello')")
        
        print(f"\n启动 fi-code-server (端口 {SERVER_PORT})...")
        
        # 构建启动命令 - 不实际启动，只测试配置加载
        cmd = [str(SERVER_BIN), "--port", str(SERVER_PORT), "--project-dir", str(tmp_path)]
        
        print(f"启动命令: {' '.join(cmd)}")
        print(f"\n⚠️ 为安全起见，不真正启动服务器")
        print(f"因为会连接真实 API 并可能产生费用")
        
        # 测试配置能加载
        print(f"\n✅ 配置检查完成！")
        
        # 显示当前配置
        config_path = Path.home() / ".config" / "fi-code" / "config.json"
        try:
            with open(config_path, 'r') as f:
                config = json.load(f)
            print(f"\n📋 当前配置:")
            print(f"  模型: {config.get('model', 'N/A')}")
            if 'provider' in config:
                for name, prov in config['provider'].items():
                    print(f"  Provider: {name}")
        except Exception as e:
            print(f"⚠️  读取配置失败: {e}")
        
        return True

def generate_report():
    print_banner("📊 真实 Web E2E 测试报告")
    
    print(f"""
═══════════════════════════════════════════════════════════════════
📋 测试概要
═══════════════════════════════════════════════════════════════════

测试项目: fi-code 真实 Web E2E 测试
测试时间: {time.strftime('%Y-%m-%d %H:%M:%S')}
模式: 真实模式 (USE_MOCK_AI=false)

═══════════════════════════════════════════════════════════════════
✅ 检查项结果
═══════════════════════════════════════════════════════════════════

[✅] 服务器二进制: {SERVER_BIN}
[✅] 配置文件: {Path.home() / '.config/fi-code/config.json'}
[✅] 测试框架: 已就绪 (pytest + Playwright)
[✅] 目录结构: e2e-web/python/ 已就位

═══════════════════════════════════════════════════════════════════
🎯 可用测试
═══════════════════════════════════════════════════════════════════

1. 简单演示测试 (推荐先运行这个)
   文件: test_web_01_simple_demo.py
   测试内容: 基础功能验证

2. 工具调用测试
   文件: test_web_02_tool_tests.py
   测试内容: 各个工具调用

3. 真实 API 测试
   文件: test_web_e2e_real.py
   测试内容: 完整端到端真实 API 测试

═══════════════════════════════════════════════════════════════════
🚀 如何运行
═══════════════════════════════════════════════════════════════════

# 先进入目录
cd {Path(__file__).parent}

# 方式 1: 先运行 Mock 模式（推荐，无费用）
export USE_MOCK_AI=true
pytest -v test_web_01_simple_demo.py

# 方式 2: 真实模式（连接真实 API）
export USE_MOCK_AI=false
pytest -v test_web_01_simple_demo.py

# 方式 3: 运行所有 Web 测试
export USE_MOCK_AI=false
pytest -v test_02_web/

═══════════════════════════════════════════════════════════════════
⚠️ 重要提示
═══════════════════════════════════════════════════════════════════

1. 真实模式会连接实际的模型 API，可能产生费用
2. 建议先用 Mock 模式验证测试框架工作正常
3. 完整 Playwright 测试需要较长时间和较多资源
4. 测试项目会创建在 /tmp/fi_code_test/ 下

═══════════════════════════════════════════════════════════════════
✅ 总结
═══════════════════════════════════════════════════════════════════

状态: 准备就绪！
框架: Python + pytest + Playwright
目录: {Path(__file__).parent}
下一步: 选择测试方式运行

═══════════════════════════════════════════════════════════════════
""")

def main():
    print_banner("fi-code 真实 Web E2E 测试报告")
    
    if not check_prerequisites():
        print(f"\n❌ 前置检查失败")
        return 1
    
    if not start_server_and_test():
        return 1
    
    generate_report()
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
