# 读写文件信息流单读优化设计文档

> 优化 read / write / edit 工具的结果展示，支持语法高亮、diff 着色，不再使用 JSON 格式展示。

---

## 1. 背景与目标

### 1.1 当前问题

- `read` 工具返回纯文本文件内容，TUI/Web 均作为原始文本直接展示，无语法高亮
- `write`/`edit` 工具返回 JSON 字符串（含 `diff`、`after_content`、`is_new_file`），TUI 尝试解析 JSON 提取字段，Web 直接展示原始 JSON
- 用户体验差：代码无高亮、diff 不直观、JSON 格式不友好

### 1.2 设计目标

- **语法高亮**：`read` 返回的文件内容按语言类型高亮展示
- **diff 着色**：`write`/`edit` 返回的 diff 按原文件语言高亮，且 `+`/`−` 行额外着色
- **无 JSON 展示**：不再以 JSON 格式向用户展示工具结果
- **元数据保留**：保留工具执行时长、文件路径等元信息
- **控制字符正确展示**：换行、制表符等空白字符保持原始结构，不折叠、不转义

---

## 2. 设计方案

### 2.1 核心思路：多 Part 序列

将单个工具调用的结果从"一个 `Part::ToolResult`"转变为**多个平铺 Part 的序列**：

```
Part::ToolResult  { content: "✓ read: src/main.rs (120ms)", ... }
Part::CodeBlock   { language: "rust", content: "fn main() {\n    println!(...);\n}" }
```

前后端按顺序渲染，先展示元数据标题，再展示带语法高亮的代码内容。

### 2.2 为什么选择多 Part 序列

- **改动面最小**：不需要修改 `Part` 枚举结构（如添加嵌套 children）
- **符合 SSE 模型**：每个 Part 可以独立通过 SSE 流推送，前端逐条接收渲染
- **复用现有基础设施**：`Part::CodeBlock` 的语法高亮在 Web（`CodeBlockPart.tsx`）和 TUI（`code_block.rs`）均已实现，只需扩展 diff 着色

### 2.3 后端改动

#### 2.3.1 `execute_single_tool_call` 返回类型变更

从返回 `Part` 变更为返回 `Vec<Part>`：

```rust
// 变更前
pub async fn execute_single_tool_call(tool_call: &ToolCall) -> Result<Part, String>

// 变更后
pub async fn execute_single_tool_call(tool_call: &ToolCall) -> Result<Vec<Part>, String>
```

#### 2.3.2 各工具返回的 Part 序列

**`read` 工具：**

```rust
vec![
    Part::ToolResult {
        content: format!("✓ read: {} ({}ms)", path, duration_ms),
        tool_name: "read".to_string(),
        duration_ms,
        success: true,
    },
    Part::CodeBlock {
        language: file_type_from_path(path), // e.g., Some("rust")
        content: file_content,
        title: Some(path.to_string()),
    },
]
```

**`write` 工具：**

```rust
vec![
    Part::ToolResult {
        content: format!("✓ write: {} ({}ms){}", path, duration_ms,
            if is_new_file { " — 新增文件" } else { "" }),
        tool_name: "write".to_string(),
        duration_ms,
        success: true,
    },
    Part::CodeBlock {
        language: file_type_from_path(path),
        content: diff_text,  // diff 文本，非 JSON
        title: Some(format!("diff: {}", path)),
    },
]
```

**`edit` 工具：**

```rust
vec![
    Part::ToolResult {
        content: format!("✓ edit: {} ({}ms)", path, duration_ms),
        tool_name: "edit".to_string(),
        duration_ms,
        success: true,
    },
    Part::CodeBlock {
        language: file_type_from_path(path),
        content: diff_text,  // diff 文本，非 JSON
        title: Some(format!("diff: {}", path)),
    },
]
```

> **注意**：`write`/`edit` 不再返回 JSON。diff 文本直接作为 `CodeBlock` 的 `content`，元数据（是否新文件、文件大小）融入 `ToolResult` 的标题文本中。

#### 2.3.3 新增辅助函数

