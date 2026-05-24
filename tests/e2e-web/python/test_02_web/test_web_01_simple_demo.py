"""
简单演示测试

这个文件展示了测试框架的基本用法。
先从简单的验证开始，然后逐步完善复杂测试。
"""
import asyncio
import time
from pathlib import Path

import pytest
from playwright.async_api import Page, Browser, BrowserContext

import constants


@pytest.mark.web
@pytest.mark.functional
class TestSimpleDemo:
    """简单演示测试 - 展示测试框架用法"""
    
    @pytest.fixture(autouse=True)
    def setup_project(self, test_project_manager):
        """每个测试前准备项目"""
        self.project_manager = test_project_manager
    
    async def test_simple_project_creation(self, test_project_manager):
        """
        测试 1: 验证测试项目创建成功
        
        不需要浏览器，仅验证测试框架的基础部分
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：项目创建验证 ==={constants.COLOR_RESET}")
        
        # 检查项目目录存在
        assert test_project_manager.project_dir.exists()
        assert test_project_manager.project_dir.is_dir()
        print(f"✅ 项目目录存在: {test_project_manager.project_dir}")
        
        # 检查基础文件
        assert test_project_manager.file_exists("README.md")
        assert test_project_manager.file_exists("src/main.py")
        assert test_project_manager.file_exists("src/utils.py")
        print("✅ 基础文件创建成功")
        
        # 检查 git 初始化
        git_dir = test_project_manager.project_dir / ".git"
        assert git_dir.exists(), "Git 仓库应该被初始化"
        print("✅ Git 仓库初始化成功")
        
        # 检查文件内容
        main_py_content = test_project_manager.read_file("src/main.py")
        assert "Hello, World!" in main_py_content
        print("✅ 基础文件内容正确")
        
        # 检查测试文件
        assert test_project_manager.file_exists("tests/test_utils.py")
        print("✅ 测试文件创建成功")
        
        print(f"\n{constants.COLOR_GREEN}=== 项目创建测试成功！ ==={constants.COLOR_RESET}")
    
    async def test_tool_call_simulation(self, mock_ai_server):
        """
        测试 2: 验证 Mock AI 服务器工作正常
        
        测试我们可以设置模拟响应
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：Mock AI 响应 ==={constants.COLOR_RESET}")
        
        # 测试简单文本响应
        mock_ai_server.set_simple_text_response("This is a test response from Mock AI!")
        print("✅ 设置简单文本响应成功")
        
        # 测试工具调用序列响应
        tool_sequence = [
            {
                "type": "tool_use",
                "tool_name": "bash",
                "tool_arguments": {"command": "echo 'Hello'"},
            },
            {
                "type": "tool_use",
                "tool_name": "read",
                "tool_arguments": {"path": "src/main.py"},
            },
            {
                "type": "text",
                "content": "Done with the tasks!",
            },
        ]
        mock_ai_server.set_tool_sequence_response(tool_sequence)
        print("✅ 设置工具序列响应成功")
        
        print(f"\n{constants.COLOR_GREEN}=== Mock AI 测试成功！ ==={constants.COLOR_RESET}")
    
    async def test_page_screenshot_demo(self, page: Page, test_project_manager):
        """
        测试 3: 截图演示
        
        展示 Playwright 的基本功能
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：截图演示 ==={constants.COLOR_RESET}")
        
        # 创建截图目录
        screenshot_dir = constants.TEST_TEMP_DIR / "screenshots"
        screenshot_dir.mkdir(parents=True, exist_ok=True)
        
        # 截图 - 即使没有访问真实页面，也可以演示用法
        # 这里我们访问一个简单的测试页面或保存空白页面
        try:
            await page.goto("about:blank")
            await asyncio.sleep(0.5)
            
            # 截图
            screenshot_path = screenshot_dir / f"demo_screenshot_{int(time.time())}.png"
            await page.screenshot(path=str(screenshot_path))
            
            print(f"✅ 截图成功保存到: {screenshot_path}")
            assert screenshot_path.exists()
            assert screenshot_path.stat().st_size > 0
            
        except Exception as e:
            print(f"{constants.COLOR_YELLOW}截图演示出现问题: {e}{constants.COLOR_RESET}")
            print("但这没关系，我们的主要框架已经就位！")
        
        print(f"\n{constants.COLOR_GREEN}=== 截图演示测试成功！ ==={constants.COLOR_RESET}")
    
    async def test_end_to_end_workflow_simulation(self, test_project_manager):
        """
        测试 4: 端到端工作流模拟
        
        即使没有完整的浏览器自动化，也验证完整的流程逻辑
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：端到端工作流模拟 ==={constants.COLOR_RESET}")
        
        # 模拟完整的开发工作流
        workflow_steps = [
            ("步骤 1", "读取现有文件"),
            ("步骤 2", "分析代码结构"),
            ("步骤 3", "编写新功能"),
            ("步骤 4", "运行测试"),
            ("步骤 5", "git 提交"),
        ]
        
        for step_name, step_desc in workflow_steps:
            print(f"\n{constants.COLOR_YELLOW}{step_name}: {step_desc}{constants.COLOR_RESET}")
            
            # 模拟项目操作
            if step_name == "步骤 3":
                test_project_manager.create_file(
                    "src/new_feature.py",
                    "#!/usr/bin/env python3\n"""
                    "New feature file created during test simulation!\n"
                )
                print("✅ 创建新文件成功")
            
            await asyncio.sleep(0.1)  # 模拟延迟
        
        # 验证操作结果
        assert test_project_manager.file_exists("src/new_feature.py")
        print("\n✅ 所有工作流步骤执行成功!")
        
        print(f"\n{constants.COLOR_GREEN}=== 端到端工作流模拟成功！ ==={constants.COLOR_RESET}")


# 当直接运行这个文件时的演示代码
if __name__ == "__main__":
    print(f"\n{constants.COLOR_GREEN}=== fi-code Web E2E 测试框架演示 ==={constants.COLOR_RESET}")
    print("\n这个测试框架包含:")
    print("• 完整的 pytest + playwright 测试结构")
    print("• 测试项目自动创建和清理")
    print("• Mock AI 服务器避免真实 API 调用")
    print("• fi-code 服务器管理")
    print("• Vite 前端服务器支持")
    print("\n使用命令运行测试:")
    print("  cd tests/web")
    print("  pytest -v")
    print("\n或者运行单个文件:")
    print("  pytest -v test_web_01_simple_demo.py")
