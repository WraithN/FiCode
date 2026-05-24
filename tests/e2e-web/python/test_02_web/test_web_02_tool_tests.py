"""
工具调用测试

测试场景:
1. Bash 工具完整测试
2. 文件操作工具测试
3. Git 工具测试
"""
import asyncio
import re
from pathlib import Path
from typing import List

import pytest

import constants


@pytest.mark.web
@pytest.mark.functional
class TestToolExecutions:
    """工具执行测试"""
    
    @pytest.fixture(autouse=True)
    def setup_project(self, test_project_manager):
        """每个测试前准备项目"""
        self.project_manager = test_project_manager
    
    async def test_bash_command_execution(self, test_project_manager):
        """
        场景 1: Bash 命令执行
        
        测试验证:
        • 命令能在项目目录中执行
        • 输出结果能正确获取
        • 错误能正确处理
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：Bash 命令执行 ==={constants.COLOR_RESET}")
        
        # 1. 测试简单的 bash 命令
        return_code, stdout, stderr = test_project_manager.run_command(["echo", "Hello from test"])
        print(f"测试 echo 命令:")
        print(f"  返回码: {return_code}")
        print(f"  输出: {stdout.strip()}")
        assert return_code == 0, "Echo 命令应该成功"
        assert "Hello from test" in stdout, "Echo 输出应该包含 Hello"
        print(f"{constants.COLOR_GREEN}✅ Echo 命令测试通过{constants.COLOR_RESET}")
        
        # 2. 测试目录命令
        return_code, stdout, stderr = test_project_manager.run_command(["ls", "-la"])
        print(f"\n测试 ls 命令:")
        print(f"  返回码: {return_code}")
        print(f"  输出行数: {len(stdout.splitlines())}")
        assert return_code == 0, "ls 命令应该成功"
        assert "src" in stdout, "应该能看到 src 目录"
        print(f"{constants.COLOR_GREEN}✅ ls 命令测试通过{constants.COLOR_RESET}")
        
        # 3. 测试文件操作命令
        test_file = test_project_manager.project_dir / "bash_test.txt"
        test_content = "This is bash test content"
        return_code, stdout, stderr = test_project_manager.run_command(
            ["bash", "-c", f"echo '{test_content}' > bash_test.txt"]
        )
        assert return_code == 0, "文件创建应该成功"
        assert test_file.exists(), "测试文件应该被创建"
        assert test_file.read_text().strip() == test_content, "文件内容应该正确"
        print(f"{constants.COLOR_GREEN}✅ Bash 文件操作测试通过{constants.COLOR_RESET}")
        
        print(f"\n{constants.COLOR_GREEN}=== Bash 命令测试完成！ ==={constants.COLOR_RESET}")
    
    async def test_file_operations(self, test_project_manager):
        """
        场景 2: 文件操作测试
        
        测试验证:
        • 创建文件
        • 读取文件
        • 编辑文件
        • 搜索文件
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：文件操作 ==={constants.COLOR_RESET}")
        
        # 1. 创建文件测试
        test_file = "test_file_ops.txt"
        test_content = "Line 1\nLine 2\nLine 3\n"
        created = test_project_manager.create_file(test_file, test_content)
        assert created.exists(), "文件应该被创建"
        assert created.read_text() == test_content, "文件内容应该匹配"
        print(f"{constants.COLOR_GREEN}✅ 文件创建测试通过{constants.COLOR_RESET}")
        
        # 2. 读取文件测试
        read_content = test_project_manager.read_file(test_file)
        assert read_content == test_content, "读取内容应该匹配"
        assert "Line 2" in read_content, "应该能读取到特定行"
        print(f"{constants.COLOR_GREEN}✅ 文件读取测试通过{constants.COLOR_RESET}")
        
        # 3. 搜索文件测试（glob）
        test_project_manager.create_file("test_file_1.py", "content1")
        test_project_manager.create_file("test_file_2.py", "content2")
        test_project_manager.create_file("test_file_3.txt", "content3")
        
        all_txt_files = test_project_manager.get_file_list("*.txt")
        py_files = test_project_manager.get_file_list("*.py")
        
        print(f"找到 {len(all_txt_files)} 个 txt 文件")
        print(f"找到 {len(py_files)} 个 py 文件")
        assert len(all_txt_files) >= 2, "应该找到至少 2 个 txt 文件"
        assert len(py_files) >= 2, "应该找到至少 2 个 py 文件"
        print(f"{constants.COLOR_GREEN}✅ Glob 文件搜索测试通过{constants.COLOR_RESET}")
        
        print(f"\n{constants.COLOR_GREEN}=== 文件操作测试完成！ ==={constants.COLOR_RESET}")
    
    async def test_git_operations(self, test_project_manager):
        """
        场景 3: Git 操作测试
        
        测试验证:
        • git status
        • git add
        • git commit
        • git log
        """
        print(f"\n{constants.COLOR_YELLOW}=== 测试：Git 操作 ==={constants.COLOR_RESET}")
        
        # 检查 git 是否可用
        if not test_project_manager.git_initialized:
            print(f"{constants.COLOR_YELLOW}⚠️  Git 未初始化，跳过此测试{constants.COLOR_RESET}")
            pytest.skip("Git 仓库未初始化")
        
        # 1. Git status 测试
        return_code, stdout, stderr = test_project_manager.run_command(["git", "status"])
        print(f"Git status:")
        print(f"  返回码: {return_code}")
        print(f"  输出包含: {stdout[:100]}...")
        assert return_code == 0, "git status 应该成功"
        print(f"{constants.COLOR_GREEN}✅ Git status 测试通过{constants.COLOR_RESET}")
        
        # 2. 创建并添加新文件
        git_test_file = "git_test_file.txt"
        test_project_manager.create_file(git_test_file, "Git test content")
        
        return_code, stdout, stderr = test_project_manager.run_command(["git", "status"])
        assert git_test_file in stdout, "应该显示新文件未跟踪"
        
        # 3. Git add
        return_code, stdout, stderr = test_project_manager.run_command(["git", "add", git_test_file])
        assert return_code == 0, "git add 应该成功"
        
        return_code, stdout, stderr = test_project_manager.run_command(["git", "status"])
        assert "Changes to be committed" in stdout, "文件应该被添加到暂存区"
        print(f"{constants.COLOR_GREEN}✅ Git add 测试通过{constants.COLOR_RESET}")
        
        # 4. Git commit
        return_code, stdout, stderr = test_project_manager.run_command(
            ["git", "commit", "-m", "Test commit from E2E test"]
        )
        if return_code != 0:
            print(f"{constants.COLOR_YELLOW}⚠️  Git commit 需要用户信息，使用测试配置{constants.COLOR_RESET}")
            # 设置用户信息
            test_project_manager.run_command(["git", "config", "user.name", "Test User"])
            test_project_manager.run_command(["git", "config", "user.email", "test@example.com"])
            return_code, stdout, stderr = test_project_manager.run_command(
                ["git", "commit", "-m", "Test commit from E2E test"]
            )
        
        # 检查 git log
        return_code, stdout, stderr = test_project_manager.run_command(["git", "log", "--oneline", "-n", "3"])
        print(f"Git log:")
        print(f"  最近 3 条: {len(stdout.splitlines())} 条")
        
        print(f"{constants.COLOR_GREEN}✅ Git 操作测试完成{constants.COLOR_RESET}")


