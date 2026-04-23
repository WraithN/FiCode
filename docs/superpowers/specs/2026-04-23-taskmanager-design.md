# TaskManager 设计文档

> 让 Agent 能够拆分复杂任务为若干子任务，交由 Subagent 执行，并将执行结果汇总返回。

## 背景

当前 `fi-code` 的 Agent 只有一个单一的对话循环。当用户提出复杂请求（如"重构整个错误处理模块"）时，主 Agent 需要在同一个上下文中完成所有工作，导致：
- 上下文膨胀，Token 消耗高
- 不同阶段的任务混杂在同一个对话中，容易互相干扰
- 无法针对不同子任务使用专门的提示词和工具集

TaskManager 通过引入**任务编排层**和**AgentRunner 抽象**，让主 Agent 可以判断任务复杂度、拆分子任务、由专门的 Subagent 执行、最后汇总结果。

## 目标

1. 主 Agent 能够判断任务是否复杂，并调用工具拆分为子任务
2. 向用户展示任务列表（名称 + 状态），并在任务进度变化时实时更新
3. 每个子任务由独立的 Subagent 执行，Subagent 拥有独立的提示词和工具集
4. Subagent 执行完成后返回 summary，所有 summary 汇总后交还给主 Agent
5. 对现有代码的侵入性最小化，主 Agent 的核心逻辑保持不变

## 非目标

- 不支持子任务间的显式依赖图（当前版本只支持串行执行）
- 不支持并行执行 Subagent（避免文件冲突）
- 子任务的历史记录不持久化到主 session 的 JSONL

## 架构设计

### 1. AgentRunner 抽象

将现有的 `agent_loop` / `run_one_turn` 抽象为可配置、可实例化的 `AgentRunner`。

```
src/agent/
├── mod.rs          # 导出 AgentRunner, AgentRunResult
├── agent.rs        # 现有的 run_one_turn 逻辑 → AgentRunner::run
├── runner.rs       # 新增：AgentRunner 结构体和实现
└── prompt.rs       # PromptBuilder（不变）
```

**`AgentRunner`** 是 Agent 循环的运行时实例：
- 持有 `Box<dyn AIClient>`、系统提示词、工具 schema
- `run(initial_messages)` 启动循环，返回完整对话历史
- 主 Agent 和 Subagent 使用同一个 `AgentRunner` 类型，但配置不同

### 2. 任务系统数据模型

```
src/task/
├── mod.rs          # 导出 Task, TaskStatus, TaskPlan, TaskManager
└── manager.rs      # TaskManager 实现
```

**核心类型：**

| 类型 | 说明 |
|------|------|
| `Task` | 单个任务：id、名称、描述、状态、结果、时间戳 |
| `TaskStatus` | `Pending` / `InProgress` / `Completed` / `Failed` |
| `TaskPlan` | 任务计划：任务列表 + 原始用户请求 |
| `TaskExecutionSummary` | 单个任务的执行结果摘要 |

### 3. TaskManager 编排器

**`TaskManager`** 负责任务计划的执行编排：

1. 接收 `TaskPlan`
2. 串行遍历每个 `Task`
3. 对每个任务：
   - 更新状态为 `InProgress`
   - 调用 `on_progress` 回调展示当前进度
   - 创建 `AgentRunner` 作为 Subagent 并运行
   - 收集结果，更新状态为 `Completed` 或 `Failed`
   - 再次调用 `on_progress` 回调
4. 返回所有任务的执行摘要

**串行执行的理由：**
- 编码任务子任务间通常有隐式依赖（先分析后修改）
- 避免多个 Subagent 同时读写同一文件导致冲突
- 后续可无痛扩展为并行

### 4. 工具集成

在主 Agent 的工具集中新增 **`create_task_plan`** 工具：

```json
{
  "name": "create_task_plan",
  "description": "将复杂任务拆分为多个子任务。仅在任务确实复杂、需要多步骤完成时调用。",
  "parameters": {
    "type": "object",
    "properties": {
      "tasks": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "name": { "type": "string" },
            "description": { "type": "string" }
          },
          "required": ["name", "description"]
        }
      }
    },
    "required": ["tasks"]
  }
}
```

**执行流程：**

```
用户输入
  ↓
主 Agent (AgentRunner) 判断需要拆分
  → ToolUse: create_task_plan
    ↓
ToolHandler 解析 JSON → TaskPlan
  → 向用户展示初始任务列表
  → 调用 TaskManager::execute_plan
    ↓
TaskManager 串行执行每个子任务
  → 每完成一个任务：更新状态 + 展示进度
    ↓
所有任务完成
  → ToolHandler 将汇总结果注入主 Agent 的消息上下文
    ↓
主 Agent 收到汇总结果，生成最终回复给用户
```

## 数据流

### 正常流程

1. **主 Agent 判断拆分**
   - 主 Agent 的 system prompt 中告知它："如果任务复杂，请调用 create_task_plan"
   - LLM 输出 `Part::ToolUse { name: "create_task_plan", arguments: {...} }`

