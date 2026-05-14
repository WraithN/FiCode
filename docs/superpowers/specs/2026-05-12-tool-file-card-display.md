# 工具文件操作卡片展示设计

**目标：** 让 read/write/edit 工具的卡片在 TUI 中展示文件详情，提升可读性。

## 设计

### 1. 读文件（read 工具）
- Card 类型：`ToolResult`
- 标题：`read: <filename>`
- 内容：文件前 10 行
- `full_content`：完整文件内容
- `right_content`：`None`
- 状态：`Collapsed`（如果超过 10 行）/ `Completed`（如果 ≤10 行）
- 底部显示 `+Expand` 按钮，点击展开全部内容

### 2. 写文件（write 工具，新文件）
- Card 类型：`WriteFile { path }`
- 标题：`write: <filename>`
- 内容：左侧显示 `"(new file)"` 或空文件提示
- `right_content`：写入后的完整文件内容
- 左右分栏显示（60/40），右侧带绿色边框表示新增

### 3. 替换文件（edit 工具）
- Card 类型：`WriteFile { path }`
- 标题：`edit: <filename>`
- 内容：左侧显示 diff（删除行带 `-`，增加行带 `+`）
- `right_content`：`None`（diff 本身包含左右信息）
- 或：左侧显示原始内容，右侧显示编辑后内容

## 改动范围
- `crates/tui/src/components/chat.rs`：ToolResult 处理逻辑
- `crates/tui/src/components/card_widget.rs`：WriteFile 卡片渲染优化