```rust
/// 从文件路径推断语言标识符
/// 
/// 示例：
/// - "src/main.rs" → Some("rust")
/// - "app.tsx" → Some("typescript")
/// - "Makefile" → Some("makefile")
/// - "无扩展名/未知" → None
pub fn file_type_from_path(path: &str) -> Option<String> {
    // 基于扩展名的映射表
}
```

语言映射表（常用语言）：

| 扩展名 | 语言标识 |
|--------|----------|
| `.rs` | `rust` |
| `.ts`, `.tsx` | `typescript` |
| `.js`, `.jsx` | `javascript` |
| `.py` | `python` |
| `.go` | `go` |
| `.java` | `java` |
| `.c`, `.h` | `c` |
| `.cpp`, `.hpp`, `.cc` | `cpp` |
| `.md` | `markdown` |
| `.json` | `json` |
| `.yaml`, `.yml` | `yaml` |
| `.toml` | `toml` |
| `.sh` | `bash` |
| `.html`, `.htm` | `html` |
| `.css` | `css` |
| `.sql` | `sql` |

### 2.4 前端/Web 改动

#### 2.4.1 `ToolResultPart` 组件

保持现状，继续显示元数据标题行。但调整样式使其更轻量：

```tsx
<div className="my-1 px-2 py-1 rounded bg-bg-secondary/50 border-l-2 border-success">
  <span className="text-xs text-success font-mono">
    ✓ Result ({duration_ms}ms)
  </span>
</div>
```

#### 2.4.2 `CodeBlockPart` 组件增强

**新增 diff 行着色功能**：

```tsx
// 在渲染代码行时，检测行首 +/−
const renderLine = (line: string, index: number) => {
  if (line.startsWith('+')) {
    return <span key={index} className="bg-green-900/30 text-green-400">{line}</span>;
  }
  if (line.startsWith('-')) {
    return <span key={index} className="bg-red-900/30 text-red-400">{line}</span>;
  }
  return <span key={index}>{line}</span>;
};
```

**控制字符正确处理**：

- 外层容器使用 `<pre className="whitespace-pre">`，确保换行和空格不被折叠
- 设置 `tab-size: 4`（或 `style={{ tabSize: 4 }}`），确保制表符显示为 4 空格宽
- 行尾空格通过 CSS `white-space: pre` 保持可见

#### 2.4.3 语法高亮

复用现有的 `CodeBlockPart` 语法高亮逻辑：
- 若 `language` 字段为 `Some(lang)`，使用对应语言的高亮规则
- 若 `language` 为 `None`，使用默认文本高亮（无特殊着色）
- diff 内容使用原文件语言高亮（如 `language: "rust"`），diff 前缀 `+`/`-` 的着色覆盖在语法高亮之上

### 2.5 TUI 改动

#### 2.5.1 `ToolResultRenderer`

保持现状，显示元数据标题行。

#### 2.5.2 `CodeBlockRenderer` 增强

**新增 diff 行着色**：

```rust
fn render_code_line(line: &str) -> Line {
    if line.starts_with('+') {
        Line::from(Span::styled(line, Style::default().fg(Color::Green)))
    } else if line.starts_with('-') {
        Line::from(Span::styled(line, Style::default().fg(Color::Red)))
    } else {
        // 原有语法高亮逻辑
        syntax_highlight_line(line)
    }
}
```

**控制字符处理**：

- `	`（制表符）替换为 4 个空格，确保对齐
- `
` 作为换行符处理，不显示为字面量
- ``（回车）直接丢弃或视为换行（统一按 `
` 处理）

### 2.6 数据流示意

```
用户请求: "请读取 src/main.rs"
        ↓
Agent 调用 read 工具
        ↓
run_read("src/main.rs") 
  → 读取文件内容
  → file_type_from_path("src/main.rs") = Some("rust")
        ↓
返回 Vec<Part>:
  [ ToolResult { content: "✓ read: src/main.rs (45ms)" },
    CodeBlock { language: Some("rust"), content: "fn main() {...}" } ]
        ↓
SSE 流式推送（2 条事件）
        ↓
前端接收并渲染:
  1. 显示绿色标题: "✓ read: src/main.rs (45ms)"
  2. 显示带 Rust 语法高亮的代码块
```

