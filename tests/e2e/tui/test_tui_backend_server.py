"""
TUI Backend Server 测试
验证 fi-code-tui 在 FI_CODE_TEST_MODE=1 时能启动后端服务器。
对应原 Rust 用例: test_tui_starts_backend_server
"""
import os
import time
import urllib.request
import pytest
import psutil
import subprocess
from common.subprocess_utils import get_available_port
from common.constants import TUI_BIN


@pytest.mark.tui
@pytest.mark.functional
def test_tui_starts_backend_server():
    """test_tui_starts_backend_server"""
    port = get_available_port()
    proc = subprocess.Popen(
        [str(TUI_BIN), "--port", str(port)],
        env={**os.environ, "FI_CODE_TEST_MODE": "1"},
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE
    )

    try:
        time.sleep(3)

        if proc.poll() is not None:
            stdout = proc.stdout.read().decode(errors="replace") if proc.stdout else ""
            stderr = proc.stderr.read().decode(errors="replace") if proc.stderr else ""
            pytest.fail(f"TUI exited early. stdout:\n{stdout}\nstderr:\n{stderr}")

        try:
            resp = urllib.request.urlopen(
                f"http://127.0.0.1:{port}/api/config", timeout=5
            )
            assert resp.status == 200, f"Expected 200, got {resp.status}"
        except Exception as e:
            pytest.fail(f"Failed to connect to TUI backend: {e}")
    finally:
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
