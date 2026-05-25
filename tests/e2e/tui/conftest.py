"""
TUI 测试共享 fixtures
"""
import os
import shutil
import subprocess
import time
import pytest
import psutil
from common.subprocess_utils import get_available_port
from common.constants import TUI_BIN, TEST_PROJECT_DIR


@pytest.fixture
def tui_server():
    """启动 TUI 后端服务器（FI_CODE_TEST_MODE=1）"""
    port = get_available_port()
    workspace = TEST_PROJECT_DIR / "tui_test"
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.mkdir(parents=True, exist_ok=True)

    proc = subprocess.Popen(
        [str(TUI_BIN), "--port", str(port)],
        env={**os.environ, "FI_CODE_TEST_MODE": "1"},
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        cwd=workspace
    )

    time.sleep(1)
    yield {"port": port, "proc": proc, "workspace": workspace}

    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)

    try:
        parent = psutil.Process(proc.pid)
        for child in parent.children(recursive=True):
            child.kill()
        parent.kill()
    except psutil.NoSuchProcess:
        pass

    shutil.rmtree(workspace, ignore_errors=True)
