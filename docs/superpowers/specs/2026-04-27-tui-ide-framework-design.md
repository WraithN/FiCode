# FiCode TUI IDE 式框架重构设计文档

> 将 FiCode 从"单栏聊天框"升级为"以对话为核心、侧边抽屉为辅助"的 IDE 式 TUI 框架。

## 1. 设计目标

- **核心定位**：终端内的轻量 IDE，文件、历史、模型、主题四大功能围绕对话核心展开
- **用户体验**：保持 TUI 的简洁高效，同时具备现代工具的操控感
- **技术债务**：解决当前 TUI 的耦合问题（`ui.rs` 硬编码、无主题系统、无会话管理 UI）
- **扩展性**：为未来的插件系统、自定义主题、远程模式打下基础

## 2. 整体架构

### 2.1 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                        TUI Frontend                          │
├─────────────┬───────────────────────────────────────────────┤
│  Components │ Header │ LeftDrawer │ Chat │ Input │ RightDrawer│
│  (渲染层)    │ (模型/主题/新建)│ (文件树)   │(消息)│(输入框) │ (会话历史) │
├─────────────┴───────────────────────────────────────────────┤
│  Layout Manager (布局层) — 计算各面板 Rect，处理抽屉互斥      │
├─────────────────────────────────────────────────────────────┤
│  App State Machine (状态层) — Focus 管理、面板显隐、主题切换   │
├─────────────────────────────────────────────────────────────┤
│  TuiClient (通信层) — HTTP Client，SSE + 新 REST API         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ HTTP (localhost:4040)
┌─────────────────────────────────────────────────────────────┐
│                      Axum Backend                            │
├─────────────┬───────────────────────────────────────────────┤
│  Existing   │ /rpc (JSON-RPC) │ /chat (SSE)                  │
├─────────────┴───────────────────────────────────────────────┤
│  New APIs   │ GET /api/files │ GET /api/sessions             │
│             │ GET /api/sessions │ POST /api/sessions         │
│             │ PUT /api/sessions/:id │ DELETE /api/sessions/:id│
└─────────────────────────────────────────────────────────────┘
```

### 2.2 目录结构

```
src/
├── main.rs
├── entry.rs                    # 入口逻辑（保持现有）
├── server/                     # 后端服务（扩展现有）
│   ├── mod.rs
│   ├── routes.rs               # 路由聚合
│   ├── chat.rs                 # 现有 /chat SSE
│   ├── rpc.rs                  # 现有 /rpc JSON-RPC
│   ├── session_api.rs          # 新增：会话管理 API
│   ├── file_api.rs             # 新增：文件树 + Git 状态 API
│   └── models.rs               # 新增：API DTO
├── tui/                        # TUI 前端（全面重构）
│   ├── mod.rs                  # 入口：run_tui()
│   ├── app.rs                  # TuiApp：状态机 + 事件循环
│   ├── theme.rs                # Theme Token 系统
│   ├── layout.rs               # LayoutManager：Rect 计算
│   ├── client.rs               # TuiClient：HTTP 通信（扩展）
│   ├── event.rs                # AppEvent / SseEvent 枚举
│   └── components/             # 组件目录
│       ├── mod.rs              # Component trait 定义
│       ├── header.rs           # Header 组件
│       ├── left_drawer.rs      # 文件导航抽屉
│       ├── right_drawer.rs     # 会话历史抽屉
│       ├── chat.rs             # 主聊天区
│       ├── input.rs            # 多行输入框
│       └── status_bar.rs       # 底部状态栏
└── session/                    # 现有模块（后端复用）
    └── ...
