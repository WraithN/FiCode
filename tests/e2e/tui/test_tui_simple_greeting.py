"""
TUI 简单问候流程测试
验证通过 HTTP API 发送消息后，能收到预期的 SSE 事件流。
对应原 Rust 用例: test_simple_greeting_flow
"""
import json
import urllib.request
import pytest
from common.subprocess_utils import parse_sse_events


@pytest.mark.tui
@pytest.mark.functional
def test_simple_greeting_flow(tui_server):
    """test_simple_greeting_flow"""
    port = tui_server["port"]

    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/chat",
        data=json.dumps({"session_id": None, "message": "你好，你是谁"}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )

    with urllib.request.urlopen(req, timeout=60) as resp:
        assert resp.status == 200
        events = parse_sse_events(resp)

        message_events = [e for e in events if e.get("type") == "Message"]
        assert len(message_events) > 0, f"Should receive Message events, got: {events}"

        all_text = "".join(e.get("content", "") for e in message_events)
        assert "FiCode" in all_text or "编程" in all_text, (
            f"Expected greeting text, got: {all_text}"
        )

        assert any(e.get("type") == "Done" for e in events), "Should receive Done event"
        assert not any(e.get("type") == "Error" for e in events), "Should not receive Error events"
