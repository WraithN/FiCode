"""
TUI SSE 流生命周期测试
验证 SSE 事件顺序和完整性。
对应原 Rust 用例: test_sse_stream_lifecycle
"""
import json
import urllib.request
import pytest
from common.subprocess_utils import parse_sse_events


@pytest.mark.tui
@pytest.mark.functional
def test_sse_stream_lifecycle(tui_server):
    """test_sse_stream_lifecycle"""
    port = tui_server["port"]

    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/chat",
        data=json.dumps({"session_id": None, "message": "你好"}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )

    with urllib.request.urlopen(req, timeout=60) as resp:
        assert resp.status == 200
        events = parse_sse_events(resp)

        assert len(events) > 0, "Should receive events"

        message_events = [e for e in events if e.get("type") == "Message"]
        assert len(message_events) > 0, "Should receive at least one Message event"

        all_text = "".join(e.get("content", "") for e in message_events)
        assert len(all_text) > 0, "Message content should not be empty"

        # 最后一条应该是 Done
        assert events[-1].get("type") == "Done", (
            f"Last event should be Done, got: {events[-1]}"
        )

        error_events = [e for e in events if e.get("type") == "Error"]
        assert len(error_events) == 0, f"Should not receive Error events, got: {error_events}"