```

### 2.3 关键设计决策

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 组件通信 | 直接调用 + AppEvent | 不引入 Redux，避免过度设计；组件通过 `handle_event` 返回 `AppEvent`，由 App 统一分发 |
| 状态位置 | App 持有全局状态，组件持有局部状态 | 如 `file_tree` 在 LeftDrawer 内，`messages` 在 Chat 内；App 只管理 `focus` 和 `panel` |
| 主题切换 | 运行时热切换 | TuiApp 持有 `Arc<Theme>`，切换时替换 Arc，所有组件自动使用新主题 |
| 文件树数据 | 后端 API 提供 | 保持架构统一，后续可支持远程工作目录 |
| 会话切换 | 后端 API + 内存状态 | 切换会话时 TUI 清空消息列表，后端加载新会话历史 via SSE |

## 3. Theme Token 系统

### 3.1 核心原则

- **零硬编码**：`ui.rs` 中不允许出现任何 `Color::Blue`、`Color::Rgb(57, 208, 216)` 等字面量
- **Token 化命名**：颜色按语义命名（`brand`, `user`, `success`），而非按视觉命名（`cyan`, `orange`）
- **运行时切换**：切换主题只是替换 `Arc<Theme>`，无需重新编译
- **预设 + 自定义**：内置 5 套预设主题，支持用户通过 YAML/JSON 自定义（P2）

### 3.2 Theme 数据结构

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    // 结构层
    pub bg_base: Color,        // 最底层背景
    pub bg_surface: Color,     // 抽屉、代码块、输入框
    pub bg_overlay: Color,     // 下拉菜单、悬浮面板
    pub border: Color,         // 所有分隔线、边框
    
    // 文字层
    pub text_primary: Color,     // 主要文本
    pub text_secondary: Color,   // 时间戳、元信息
    pub text_muted: Color,       // 禁用态、分隔符
    pub text_placeholder: Color, // 输入框 placeholder
    
    // 品牌与语义
    pub brand: Color,      // AI 消息、Logo、选中态
    pub user: Color,       // 用户消息标识
    pub success: Color,    // 就绪、成功操作
    pub warning: Color,    // 警告、慢速提示
    pub error: Color,      // 错误、删除确认
    
    // 交互层
    pub selection_bg: Color,   // 树形选中、列表聚焦
    pub selection_fg: Color,   // 选中文字
    pub accent_hover: Color,   // 可点击元素 hover
}
```

### 3.3 预设主题

| Token | Deep Ocean (默认) | GitHub Dark | Monokai Pro | Solarized Dark | High Contrast |
|-------|-------------------|-------------|-------------|----------------|---------------|
| `bg_base` | `#0d1117` | `#0d1117` | `#2d2a2e` | `#002b36` | `#000000` |
| `bg_surface` | `#161b22` | `#161b22` | `#383539` | `#073642` | `#1a1a1a` |
| `brand` | `#39d0d8` | `#58a6ff` | `#78dce8` | `#2aa198` | `#ffff00` |
| `user` | `#f0883e` | `#f0883e` | `#fc9867` | `#cb4b16` | `#ffffff` |
| `success` | `#3fb950` | `#3fb950` | `#a9dc76` | `#859900` | `#00ff00` |
| `error` | `#f85149` | `#f85149` | `#ff6188` | `#dc322f` | `#ff0000` |

### 3.4 主题切换机制

- `Ctrl+T`：快速轮询（循环切换，适合盲操）
- `Ctrl+Shift+T`：展开下拉带实时预览（上下切换时背景即时变化）
- TuiApp 持有 `Arc<Theme>`，切换时原子替换

## 4. Component Trait 与组件接口

### 4.1 核心 Trait

```rust
pub trait Component {
    fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme);
    fn handle_event(&mut self, event: &Event, focus: bool) -> Option<AppEvent>;
    fn update(&mut self, event: &AppEvent);
    fn is_focusable(&self) -> bool { true }
}
```

### 4.2 AppEvent 枚举（核心事件）

```rust
pub enum AppEvent {
    Tick, Resize(u16, u16),
    ToggleLeftDrawer, ToggleRightDrawer, CloseDrawers,
    FocusNext, FocusPrev, SetFocus(FocusArea),
    ToggleModelDropdown, ToggleThemeDropdown,
    SelectModel(String), SelectTheme(usize),
    NewSession, NewSessionWithName(String), NewSessionFromTemplate(SessionTemplate),
    SubmitMessage(String), InputChanged(String),
    ScrollUp, ScrollDown, CopyLastCode, StopGeneration,
    SseEvent(SsePayload), ChatComplete, ExecuteComplete(String),
    SwitchSession(String), DeleteSession(String), RenameSession(String, String),
    ToggleFolder(String), SelectFile(String), OpenFile(String),
    PreviewFile(String), AddToContext(String),
    Quit,
}
```