---

## 3. 边界情况处理

### 3.1 大文件截断

若文件内容超过压缩阈值（`read` 为 100KB），`compress_tool_result` 会在内容层面截断。截断后的内容仍作为 `CodeBlock` 发送，但会在顶部添加截断提示（如 `// ... 内容已截断，显示前 1000 行和后 2000 行 ...`）。

### 3.2 二进制文件

若 `read` 读取的是二进制文件（如图片、可执行文件），不生成 `CodeBlock`，仅返回 `ToolResult` 提示"二进制文件，无法展示内容"。

### 3.3 无扩展名文件

`file_type_from_path` 返回 `None`，`CodeBlock` 使用默认文本高亮（无特殊语法着色）。

### 3.4 diff 格式中的特殊标记

diff 标准格式中的以下标记保持原样显示：
- `--- a/xxx` / `+++ b/xxx`（文件头）
- `@@ -1,3 +1,4 @@`（hunk 头）
- `\ No newline at end of file`（无换行提示）

这些标记不会被误识别为 `+`/`-` 行（因为它们不以 `+`/`-` 后接内容开头）。

### 3.5 控制字符展示

- **换行符 `\n`**：正确换行，不在界面显示 `\n` 字符
- **制表符 `\t`**：Web 通过 CSS `tab-size: 4` 渲染；TUI 替换为 4 个空格
- **回车符 `\r`**：丢弃或统一视为换行
- **其他控制字符**（如 `\0`）：Web 显示为 `␀` 等可见符号；TUI 显示为 `^@` 或替换为空格

---

## 4. 测试策略

### 4.1 单元测试

- `file_type_from_path` 函数：覆盖各种扩展名和无扩展名情况
- `execute_single_tool_call` 返回 `Vec<Part>` 的结构验证

### 4.2 BDD/E2E 测试

- 验证 `read` 工具产生 2 个 parts（ToolResult + CodeBlock）
- 验证 `write`/`edit` 工具产生 2 个 parts，且内容非 JSON
- 验证 CodeBlock 的 `language` 字段正确

### 4.3 视觉测试

- 验证 diff 行着色（+ 绿色，− 红色）
- 验证语法高亮正确（Rust/TypeScript 等）
- 验证制表符和换行符展示正确

---

## 5. 实施范围

### 5.1 涉及文件

**后端：**
- `crates/core/src/tools/basic_tools.rs` — 修改 `run_read`/`run_write`/`run_edit` 返回 `Vec<Part>`
- `crates/core/src/tools/tools_registry.rs` — 调整 `execute_single_tool_call` 调用处
- `crates/core/src/agent/agent.rs` 或 `runner.rs` — 处理 `Vec<Part>` 的 SSE 推送
- 新增语言映射函数（可放在 `crates/core/src/utils/` 或 `basic_tools.rs` 中）

**Web 前端：**
- `frontend/src/components/part-renderers/ToolResultPart.tsx` — 样式微调
- `frontend/src/components/part-renderers/CodeBlockPart.tsx` — 新增 diff 着色、控制字符处理

**TUI 前端：**
- `crates/tui/src/components/part_renderer/code_block.rs` — 新增 diff 着色、控制字符处理

### 5.2 不修改的文件

- `Part` 枚举定义（`crates/shared/src/dto.rs`）—— 不需要修改，复用现有 `ToolResult` 和 `CodeBlock`
- SSE 协议格式 —— 复用现有的 Part 序列化/反序列化

---

## 6. 成功标准

- [ ] `read` 工具结果展示带语法高亮的代码块，无原始文本展示
- [ ] `write`/`edit` 工具结果展示带 diff 着色的代码块，不展示 JSON
- [ ] `ToolResult` 标题行保留工具名、文件路径、执行时长
- [ ] 换行、制表符等控制字符保持原始结构
- [ ] 所有现有测试通过
- [ ] 新增单元测试覆盖 `file_type_from_path` 和 Part 序列结构

---

*设计日期：2026-05-18*
