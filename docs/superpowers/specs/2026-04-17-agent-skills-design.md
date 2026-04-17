# Agent Skills 加载能力设计文档

> 日期：2026-04-17
> 状态：待实现

---

## 1. 背景与目标

shun-code 是一个基于 Rust 的终端 AI Coding Agent CLI。当前 Agent 的 system prompt 只包含固定规则和工具描述，缺乏可扩展的领域知识注入机制。

本设计为 shun-code 引入 **Agent Skills 加载能力**，允许用户通过声明式 Markdown 文件（SKILL.md）扩展 Agent 在特定场景下的行为（如代码审查、Git 提交、测试编写等）。Skills 支持按需加载，仅在当前对话回合生效，避免 system prompt 膨胀。

---

## 2. 术语

| 术语 | 说明 |
|------|------|
| Skill | 一个包含 `SKILL.md` 的目录，声明 Agent 在特定场景下的行为指南 |
| Registry | Skill 的元数据索引，包含 id、name、description、source、软链路径等 |
| Scope | Skill 的来源标识，如 `claude`、`shun-code`、`myproject` |
| Skill ID | 唯一标识，`{scope}-{name}`，如 `claude-commit` |

---

## 3. Skill 文件格式

### 3.1 目录结构

```
my-skill/
├── SKILL.md              # 核心文件（必需）
├── REFERENCE.md          # 补充参考资料（可选）
├── examples/             # 示例文件（可选）
│   ├── sample-input.md
│   └── sample-output.md
└── scripts/              # 可执行脚本（可选）
    ├── validate.py
    └── helper.js
```

### 3.2 SKILL.md 格式

顶部为 YAML front matter，下方为 Markdown 正文：

```markdown
---
name: commit
description: Create conventional commits with proper formatting and emoji prefixes
---

## Context
Current git status: !`git status`

## Steps
1. Analyze staged changes
2. Choose type (feat/fix/docs)
3. Format: `emoji type(scope): description`
4. Execute commit
```

---

## 4. 架构设计

### 4.1 模块划分

```
src/
├── skills/
│   ├── mod.rs              # 模块聚合与公共导出
│   ├── scanner.rs          # Skill 扫描与 Registry 构建
│   ├── registry.rs         # Registry 结构、持久化与查询
│   ├── loader.rs           # Skill 内容读取与拼接
│   └── skill_type.rs       # Skill、SkillMetadata、SkillSource 等类型
```

### 4.2 核心数据类型

```rust
/// Skill 元数据（从 SKILL.md 的 YAML front matter 解析）
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// Skill 来源类型
pub enum SkillSourceType {
    Project,    // <workspace>/.skills/
    Global,     // ~/.config/shun-code/skills/
    Agent,      // ~/.config/agent/skills/
    Claude,     // ~/.claude/skills/
}

/// Registry 中的单个 Skill 条目
pub struct SkillEntry {
    pub id: String,            // "{scope}-{name}"
    pub scope: String,         // 来源标识
    pub source_type: SkillSourceType,
    pub symlink_path: PathBuf, // ~/.cache/shun-code/skills/{id}
    pub target_path: PathBuf,  // 原始目录
    pub metadata: SkillMetadata,
}

/// 完整 Registry
pub struct SkillRegistry {
    pub version: String,
    pub entries: Vec<SkillEntry>,
}
```

---

## 5. 扫描与注册流程

### 5.1 扫描来源（按优先级，后加载覆盖前者）

| 优先级 | 来源路径 | Scope 前缀 | 类型 |
|--------|----------|------------|------|
| 1 | `<workspace>/.skills/` | `{workspace_basename}` | Project |
| 2 | `~/.config/shun-code/skills/` | `shun-code` | Global |
| 3 | `~/.config/agent/skills/` | `agent` | Agent |
| 4 | `~/.claude/skills/` | `claude` | Claude |

### 5.2 扫描规则

1. 遍历每个来源目录下的**直接子目录**
2. 检查子目录中是否存在 `SKILL.md`
3. 解析 YAML front matter，提取 `name` 和 `description`
4. 若 `name` 缺失或为空，跳过该目录并输出 warning
5. 生成 `id = "{scope}-{name}"`
6. 创建软链：`~/.cache/shun-code/skills/{id} -> {原始目录}`
7. 将条目写入 Registry

### 5.3 隔离规则

