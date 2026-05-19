# @ 文件引用（File Mention）设计文档

## 需求概述

在 TUI / Desktop / Web 三个前端中支持 `@` 文件引用：输入 `@` 弹出文件选择器，选择文件后，后端自动将文件内容注入到用户消息上下文中。

## 核心决策

- 触发方式：输入框中输入 `@` 字符立即弹出文件选择器
- 内容传递：前端只发送 `@path` 文本，后端在 `/chat` endpoint 中解析并读取文件
- 多文件：支持 `@file1 @file2 提示词`

## 架构

```
用户输入: "@src/main.rs 解释一下"

前端 (Web/TUI/Desktop)
  └── 输入 @ → 弹出文件树选择器
  └── 选择文件 → 输入框插入 "@src/main.rs 解释一下"
  └── POST /chat { message }

后端 chat_api::handle_chat_endpoint
  └── regex @(\S+) 提取路径列表
  └── safe_path + read_file 读取每个文件
  └── 构造注入消息并送入 agent_loop
```

## 后端改动

### `crates/core/src/server/api/chat_api.rs`

在 `handle_chat_endpoint` 中，消息送入 `agent_loop` 之前：

```rust
fn inject_file_contents(message: &str, workspace: &Path) -> String {
    let re = regex::Regex::new(r"@(\S+)").unwrap();
    let mut result = message.to_string();
    let mut injections = Vec::new();
    
    for cap in re.captures_iter(message) {
        let path_str = &cap[1];
        if let Ok(content) = read_file_content(workspace, path_str) {
            injections.push(format!("File: {}\n```\n{}\n```", path_str, content));
        }
    }
    
    if !injections.is_empty() {
        // 移除所有 @path 标记，在消息开头注入文件内容
        let cleaned = re.replace_all(message, "").trim().to_string();
        result = format!("{}\n\n{}", injections.join("\n\n"), cleaned);
    }
    
    result
}
```

文件读取限制：
- 通过 `safe_path` 检查防止目录遍历
- 单文件内容截断至 10000 字符
- 读取失败时记录日志但不中断流程

## 前端改动

### Web / Desktop（共享前端代码）

**文件：** `frontend/src/components/chat/InputBox.tsx`

新增 `@` 文件选择器：
- `@` 触发状态：`showFilePicker`
- 调用 `/api/files/tree` 获取当前目录文件树
- 复用 `FileTreeNode` 组件渲染目录结构
- 选择文件后，在输入框当前光标位置插入 `@file_path `
- `@` 和 `/` 菜单互斥

**键盘交互：**
- ↑↓ 在文件树中导航
- Enter 选择文件
- Esc 关闭文件选择器
- → 展开目录，← 折叠目录

### TUI

**文件：** `crates/tui/src/components/input.rs`

新增 `@` 模式：
- `file_picker_visible: bool`
- `file_picker_items: Vec<FileEntry>`
- `file_picker_selected: usize`
- 调用后端 `/api/files/tree`（通过 TuiClient）获取文件树
- ↑↓ 导航，→ 展开目录，Enter 选择，Esc 关闭

## 多文件注入格式

```
File: src/lib.rs
```
pub fn add(a: i32, b: i32) -> i32 { ... }
```

File: src/main.rs
```
fn main() { ... }
```

比较这两个文件
```

## 安全

- `safe_path` 检查确保文件在工作目录内
- 超大文件截断，防止 LLM 上下文溢出
- 不存在的文件：静默忽略（或注入 "File not found" 提示）
