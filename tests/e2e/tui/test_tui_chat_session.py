"""
TUI 会话对话流程测试
验证在已有会话中继续对话。
对应原 Rust 用例: test_chat_with_existing_session
"""
import json
import urllib.request
import pytest
from common.subprocess_utils import parse_sse_events


def create_session(port: int) -> str:
    """创建会话并返回 session_id"""
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/sessions",
        data=json.dumps({"name": "test-session"}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        data = json.loads(resp.read())
        return data["data"]["id"]


def chat_with_session(port: int, session_id: str, message: str) -> list:
    """发送消息并收集 SSE 事件"""
    req = urllib.request.Request(
        f"http://127.0.0.1:{port}/chat",
        data=json.dumps({"session_id": session_id, "message": message}).encode(),
        headers={"Content-Type": "application/json"},
        method="POST"
    )
    with urllib.request.urlopen(req, timeout=60) as resp:
        return parse_sse_events(resp)


@pytest.mark.tui
@pytest.mark.functional
def test_chat_with_existing_session(tui_server):
    """test_chat_with_existing_session"""
    port = tui_server["port"]

    session_id = create_session(port)
    print(f"Created session: {session_id}")

    # 第一轮对话
    events1 = chat_with_session(port, session_id, "你好")
    assert any(e.get("type") == "Done" for e in events1), "First chat should receive Done event"

    # 第二轮对话
    events2 = chat_with_session(port, session_id, "再见")
    assert any(e.get("type") == "Done" for e in events2), "Second chat should receive Done event"

    message_events = [e for e in events2 if e.get("type") == "Message"]
    assert len(message_events) > 0, "Second chat should receive Message events"
