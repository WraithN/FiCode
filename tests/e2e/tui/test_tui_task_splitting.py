"""
TUI 任务拆分流程测试
验证 Agent 调用 handle_task_plan 工具拆分任务。
对应原 Rust 用例: test_task_splitting_flow
"""
import json
import urllib.request
import pytest
from common.subprocess_utils import parse_sse_events


@pytest.mark.tui
@pytest.mark.functional
def test_task_splitting_flow(tui_server):
    """test_task_splitting_flow"""
    port = tui_server["port"]

    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/chat",
        data=json.dumps({
            "session_id": None,
            "message": "我有一个复杂任务，请帮我拆分任务并执行"
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
        assert "handle_task_plan" in tool_names, (
            f"Should use handle_task_plan tool, got: {tool_names}"
        )

        assert any(e.get("type") == "ToolResult" for e in events), "Should receive ToolResult event"
        assert any(e.get("type") == "Done" for e in events), "Should receive Done event"