- **项目级 Skill（`.skills/`）**：仅在工作目录为该项目目录时扫描加载
- **全局 Skill（`~/.config/...`、`~/.claude/...`）**：任何工作目录下都加载
- 启动时通过 `workspace_dir()` 判断当前是否在某个项目目录内，若不是，则跳过项目级 Skill

### 5.4 持久化格式

`~/.config/shun-code/registry-skills.json`：

```json
{
  "version": "1.0",
  "sources": [
    {
      "id": "claude-commit",
      "type": "symlink",
      "path": "~/.cache/shun-code/skills/claude-commit",
      "target": "/home/user/.claude/skills/commit",
      "metadata": {
        "name": "commit",
        "description": "Create conventional commits",
        "tags": ["git"]
      }
    }
  ]
}
```

### 5.5 边界处理

- **无效 skill 目录**：无 `SKILL.md` 或 YAML 解析失败 → 跳过并 warning
- **原始目录被删除**：下次启动扫描时检测软链失效，自动清理 registry 条目和软链
- **同名 skill 冲突**：后加载的来源（更高优先级序号）覆盖前者
- **软链目标变更**：删除旧软链，重新创建并更新 registry

---

## 6. System Prompt 集成

### 6.1 注入位置

在现有 system prompt 末尾追加 **"Available Skills"** 段：

```
## Available Skills
You can load any of the following skills on-demand by calling the `use_skill` tool:

- `commit` (claude): Create conventional commits with proper formatting and emoji prefixes
- `code-review` (myproject): Perform structured code review with checklists
```

### 6.2 注入内容

仅包含 **name + scope + description**，不包含完整 skill 内容。保持 system prompt 精简。

### 6.3 无 Skill 场景

若 Registry 为空，则不追加 Available Skills 段。

---

## 7. Skill 加载与消息注入

### 7.1 `use_skill` 工具

新增内置工具，注册到 `ToolsRegistry`：

```json
{
  "name": "use_skill",
  "description": "Load a skill by name or ID to inject its instructions into the conversation. The skill content will be appended as a context message for the current turn only.",
  "parameters": {
    "type": "object",
    "properties": {
      "name": {
        "type": "string",
        "description": "Skill name or full ID (e.g., 'commit' or 'claude-commit')"
      }
    },
    "required": ["name"]
  }
}
```

### 7.2 加载流程

当 LLM 调用 `use_skill` 时：

1. **查找 Skill**：通过 `name` 或 `id` 在 Registry 中匹配
   - 先精确匹配 id（如 `claude-commit`）
   - 再按 name 匹配，若多个 scope 同名，返回最近加载的那个（优先级高的）
2. **读取内容**：从软链目录读取以下文件：
   - `SKILL.md`：YAML front matter + Markdown 正文（正文部分保留）
   - `REFERENCE.md`（可选）：直接追加
   - `examples/*.md`（可选）：按文件名排序后全部追加
