"""
测试常量定义
"""
import os
from pathlib import Path

# 项目根目录 (tests/e2e-web/python/ 上溯 4 级)
PROJECT_ROOT = Path(__file__).parent.parent.parent.parent

# 测试临时目录
TEST_TEMP_DIR = Path("/tmp/fi_code_test")
TEST_PROJECT_DIR = TEST_TEMP_DIR / "test_project"

# 前端和服务器配置
FRONTEND_PORT = 3001  # 使用非默认端口避免冲突
SERVER_PORT = 4041

# 服务器二进制路径
# 注意：fi-code-server 不支持 CLI 参数，无法指定端口；改用 fi-code-cli server --port
SERVER_BIN = PROJECT_ROOT / "target/debug/fi-code-cli"
CLI_BIN = PROJECT_ROOT / "target/debug/fi-code-cli"

# Mock 配置
MOCK_SERVER_PORT = 8888
# 是否使用 Mock AI - 可以通过环境变量 USE_MOCK_AI=false 关闭
USE_MOCK_AI = os.getenv("USE_MOCK_AI", "true").lower() != "false"

# 浏览器配置
BROWSER_TIMEOUT = 30000  # 30秒
PAGE_LOAD_TIMEOUT = 30000
SSE_WAIT_TIMEOUT = 60000

# 测试用例超时
TEST_TIMEOUT = 300  # 5分钟

# 工具列表
ALL_TOOLS = [
    "bash",
    "read",
    "write",
    "edit",
    "glob",
    "grep",
    "git",
    "git_status",
    "git_diff",
    "git_add",
    "git_commit",
    "git_log",
    "git_worktree",
    "ask_for_question",
    "create_task_plan",
    "handle_task_plan",
    "use_skill",
    "web_fetch",
]

# 颜色常量 (用于日志)
COLOR_GREEN = "\033[92m"
COLOR_YELLOW = "\033[93m"
COLOR_RED = "\033[91m"
COLOR_RESET = "\033[0m"
