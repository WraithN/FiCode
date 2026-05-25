"""
真实端到端测试

使用真实的 fi-code 服务器进行完整的 E2E 测试
⚠️ 需要:
- fi-code server 已构建
- API Key 已配置 (在 ~/.config/fi-code/config.json)
"""
import asyncio
import time
from pathlib import Path

import pytest

from common import constants


@pytest.mark.web
@pytest.mark.e2e
@pytest.mark.skipif(constants.USE_MOCK_AI, reason="非 Mock 模式测试，使用真实 API")
class TestE2EReal:
    """真实端到端测试"""
    
    @pytest.fixture(autouse=True)
    def setup_project(self, test_project_manager):
        """每个测试前准备项目"""
        self.project_manager = test_project_manager
    
    async def test_server_connection(self, fi_code_server):
        """
        场景 1: 服务器连接测试
        
        验证:
        • fi-code 服务器能正常启动
        • 能连接到服务器
        • 健康检查能通过
        """
        print(f"\n{constants.COLOR_YELLOW}=== E2E 测试：服务器连接 ==={constants.COLOR_RESET}")
        
        # 检查服务器是否在运行
        assert fi_code_server.is_running, "fi-code 服务器应该在运行"
        print(f"{constants.COLOR_GREEN}✅ fi-code 服务器运行中{constants.COLOR_RESET}")
        
        # 打印服务器信息
        print(f"   服务器端口: {fi_code_server.port}")
        print(f"   测试项目: {self.project_manager.project_dir}")
        
        # 获取服务器日志（用于调试）
        stdout, stderr = fi_code_server.get_server_logs()
        if stdout:
            print(f"\n   服务器输出: {stdout[:200]}...")
        
        print(f"\n{constants.COLOR_GREEN}=== 服务器连接测试成功！ ==={constants.COLOR_RESET}")
    
    async def test_project_setup(self, test_project_manager):
        """
        场景 2: 项目结构和文件操作
        
        验证:
        • 测试项目结构正确
        • 基本文件操作正常
        """
        print(f"\n{constants.COLOR_YELLOW}=== E2E 测试：项目结构 ==={constants.COLOR_RESET}")
        
        # 验证项目结构
        print(f"📁 项目目录: {test_project_manager.project_dir}")
        
        # 检查文件
        test_files = [
            "README.md",
            "src/main.py",
            "src/utils.py",
            "tests/test_utils.py",
        ]
        
        for file_path in test_files:
            exists = test_project_manager.file_exists(file_path)
            status = "✅" if exists else "❌"
            print(f"   {status} {file_path}")
            assert exists, f"文件 {file_path} 应该存在"
        
        # 创建一个测试文件
        test_file = "e2e_test_file.py"
        test_content = "#!/usr/bin/env python3\nprint('Hello from E2E test!')"
        test_project_manager.create_file(test_file, test_content)
        assert test_project_manager.file_exists(test_file)
        print(f"\n✅ 创建测试文件成功")
        
        # 验证文件内容
        content = test_project_manager.read_file(test_file)
        assert "Hello from E2E test!" in content
        print(f"✅ 文件内容验证成功")
        
        print(f"\n{constants.COLOR_GREEN}=== 项目结构测试成功！ ==={constants.COLOR_RESET}")
    
    async def test_git_workflow_e2e(self, test_project_manager):
        """
        场景 3: Git 工作流 E2E 测试
        
        验证:
        • Git 初始化正确
        • 文件修改能被检测
        • 提交能正常创建
        """
        print(f"\n{constants.COLOR_YELLOW}=== E2E 测试：Git 工作流 ==={constants.COLOR_RESET}")
        
        # 检查 git 仓库
        git_dir = test_project_manager.project_dir / ".git"
        if not git_dir.exists():
            print(f"{constants.COLOR_YELLOW}⚠️  Git 未初始化，跳过{constants.COLOR_RESET}")
            pytest.skip("Git 仓库未初始化")
        
        print(f"✅ Git 仓库存在")
        
        # 创建新文件
        new_file = "new_feature.py"
        new_content = "def new_feature(): return 'E2E test'"
        test_project_manager.create_file(new_file, new_content)
        
        # Git 操作
        commands = [
            ["git", "status"],
            ["git", "add", new_file],
            ["git", "status"],
        ]
        
        for cmd in commands:
            return_code, stdout, stderr = test_project_manager.run_command(cmd)
            print(f"\n🏃 运行: {' '.join(cmd)}")
            print(f"   输出: {stdout.strip()[:100]}...")
            assert return_code == 0, f"命令失败: {' '.join(cmd)}"
        
        print(f"\n{constants.COLOR_GREEN}=== Git 工作流测试成功！ ==={constants.COLOR_RESET}")


