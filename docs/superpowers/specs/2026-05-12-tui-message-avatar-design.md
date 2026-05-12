# TUI 消息头图标化设计

**目标：** 将聊天消息头中的纯文本 "You" / "◆ AI" 替换为带图标的彩色标识，提升视觉辨识度。

**范围：** 仅修改 `crates/core/src/tui/components/chat.rs` 的消息前缀渲染逻辑，不涉及动画、布局高度变更或其他模块。

## 设计要点

### 图标选择
- **User：** 橙色圆点 `●` + "You"
- **Assistant：** 青色菱形 `◆` + "FiCodeAgent"（替代原来的 "AI"）

### 颜色
- User 图标沿用现有 `theme.style_user()`（橙色）
- Assistant 图标沿用现有 `theme.style_brand()`（青色）

### 改动位置
1. `draw()` 方法中用户消息前缀渲染（第 465-468 行）
2. `draw()` 方法中系统消息 Assistant 前缀渲染（第 525-528 行）
3. `draw()` 方法中 spinner 前缀渲染（第 572-573 行）

### 约束
- 保持 1 行高度，不增加垂直空间占用
- 保持现有 `total_height()` 计算逻辑不变
- 不引入新的状态或动画
