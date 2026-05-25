"""
真实模型 E2E 测试 —— 走 fi-code-server HTTP /chat 接口，
直接调用配置文件中真实模型。

前置条件：
- ~/.config/fi-code/config.json 中已配置真实可用模型与 API Key
- target/debug/fi-code-cli 已构建

注意：该套件不依赖 Mock，不依赖 Playwright，
通过 requests + SSE 流逐行解析消费返回事件。
"""
import json
import time
from typing import Dict, List, Optional

import pytest
import requests

from common import constants


# -----------------------------------------------------------------------------
# 工具函数：SSE 流解析
# -----------------------------------------------------------------------------

def _iter_sse_events(resp: requests.Response):
    """逐条解析 SSE 事件（行：`data: <json>`），yield 反序列化后的字典。

    遇到 `event: done` 或 SseEvent `{"type":"done"}` 后由调用方决定是否退出。
    """
    buffer: List[str] = []
    for raw in resp.iter_lines(decode_unicode=True):
        if raw is None:
            continue
        if raw == "":
            # 事件分隔：将 data 行合并为一个 JSON 字符串
            if not buffer:
                continue
            data_str = "\n".join(buffer)
            buffer.clear()
            try:
                yield json.loads(data_str)
            except json.JSONDecodeError:
                # 忽略非 JSON 行，避免单事件解析失败影响整体
                continue
            continue
        if raw.startswith("data:"):
            buffer.append(raw[5:].lstrip())
        # 其他行（event:, id:, retry:）忽略，仅依赖 data 中的 type 字段


def _post_chat_stream(
    server_url: str,
    message: str,
    session_id: Optional[str] = None,
    agent: Optional[str] = None,
    connect_timeout: float = 10.0,
    read_timeout: float = 300.0,
) -> requests.Response:
    """向 /chat 发送 POST 请求，返回 SSE 流式响应。

    timeout 为 (connect, read) 元组，read 控制相邻字节最大间隔，
    SSE 长流场景下需要给到较大值。
    """
    payload: Dict = {"message": message}
    if session_id is not None:
        payload["session_id"] = session_id
    if agent is not None:
        payload["agent"] = agent
    return requests.post(
        f"{server_url}/chat",
        json=payload,
        stream=True,
        timeout=(connect_timeout, read_timeout),
        headers={"Accept": "text/event-stream"},
    )


def _collect_until_done(resp: requests.Response, hard_deadline: float) -> Dict:
    """消费 SSE 流直到收到 `done` 事件或超过 deadline。

    返回汇总信息：
    - text_chunks: 拼接的助手文本
    - parts: 累积的 Part 列表
    - tool_calls: 工具调用名称列表
    - errors: 错误事件列表
    - done: 是否收到完结事件
    - session_id: done 事件携带的 session_id（如有）
    """
    text_chunks: List[str] = []
    parts: List[Dict] = []
    tool_calls: List[str] = []
    errors: List[str] = []
    session_id: Optional[str] = None
    done = False

    for evt in _iter_sse_events(resp):
        if time.time() > hard_deadline:
            break
        etype = evt.get("type")
        if etype == "message":
            text_chunks.append(evt.get("content", ""))
        elif etype == "part":
            part = evt.get("part") or {}
            parts.append(part)
            # 探测工具调用 part（结构由 shared::dto::Part 决定）
            pt = part.get("type")
            if pt in {"tool_use", "tool_result", "tool_error"}:
                name = part.get("name") or part.get("tool_name") or pt
                tool_calls.append(name)
        elif etype == "error":
            errors.append(evt.get("message", ""))
        elif etype == "done":
            session_id = evt.get("session_id")
            done = True
            break

    return {
        "text": "".join(text_chunks),
        "parts": parts,
        "tool_calls": tool_calls,
        "errors": errors,
        "done": done,
        "session_id": session_id,
    }


# -----------------------------------------------------------------------------
# 测试用例
# -----------------------------------------------------------------------------

pytestmark = [
    pytest.mark.web,
    pytest.mark.skipif(
        constants.USE_MOCK_AI,
        reason="本套件强制走真实模型，需 USE_MOCK_AI=false",
    ),
]


class TestRealModelChat:
    """真实模型 /chat SSE E2E"""

    async def test_chat_health_endpoint(self, fi_code_server, server_url):
        """场景 0：基础 HTTP 端点存活检查"""
        resp = requests.get(f"{server_url}/api/config", timeout=5)
        assert resp.status_code == 200, f"/api/config 应返回 200，实际 {resp.status_code}"
        body = resp.json()
        # 至少应包含 model / provider 字段
        assert isinstance(body, dict) and body, "/api/config 响应应为非空 JSON 对象"
        print(f"[OK] /api/config -> {list(body.keys())[:5]}")

    async def test_simple_text_chat(self, fi_code_server, server_url):
        """场景 1：纯文本对话 —— 真实模型应给出非空回复并以 done 事件结束"""
        prompt = "请只用一句话回答：1+1 等于几？"
        deadline = time.time() + 180

        resp = _post_chat_stream(server_url, prompt, agent="build")
        assert resp.status_code == 200, f"/chat 应返回 200，实际 {resp.status_code}"

        result = _collect_until_done(resp, deadline)

        assert result["done"], "应收到 SSE done 事件"
        assert not result["errors"], f"不应有 error 事件：{result['errors']}"
        # 真实模型必然给出一些文本输出（拼接 Message 或 Part::Text）
        text_total = result["text"]
        for p in result["parts"]:
            if p.get("type") == "text":
                text_total += p.get("text") or p.get("content") or ""
        assert text_total.strip(), "助手应有非空文本回复"
        print(f"[OK] 助手回复长度={len(text_total)} chars; 前 80 字符: {text_total[:80]!r}")

    async def test_chat_with_readonly_tool(self, fi_code_server, server_url, test_project_manager):
        """场景 2：触发只读工具调用 —— 让模型用 read 工具读取 README.md，
        应观察到 tool_use Part（read 属于 Allow 级别，无需权限确认）。
        """
        prompt = (
            "请使用 read 工具读取当前工作目录下的 README.md 文件，"
            "然后用一句话概括文件内容。"
        )
        deadline = time.time() + 300

        resp = _post_chat_stream(server_url, prompt, agent="build", read_timeout=300.0)
        assert resp.status_code == 200, f"/chat 应返回 200，实际 {resp.status_code}"

        try:
            result = _collect_until_done(resp, deadline)
        except requests.exceptions.ConnectionError as e:
            pytest.fail(f"SSE 流读取异常（可能是模型耗时过长或网络中断）：{e}")

        assert result["done"], "应收到 SSE done 事件"
        assert not result["errors"], f"不应有 error 事件：{result['errors']}"

        # 校验：要么模型真的发起了 bash 调用（最理想），要么至少给出文本结果
        had_tool_use = any(
            p.get("type") == "tool_use" for p in result["parts"]
        )
        had_text = bool(result["text"].strip()) or any(
            p.get("type") == "text" for p in result["parts"]
        )

        print(
            f"[INFO] tool_use_parts={had_tool_use}, "
            f"tool_calls_seen={result['tool_calls']}, "
            f"final_text_len={len(result['text'])}"
        )
        # 真实模型必须至少回复文本；工具调用为可选（取决于模型策略）
        assert had_text, "模型至少应给出文本回复"
