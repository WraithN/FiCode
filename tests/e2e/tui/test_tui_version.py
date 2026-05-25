"""
TUI Version 测试
验证 fi-code-tui --version 输出包含版本号。
对应原 Rust 用例: test_tui_version_flag_shows_version
"""
import pytest
from common.subprocess_utils import run_binary
from common.constants import TUI_BIN


@pytest.mark.tui
@pytest.mark.functional
def test_tui_version_flag_shows_version():
    """test_tui_version_flag_shows_version"""
    result = run_binary(TUI_BIN, ["--version"])
    assert result.returncode == 0
    assert "0.1.0" in result.stdout, (
        f"Expected version output, got:\n{result.stdout}"
    )
