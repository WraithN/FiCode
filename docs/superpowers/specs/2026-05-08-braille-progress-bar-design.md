# Braille Progress Bar Integration Design

**Date**: 2026-05-08  
**Author**: AI Assistant  
**Status**: Design phase

## 1. 概述

将 fi-code TUI 应用中原有的 "ready" 文本状态显示替换为基于盲文（Braille）的进度条，使用 `ratatui-braille-bar` 库实现，并完善状态更新机制。

## 2. 目标

- 用更直观的进度条替换纯文本状态显示
- 在不同状态（Ready/Generating/Streaming）下显示不同的进度条动画
- 完善状态更新逻辑，确保进度条能真实反映应用当前状态
- 保持现有架构不变，只做必要的增强

## 3. 技术方案

### 3.1 依赖

添加新依赖到 `Cargo.toml`：
```toml
ratatui-braille-bar = "0.2.2"
```

### 3.2 组件修改

#### Header 组件 (`src/tui/components/header.rs`)

**新增字段**：
- `progress_tick: u64` - 用于动画计数的 ticker

**修改方法**：
1. `new()` - 初始化 `progress_tick = 0`
2. `on_tick()` - 增加 tick 计数，用于驱动动画
3. `draw()` - 用 `BrailleProgressBar` 替换原有的状态文本显示

**显示逻辑**：
- **Ready**：100% 进度（满格），绿色，显示 "Ready"
- **Generating**：循环动画（0→100%），黄色，显示 "Generating..."
- **Streaming**：循环动画（0→100%），品牌色，显示 "Streaming..."

#### App 主循环 (`src/tui/app.rs`)

在 `handle_app_event()` 中添加状态更新调用：
- `AppEvent::SubmitMessage` → `header.set_status(HeaderStatus::Generating)`
- `AppEvent::SseEvent(SseEvent::Content { ... })` → `header.set_status(HeaderStatus::Streaming)`
- `AppEvent::ChatComplete` | `AppEvent::StopGeneration` → `header.set_status(HeaderStatus::Ready)`

## 4. 数据流程

```
用户输入消息
    ↓
AppEvent::SubmitMessage
    ↓
header.set_status(Generating)
    ↓
SSE 流开始，Content 事件到达
    ↓
AppEvent::SseEvent::Content
    ↓
header.set_status(Streaming)
    ↓
流完成或用户停止
    ↓
AppEvent::ChatComplete / StopGeneration
    ↓
header.set_status(Ready)
```

## 5. 视觉设计

| 状态 | 进度值 | 颜色 | 文本 |
|------|--------|------|------|
| Ready | 100% | theme.success | "Ready" |
| Generating | 动画 (0→100% 循环) | theme.warning | "Generating..." |
| Streaming | 动画 (0→100% 循环) | theme.brand | "Streaming..." |

## 6. 错误处理

- 保持现有的错误处理机制不变
- 无论出现什么错误，最终都会回退到 Ready 状态
- 进度条动画不会因为错误而卡死

## 7. 测试策略

- 保持 `header.rs` 中现有的测试用例继续有效
- 验证 `set_status()` 方法正确工作
- 不添加复杂的 UI 测试（视觉效果通过手动验证）

## 8. 范围边界

- **在范围内**：Header 组件状态显示替换、App 状态同步、添加依赖
- **超出范围**：重构其他组件、修改非相关功能、添加新的状态类型
