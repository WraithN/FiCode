"""
TUI 代码书写流程测试
验证 Agent 调用 write 工具创建文件。
对应原 Rust 用例: test_code_writing_flow
"""
import json
import time
import urllib.request
import pytest
from common.subprocess_utils import parse_sse_events


@pytest.mark.tui
@pytest.mark.functional
def test_code_writing_flow(tui_server):
    """test_code_writing_flow"""
    port = tui_server["port"]
    workspace = tui_server["workspace"]

    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/chat",
        data=json.dumps({
            "session_id": None,
            "message": "帮我写一段代码，创建一个 hello.rs 文件"
        }).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )

    with urllib.request.urlopen(req, timeout=60) as resp:
        assert resp.status == 200
        events = parse_sse_events(resp)

        tool_use_events = [e for e in events if e.get("type") == "ToolUse"]
        assert len(tool_use_events) > 0, "Should receive ToolUse events"

        tool_names = [e.get("name", "") for e in tool_use_events]
        assert "write" in tool_names, f"Should use write tool, got: {tool_names}"

        assert any(e.get("type") == "ToolResult" for e in events), "Should receive ToolResult event"

        # 验证文件已写入
        file_path = workspace / "hello.rs"
        retries = 0
        while not file_path.exists() and retries < 20:
            time.sleep(0.1)
            retries += 1

        assert file_path.exists(), f"File should be written to {file_path}"

        content = file_path.read_text()
        assert "fn main()" in content, f"File should contain Rust code, got: {content}"

        assert any(e.get("type") == "Done" for e in events), "Should receive Done event"