### 4.3 组件清单

| 组件 | 核心状态 | 主要交互 |
|------|----------|----------|
| **Header** | `model_dropdown_open`, `theme_dropdown_open`, `dropdown_selected` | `Ctrl+M` 模型下拉, `Ctrl+T` 主题切换, `Ctrl+N` 新建会话 |
| **LeftDrawer** | `tree: FileTree`, `selected_index`, `expanded_folders` | `↑↓` 导航, `→` 展开, `Enter` 引用, `Space` 预览, `O` 打开, `A` 添加全部 |
| **RightDrawer** | `sessions: Vec<SessionMeta>`, `filter`, `filter_active` | `Enter` 切换, `D` 删除, `R` 重命名, `/` 过滤 |
| **Chat** | `messages: Vec<Message>`, `scroll_offset`, `is_generating`, `preview_mode` | `PageUp/Down` 滚动, 代码块操作 `[C]opy [I]nsert [R]un` |
| **Input** | `content`, `cursor_position`, `multiline`, `dropdown_visible` | `Shift+Enter` 换行, `Enter` 发送, `/` 命令面板 |
| **StatusBar** | `shortcuts: Vec<ShortcutHint>` | 动态显示当前可用快捷键 |

## 5. 布局管理器

### 5.1 1+2 抽屉系统

- **左右抽屉互斥**：打开左侧文件栏时，自动收起右侧历史栏，反之亦然
- **抽屉宽度固定**：占屏幕 25-30%，最小 24 列，最大 40 列
- **窄屏自适应**：终端 < 80 列时，抽屉变为 Overlay 悬浮在主区上方，主区变暗

### 5.2 布局区域

```
┌─────────────────────────────────────────────────────────────────────┐
│ Header Bar (固定 3 行)                                               │
├──────────┬──────────────────────────────┬─────────────────────────┤
│          │                              │                         │
│  Drawer  │                              │   Drawer                │
│  Left    │      Main Chat Area          │   Right                 │
│  [可隐藏] │      (Messages + Input)      │   History               │
│          │                              │   [可隐藏]               │
│          │                              │                         │
├──────────┴──────────────────────────────┴─────────────────────────┤
│ Status Bar (固定 1 行)                                               │
└─────────────────────────────────────────────────────────────────────┘
```

### 5.3 主区内部分割

Main 区域动态分割为 Messages（滚动区域）和 Input（输入框）：
- Input 高度自适应：1-5 行（根据内容），超出滚动
- Messages 占据剩余空间

## 6. 后端 API 设计

### 6.1 路由表

```
GET    /api/files              # 文件树 + Git 状态
GET    /api/files/content      # 文件内容预览
GET    /api/sessions           # 会话列表
POST   /api/sessions           # 新建会话
PUT    /api/sessions/:id       # 重命名会话
DELETE /api/sessions/:id       # 删除会话
POST   /api/sessions/:id/switch # 切换会话并加载历史
```

### 6.2 统一响应格式

```json
{
  "success": true,
  "data": { ... },
  "error": null,
  "code": null
}
```

### 6.3 与现有架构集成

- **复用 `SessionManager`**：现有的 JSONL 读写逻辑完全复用
- **文件系统只读**：文件 API 只提供读取和遍历，写操作仍通过现有工具调用机制
- **Git 状态获取**：P0 通过执行 `git status --porcelain` 命令解析；P1 可引入 `git2` crate

## 7. 事件循环与交互路由

### 7.1 全局快捷键

| 快捷键 | 功能 | 状态 |
|--------|------|------|
| `Ctrl+B` | 切换左侧文件抽屉 | 全局 |
| `Ctrl+H` | 切换右侧历史抽屉 | 全局 |
| `Ctrl+M` | 展开模型下拉 | 全局 |
| `Ctrl+T` | 快速轮询主题 | 全局 |
| `Ctrl+Shift+T` | 展开主题下拉预览 | 全局 |
| `Ctrl+N` | 快速新建会话 | 全局 |
| `Ctrl+Shift+N` | 带模板新建会话 | 全局 |
| `Tab` / `Shift+Tab` | 焦点循环 | 全局 |
| `Esc` | 关闭抽屉/下拉 | 全局 |
| `Ctrl+C` | 生成中时停止 / 空闲时退出 | 全局 |
| `Shift+Enter` | 输入框换行 | Input 聚焦时 |
| `Enter` | 发送消息 | Input 聚焦时 |
| `PageUp/Down` | 消息区域滚动 | Main 聚焦时 |

