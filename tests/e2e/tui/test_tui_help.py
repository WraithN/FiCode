"""
TUI Help 测试
验证 fi-code-tui --help 输出包含使用说明。
对应原 Rust 用例: test_tui_help_flag_shows_usage
"""
import pytest
from common.subprocess_utils import run_binary
from common.constants import TUI_BIN


@pytest.mark.tui
@pytest.mark.functional
def test_tui_help_flag_shows_usage():
    """test_tui_help_flag_shows_usage"""
    result = run_binary(TUI_BIN, ["--help"])
    assert result.returncode == 0
    output = result.stdout + result.stderr
    assert "fi-code" in output or "Usage:" in output, (
        f"Expected help output, got:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}"
    )