2. **`create_task_plan` Handler**
   - 解析参数中的 `tasks` 数组，构建 `TaskPlan`
   - 向终端打印任务列表：
     ```
     📋 Task Plan (3 tasks):
     [Pending] 1. 分析现有错误处理
     [Pending] 2. 设计新的错误类型
     [Pending] 3. 实现重构
     ```
   - 调用 `TaskManager::execute_plan`

3. **TaskManager 执行**
   - 对每个任务，创建新的 `AgentRunner` 实例：
     ```rust
     let subagent = AgentRunner::new(
         client_factory(),           // 新的 client 实例
         SUBAGENT_PROMPT.to_string(), // 子任务专用提示词
         SUBAGENT_TOOLS_SCHEMA.clone(), // 子任务可用工具
     );
     ```
   - Subagent 的初始消息包含任务名称和描述
   - Subagent 运行完成后，从最后一条 Assistant 消息提取文本作为 summary

4. **进度展示**
   - `execute_plan` 的 `on_progress` 回调接收当前 `TaskPlan`
   - 回调向终端打印更新后的任务列表
   - 示例：
     ```
     ✅ [Completed] 1. 分析现有错误处理
     🔄 [InProgress] 2. 设计新的错误类型
     [Pending] 3. 实现重构
     ```

5. **结果汇总**
   - 所有任务执行完毕后，`create_task_plan` Handler 构建汇总消息：
     ```
     所有子任务已完成，结果汇总如下：
     
     [任务 1: 分析现有错误处理]
     {summary_1}
     
     [任务 2: 设计新的错误类型]
     {summary_2}
     
     [任务 3: 实现重构]
     {summary_3}
     ```
   - 将汇总消息以 `Message::User` 的形式插入主 Agent 的 `LoopState.messages`
   - 返回空字符串给 ToolResult（因为结果已通过消息注入）

6. **主 Agent 继续**
   - 主 Agent 的下一次 `run_one_turn` 会收到包含汇总结果的 User 消息
   - 主 Agent 基于汇总结果生成最终回复给用户

## 错误处理

| 场景 | 策略 |
|------|------|
| 单个子任务失败 | 记录 `Failed` 状态，继续执行后续任务。失败原因保存在 `Task.result` 中 |
| 所有子任务都失败 | 汇总时告知主 Agent"所有子任务均失败"，由主 Agent 决定重试或放弃 |
| `execute_plan` 本身失败 | 抛出 `anyhow::Error`，主 Agent 的 `run_one_turn` catch 后返回错误提示 |
| Subagent 达到 max_turns | 截断当前对话，将已有内容作为 summary，标记为 `Completed`（带警告） |

## 文件变更计划

### 新增文件

| 文件 | 说明 |
|------|------|
| `src/agent/runner.rs` | `AgentRunner` 和 `AgentRunResult` |
| `src/task/mod.rs` | 任务系统模块声明和类型导出 |
| `src/task/manager.rs` | `TaskManager` 实现 |

### 修改文件

| 文件 | 修改内容 |
|------|----------|
| `src/agent/mod.rs` | 导出 `AgentRunner`, `AgentRunResult` |
| `src/agent/agent.rs` | 将 `agent_loop` / `run_one_turn` 逻辑迁移到 `AgentRunner`；保留对现有调用方的兼容层 |
| `src/main.rs` | 导入 `task` 模块；在交互/单命令模式中集成 TaskManager |
| `src/tools/mod.rs` | 注册 `create_task_plan` 工具；提供 `TaskManager` 的全局访问或传入方式 |

## Subagent 配置

Subagent 的提示词和工具集需要专门设计：

**Subagent System Prompt 模板：**
```
你是一个专注于执行特定子任务的 AI 助手。
你的任务是完成用户交给你的具体任务，不要偏离主题。
完成后，请用一段话总结你做了什么、结果是什么。
```

**Subagent 工具集：**
- 默认包含所有基础工具（read, write, edit, bash, grep, web_fetch）
- 不包含 `create_task_plan`（避免递归拆分导致无限循环）
- 未来可扩展为根据任务类型动态选择工具集

## 测试策略

1. **单元测试：**
   - `TaskManager::execute_plan` 的串行执行逻辑
   - `Task` 状态流转（Pending → InProgress → Completed/Failed）
   - `AgentRunner::run` 的抽象正确性（使用 mock client）

2. **集成测试：**
   - `create_task_plan` 工具的 JSON 解析和 TaskPlan 构建
   - 端到端流程：主 Agent 调用 create_task_plan → TaskManager 执行 → 结果注入主上下文

## 未来扩展

- **并行执行：** 在 `TaskManager` 中引入依赖图，对无依赖的任务使用 `tokio::spawn` 并行执行
- **任务持久化：** 将 TaskPlan 保存到 session JSONL，支持中断后恢复子任务执行
- **动态工具集：** 根据子任务类型（分析/编码/测试）自动选择不同的 Subagent 工具集和提示词
- **人机协作：** 在执行关键子任务前询问用户确认