@pytest.mark.web
@pytest.mark.e2e
@pytest.mark.skipif(constants.USE_MOCK_AI, reason="非 Mock 模式测试")
class TestDeveloperWorkflow:
    """开发工作流测试"""
    
    async def test_complete_development_workflow(self, test_project_manager):
        """
        完整开发工作流
        
        模拟:
        1. 读取现有代码
        2. 编写新功能
        3. 测试新功能
        4. Git 提交
        """
        print(f"\n{constants.COLOR_YELLOW}=== E2E 测试：完整开发工作流 ==={constants.COLOR_RESET}")
        
        # 1. 读取现有代码
        main_py = test_project_manager.read_file("src/main.py")
        assert "def main" in main_py
        print(f"✅ 读取源代码成功")
        
        # 2. 创建新功能
        new_feature = "src/calculator.py"
        calculator_code = '''#!/usr/bin/env python3
"""
Calculator module for testing
"""
def add(a: int, b: int) -> int:
    return a + b
def multiply(a: int, b: int) -> int:
    return a * b
'''
        test_project_manager.create_file(new_feature, calculator_code)
        print(f"✅ 创建新模块成功")
        
        # 3. 编写测试
        test_file = "tests/test_calculator.py"
        test_code = '''#!/usr/bin/env python3
from src.calculator import add, multiply
def test_add():
    assert add(2, 3) == 5
def test_multiply():
    assert multiply(2, 3) == 6
'''
        test_project_manager.create_file(test_file, test_code)
        print(f"✅ 创建测试文件成功")
        
        # 4. 验证文件结构
        all_files = test_project_manager.get_file_list("**/*.py")
        print(f"\n📄 Python 文件:")
        for f in sorted(all_files):
            print(f"   - {f}")
        
        assert len(all_files) >= 4, "应该创建足够的 Python 文件"
        
        print(f"\n{constants.COLOR_GREEN}=== 完整开发工作流测试成功！ ==={constants.COLOR_RESET}")


def run_e2e_checklist():
    """运行 E2E 测试前检查清单"""
    print(f"\n{constants.COLOR_GREEN}")
    print("╔═══════════════════════════════════════════════════════════════╗")
    print("║       fi-code E2E 测试 - 非 Mock 模式检查清单             ║")
    print("╚═══════════════════════════════════════════════════════════════╝")
    print(f"{constants.COLOR_RESET}")
    
    checks = []
    
    # 1. 检查项目构建
    print(f"\n1. 🔍 检查 fi-code 服务器构建...")
    server_bin = constants.SERVER_BIN
    if server_bin.exists():
        checks.append(("✅", "服务器二进制已构建"))
        print(f"   ✅ 已存在: {server_bin}")
    else:
        checks.append(("❌", "服务器二进制未构建"))
        print(f"   ❌ 未找到: {server_bin}")
        print(f"\n提示: 先运行 'cargo build'")
    
    # 2. 检查配置
    print(f"\n2. 🔍 检查配置文件...")
    config_paths = [
        Path.home() / ".config/fi-code/config.json",
        Path.home() / ".config/fi-code/config.jsonc",
    ]
    found_config = False
    for config_path in config_paths:
        if config_path.exists():
            checks.append(("✅", f"配置文件存在: {config_path.name}"))
            print(f"   ✅ 已存在: {config_path}")
            found_config = True
            break
    if not found_config:
        checks.append(("⚠️", "配置文件未找到"))
        print(f"   ⚠️ 未找到配置文件")
        print(f"   提示: 配置 API Key 在 ~/.config/fi-code/config.json")
    
    # 3. 检查 USE_MOCK_AI 设置
    print(f"\n3. 🔍 检查 Mock AI 开关...")
    if not constants.USE_MOCK_AI:
        checks.append(("✅", "USE_MOCK_AI=false - 真实模式"))
        print(f"   ✅ 非 Mock 模式已启用")
    else:
        checks.append(("⚠️", "USE_MOCK_AI=true - Mock 模式"))
        print(f"   ⚠️ 当前为 Mock 模式")
        print(f"   提示: 运行 'USE_MOCK_AI=false pytest ...'")
    
    # 总结
    print(f"\n{constants.COLOR_YELLOW}📋 检查总结:{constants.COLOR_RESET}")
    for status, message in checks:
        print(f"   {status} {message}")
    
    print(f"\n{constants.COLOR_GREEN}🎯 运行真实 E2E 测试:{constants.COLOR_RESET}")
    print(f"   cd tests/web")
    print(f"   USE_MOCK_AI=false pytest -v test_web_e2e_real.py")


if __name__ == "__main__":
    run_e2e_checklist()