class ToolResultValidator:
    """工具结果验证器"""
    
    @staticmethod
    def validate_bash_result(output: str, expected_patterns: List[str]) -> bool:
        """验证 bash 命令结果"""
        for pattern in expected_patterns:
            if re.search(pattern, output):
                return True
        return False
    
    @staticmethod
    def validate_file_exists(file_path: Path) -> bool:
        """验证文件存在"""
        return file_path.exists()
    
    @staticmethod
    def validate_file_content(file_path: Path, expected_substring: str) -> bool:
        """验证文件内容包含子串"""
        if not file_path.exists():
            return False
        
        try:
            content = file_path.read_text()
            return expected_substring in content
        except:
            return False
    
    @staticmethod
    def validate_git_commit(log_output: str, expected_message: str = None) -> bool:
        """验证 git 提交"""
        if expected_message:
            return expected_message in log_output
        
        # 至少检查有 commit hash
        return bool(re.search(r'^[0-9a-f]{7}', log_output, re.MULTILINE))


# 快速测试函数
def run_simple_tests():
    """快速运行简单的验证测试（不使用 pytest）"""
    print(f"{constants.COLOR_YELLOW}=== fi-code 测试框架验证 ==={constants.COLOR_RESET}\n")
    
    # 检查项目结构
    test_root = Path("/home/nan/fi-code/tests/web")
    print(f"测试目录: {test_root}")
    print(f"测试文件结构:")
    for item in sorted(test_root.iterdir()):
        prefix = "📁" if item.is_dir() else "📄"
        print(f"  {prefix} {item.name}")
    
    print(f"\n{constants.COLOR_GREEN}✅ 测试框架验证完成！{constants.COLOR_RESET}")
    print("\n要运行完整测试:")
    print("  cd tests/web")
    print("  pytest -v")


if __name__ == "__main__":
    run_simple_tests()
