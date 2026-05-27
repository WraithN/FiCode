# TTFT（Time To First Token）可观测性设计文档

## 背景

TTFT 是衡量 LLM 交互体验的核心指标，定义为：从用户发送请求到界面上渲染出第一个字符的总耗时。为了定位延迟瓶颈（网络？服务端处理？LLM API？），需要在全链路关键节点埋点。

## 埋点设计

### 1. 前端侧（Browser）

| 埋点 | 位置 | 说明 |
|------|------|------|
| `T0_request_sent` | `useChatStream.ts` `send()` | 用户点击发送按钮，前端调用 `fetch()` 的瞬间 |
| `T1_first_sse_received` | `useChatStream.ts` 事件循环 | 前端 `reader.read()` 首次收到 SSE 数据 |

**前端计算公式：**
- 前端总 TTFT = `T1 - T0`（从点击发送到收到第一个 SSE 事件）

### 2. 服务端侧（Rust HTTP Server）

| 埋点 | 位置 | 说明 |
|------|------|------|
| `T2_request_received` | `chat_api.rs` `handle_chat_endpoint` | Axum 收到 HTTP POST /chat 请求 |
| `T3_llm_call_started` | `agent.rs` `run_one_turn` | 调用 `client.stream_message()` 前 |
| `T4_http_response_received` | `openapi_client.rs` `stream_message` | `send_with_retry()` 返回 HTTP 200，收到 Response Headers |
| `T5_first_sse_parsed` | `openapi_client.rs` `parse_openai_sse` | 解析出第一个包含 `choices` 的 SSE data 行 |
| `T6_first_chunk_from_llm` | `agent.rs` `on_chunk` 回调 | 收到第一个 `Text` / `Think` / `ToolUse` `ChunkContent` |
| `T7_first_sse_sent` | `chat_api.rs` `on_text` 回调 | 服务端通过 SSE channel 向前端发送第一个 `SseEvent::Message` |

**服务端计算公式：**
- 服务端总耗时 = `T7 - T2`（从收到请求到发送第一个 SSE）
- 请求预处理耗时 = `T3 - T2`（认证、session 加载、prompt 构建）
- LLM 网络 RTT = `T4 - T3`（从发起 HTTP 请求到收到 Response Headers）
- LLM 首 token 生成耗时 = `T5 - T4`（从收到 Headers 到第一个 SSE data 行）
- LLM 解析到首 chunk 耗时 = `T6 - T3`（从调用 stream_message 到第一个有效 chunk）
- SSE 推送耗时 = `T7 - T6`（从收到 chunk 到推送给前端）

### 3. 端到端（E2E）

- **端到端 TTFT** ≈ 前端 `T1 - T0`
- **服务端内部 TTFT** = `T6 - T3`
- **网络传输（前端→服务端）** ≈ `T2 - T0`（难以精确测量，受 NTP 影响）
- **网络传输（服务端→前端）** ≈ `T1 - T7`（同上）

## 关键代码位置

### frontend/src/hooks/useChatStream.ts
```typescript
const requestSentAt = performance.now();
// ...
for await (const event of stream) {
  if (firstSseAt === null) {
    firstSseAt = performance.now();
    const ttft = Math.round(firstSseAt - requestSentAt);
    console.log(`[TTFT] first SSE received | total=${ttft}ms`);
  }
}
```

### crates/core/src/server/api/chat_api.rs
```rust
let request_received_at = std::time::Instant::now();
// ...
let session_id_for_ttft = session_id.clone();
let mut on_text: Option<Box<dyn FnMut(&str) + Send>> = Some(Box::new(move |text: &str| {
    if !first_text_sent.swap(true, std::sync::atomic::Ordering::SeqCst) {
        let elapsed_ms = request_received_at.elapsed().as_millis() as u64;
        log_info!("[TTFT] first token SSE sent | total={}ms | session_id={}", elapsed_ms, session_id_for_ttft);
    }
    // ...
}));
```

### crates/core/src/agent/agent.rs
```rust
let llm_call_start = std::time::Instant::now();
let mut first_chunk_received = false;
client.stream_message(&system_prompt, &llm_messages, &schema, &mut |chunk| {
    if !first_chunk_received {
        match &chunk.content {
            ChunkContent::Text(_) | ChunkContent::Think(_) | ChunkContent::ToolUse(_) => {
                first_chunk_received = true;
                let elapsed_ms = llm_call_start.elapsed().as_millis() as u64;
                log_info!("[TTFT] first chunk from LLM | latency={}ms | turn={}", elapsed_ms, state.turn_count);
            }
            _ => {}
        }
    }
    // ...
})
```

### crates/core/src/provider/client/openapi_client.rs
```rust
let http_req_start = std::time::Instant::now();
let resp = send_with_retry(&self.client, request, &self.retry_config, Some(notifier)).await?;
let http_resp_elapsed_ms = http_req_start.elapsed().as_millis() as u64;
log_info!("[TTFT] HTTP response received | latency={}ms | url={}", http_resp_elapsed_ms, url);
// ...
// parse_openai_sse 中：
if !first_sse_parsed {
    first_sse_parsed = true;
    let elapsed_ms = http_req_start.elapsed().as_millis() as u64;
    log_info!("[TTFT] first SSE line parsed | latency={}ms", elapsed_ms);
}
```

## 日志输出示例

当用户发送一条消息时，日志会按顺序输出：

```
[Server] handle_chat_endpoint | session_id=01KSK... | message_len=42
[Server] run_agent_chat start | session_id=01KSK... | message_len=42 | agent=Build
[Server] agent_loop starting | messages=3
[TTFT] HTTP response received | latency=245ms | url=https://api.example.com/v1/chat/completions
[TTFT] first SSE line parsed | latency=312ms
[TTFT] first chunk from LLM | latency=315ms | turn=1
[TTFT] first token SSE sent | total=412ms | session_id=01KSK...
```

前端 Console：
```
[TTFT] first SSE received | total=518ms
```

## 延迟拆解示例

| 阶段 | 耗时 | 说明 |
|------|------|------|
| 前端→服务端网络 | ~50ms | `T2 - T0` 估算 |
| 服务端预处理 | ~97ms | `T3 - T2`（session 加载、prompt 构建） |
| LLM HTTP RTT | ~245ms | `T4 - T3`（TCP + TLS + 请求传输） |
| LLM 首 token 生成 | ~67ms | `T5 - T4`（模型推理到第一个 token） |
| 服务端处理+SSE推送 | ~97ms | `T7 - T6`（chunk 处理、SSE 序列化、channel 发送） |
| 服务端→前端网络 | ~106ms | `T1 - T7` 估算 |
| **端到端 TTFT** | **~518ms** | 前端 `T1 - T0` |

## 注意事项

1. **前端与服务端时钟不同步**：`T0/T1`（前端 `performance.now()`）与 `T2~T7`（服务端 `Instant::now()`）不在同一时钟域，不能直接相减计算网络耗时。如需精确网络耗时，需要服务端在响应头中返回 `T2` 时间戳。
2. **Anthropic 客户端**：当前 TTFT 埋点仅在 `openapi_client.rs` 中实现。`anthropic_client.rs` 如需相同能力，需在对应位置添加相同逻辑。
3. **日志级别**：所有 TTFT 日志使用 `log_info!`，在生产环境中会被正常收集。
4. ** overhead**：`Instant::now()` 和 `AtomicBool::swap` 的开销极低（纳秒级），对 TTFT 本身的影响可以忽略。