3. **拼接格式**：
   ````markdown
   <skill name="commit" id="claude-commit">
   ## Context
   Current git status: !`git status`

   ## Steps
   1. Analyze staged changes
   ...

   --- Reference ---
   ...REFERENCE.md content...

   --- Examples ---
   ...examples/*.md content...
   </skill>
   ````
4. **作为 ToolResult 返回**：将上述文本作为 `use_skill` 工具调用的结果返回给 LLM

### 7.3 "只影响当前回合"的实现

Skill 内容通过 **ToolResult** 注入对话上下文，其生命周期如下：

- **当前 agent_loop 周期内**：LLM 在后续轮次中能看到 skill 内容（因为它已存在于 messages 历史中）
- **system prompt 中不永久驻留**：下次用户发新消息时，不会自动重新加载 skill
- **不修改 Session 持久化**：skill 内容作为普通 tool result 随 messages 一起被保存到 JSONL，但这属于正常的对话历史，而非"永久激活"

---

## 8. 与现有系统的集成

### 8.1 启动时序（main.rs）

在现有启动流程中插入 Skill 初始化：

```rust
// 1. 设置 workspace（已有）
set_workspace(workspace.clone());

// 2. 初始化 Skill Registry（新增）
let skill_registry = SkillRegistry::load_or_scan(&workspace)?;

// 3. 初始化工具注册表（已有）
init_tools();

// 4. 注册 use_skill 工具（新增 - 在 init_tools 闭包中完成）
```

### 8.2 PromptBuilder 修改

`PromptBuilder::build()` 增加 `SkillRegistry` 参数：

```rust
pub fn build(&self, tools_schema: &serde_json::Value, registry: &SkillRegistry) -> String {
    let mut prompt = // 现有模板 + tools_schema
    
    if !registry.entries.is_empty() {
        prompt.push_str("\n\n## Available Skills\n");
        prompt.push_str("You can load any of the following skills on-demand by calling the `use_skill` tool:\n\n");
        for entry in &registry.entries {
            prompt.push_str(&format!("- `{}` ({}): {}\n",
                entry.metadata.name,
                entry.scope,
                entry.metadata.description
            ));
        }
    }
    
    prompt
}
```

### 8.3 agent_loop 修改

`run_one_turn` 接收 `SkillRegistry` 引用，以便 `use_skill` 工具在执行时能查找 skill：

```rust
pub async fn run_one_turn<C: AIClient + ?Sized>(
    client: &C,
    state: &mut LoopState,
    registry: &SkillRegistry,
) -> Result<bool> {
    let system_prompt = PromptBuilder::new().build(&tool_schema(), registry);
    // ...
}
```

### 8.4 工具注册表扩展

在 `src/tools/mod.rs` 的 `REGISTRY` 初始化中新增 `use_skill` 工具：

```rust
registry.register(
    "use_skill",
    "Load a skill by name or ID to inject its instructions into the conversation.",
    r#"{"type":"object","properties":{"name":{"type":"string","description":"Skill name or full ID"}},"required":["name"]}"#,
    Box::new(UseSkillHandler { registry: ... }),
)
```

---

## 9. 错误处理

| 场景 | 行为 |
|------|------|
| Skill 目录无 `SKILL.md` | 跳过，启动时 warning |
| YAML front matter 解析失败 | 跳过，启动时 warning |
| `SKILL.md` 缺少 `name` 字段 | 跳过，启动时 warning |
| `use_skill` 传入不存在的 name | ToolResult 返回 `"Error: Skill '{name}' not found in registry."` |
| 软链目标目录被删除 | 下次启动扫描时清理，本次运行内查找返回错误 |
| Skill 文件读取失败 | ToolResult 返回 `"Error: Failed to read skill content: {e}"` |

---

## 10. 测试策略

### 10.1 单元测试（`src/skills/`）

- `scanner::tests`：
  - 扫描含有效/无效 skill 的目录，验证 registry 条目数
  - 验证同名 skill 的覆盖优先级
  - 验证项目级 skill 的隔离（非项目目录下不加载 `.skills/`）
- `registry::tests`：
  - Registry 序列化/反序列化
  - 按 name 和 id 查找 skill
- `loader::tests`：
  - 读取 SKILL.md 并正确分离 YAML front matter 和 Markdown 正文
  - 拼接 REFERENCE.md 和 examples

### 10.2 集成测试

- 在临时目录构造 skill，验证完整流程：扫描 → 注册 → use_skill 调用 → 内容注入

---

## 11. 文件清单

| 文件 | 说明 |
|------|------|
| `src/skills/mod.rs` | 模块声明与公共导出 |
| `src/skills/scanner.rs` | 目录扫描、YAML 解析、软链创建 |
| `src/skills/registry.rs` | Registry 结构、序列化、反序列化、查询 |
| `src/skills/loader.rs` | Skill 内容读取与拼接 |
| `src/skills/skill_type.rs` | 核心数据类型定义 |
| `src/agent/prompt.rs` | 修改：追加 Available Skills 段 |
| `src/agent/agent.rs` | 修改：传递 SkillRegistry 到 run_one_turn |
| `src/tools/mod.rs` | 修改：注册 use_skill 工具 |
| `src/main.rs` | 修改：启动时初始化 SkillRegistry |

---

## 12. 依赖评估

新增依赖需求：
- `yaml-rust` 或 `serde_yaml`：解析 SKILL.md 的 YAML front matter

现有依赖可复用：
- `serde` / `serde_json`：Registry 持久化
- `regex`：YAML front matter 边界匹配（或用字符串分割）
- `anyhow`：错误传播

---

## 13. 未来扩展（非本次范围）

- Skill 热重载：运行中检测 skill 目录变化并刷新 registry
- Skill 依赖：SKILL.md 中声明 `depends_on`，自动级联加载
- Skill 版本管理：支持多版本并存