### 7.2 焦点管理

- 根据当前面板状态动态调整焦点循环顺序
- 抽屉打开时，焦点锁定在抽屉内
- 窄屏 Overlay 模式下，焦点同样锁定在悬浮抽屉内

### 7.3 生成状态机

```
[Idle] ──SubmitMessage──► [Generating]
                              │
                              ▼ SSE 流式事件
                        [Streaming] ◄──────┐
                              │            │
                              ▼            │
                        [ChatComplete] ────┘
                              │
                              ▼
                          [Idle]
```

## 8. 实现优先级

| 优先级 | 模块 | 内容 |
|--------|------|------|
| **P0** | Theme 系统 | `Theme` struct、5 套预设、`Arc<Theme>` 切换机制 |
| **P0** | Component trait | 定义 trait、创建 `components/` 目录结构 |
| **P0** | Layout Manager | 布局计算、抽屉互斥、窄屏 Overlay |
| **P0** | App 状态机 | 事件循环重构、焦点管理、全局快捷键路由 |
| **P0** | Header | Logo、模型下拉、主题切换、新建会话 |
| **P1** | Chat | 消息渲染、代码块、时间戳、Loading 动画 |
| **P1** | Input | 多行输入、Slash 命令、输入历史 |
| **P1** | StatusBar | 动态快捷键提示 |
| **P1** | 后端 API - 会话 | `SessionManager` 封装为 REST API |
| **P1** | RightDrawer | 会话历史列表、切换、删除、重命名、过滤 |
| **P2** | 后端 API - 文件 | 文件树遍历、Git 状态解析、内容读取 |
| **P2** | LeftDrawer | 文件导航树、选中、预览、添加到上下文 |
| **P2** | 空状态 & 动画 | ASCII Logo、加载脉冲动画、代码块操作按钮 |
| **P2** | 自定义主题 | YAML/JSON 主题文件加载 |

## 9. 风险评估与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 重构期间破坏现有 TUI 功能 | 高 | 保留旧 TUI 代码作为 fallback（feature flag），逐个组件替换验证 |
| 多行输入与现有事件循环冲突 | 中 | 先独立测试 Input 组件的事件处理，再集成到 App |
| 后端 API 延迟影响 TUI 响应 | 中 | 文件树和会话列表首次加载时显示骨架屏/Loading |
| 终端兼容性（不同 TERM） | 低 | 使用 ratatui 的跨终端抽象，避免依赖特定终端特性 |
| Git 状态解析性能（大仓库） | 低 | 限制遍历深度，异步执行 git 命令，超时降级 |

## 10. 附录

### 10.1 新增依赖预测

```toml
# Cargo.toml 可能新增
walkdir = "2.5"       # 文件树遍历（P2）
chrono = "0.4"        # 时间戳处理（会话历史）
# git2 = "0.18"       # Git 状态解析（P1/P2，可选）
```

### 10.2 代码行数预估

| 模块 | 预估行数 | 说明 |
|------|----------|------|
| `theme.rs` | 150 | 5 套预设 + Style 构造器 |
| `layout.rs` | 120 | 布局计算 + 响应式逻辑 |
| `event.rs` | 80 | 枚举定义 |
| `components/mod.rs` | 30 | Component trait |
| `header.rs` | 200 | 下拉菜单 + 交互 |
| `left_drawer.rs` | 180 | 树形渲染 + Git 标记 |
| `right_drawer.rs` | 160 | 列表 + 过滤 + 重命名 |
| `chat.rs` | 200 | 消息气泡 + 代码块 + 滚动 |
| `input.rs` | 150 | 多行 + 命令面板 |
| `status_bar.rs` | 60 | 快捷键动态渲染 |
| `app.rs` | 250 | 状态机 + 事件循环 + 路由 |
| `server/*.rs` | 300 | 新增 API 路由 + Handler |
| **总计** | **~1880** | 不含测试 |
