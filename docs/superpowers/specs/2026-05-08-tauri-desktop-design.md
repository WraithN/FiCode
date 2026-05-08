# fi-code 桌面端设计文档 —— Tauri + React

> 日期：2026-05-08
> 主题：基于 Tauri 框架的 fi-code 桌面端应用
> 状态：已评审

---

## 1. 设计目标

为 fi-code 构建一个跨平台桌面端应用，支持两种运行模式：

1. **独立模式**：桌面应用内嵌启动 fi-code Rust CLI（sidecar），通过本地 HTTP 通信
2. **远程模式**：配置连接远程 fi-code 服务端 URL

桌面端 UI 与现有 TUI 保持相同设计风格，但将底部快捷键提示栏取消，功能重分布到 Header 菜单和左侧常驻 Sidebar。

---

## 2. 架构决策

### 2.1 方案选择：单仓库内嵌 Tauri 项目

在现有 `fi-code` 仓库中新增 `desktop/` 目录，作为标准 Tauri 项目。现有 Rust 后端（`src/`）完全不变，桌面端通过 HTTP 复用同一套 API。

**不选 Workspace 重构方案的原因**：重构量过大（需拆分 core/server/tui crate，处理循环依赖），会显著延迟交付。

### 2.2 技术栈

| 层级 | 技术 | 说明 |
|------|------|------|
| 桌面框架 | Tauri v2 | 轻量、安全、跨平台 |
| 前端框架 | React 18 + TypeScript | 组件化、类型安全 |
| 构建工具 | Vite | Tauri 官方推荐，HMR 快 |
| 样式方案 | Tailwind CSS | 原子化 CSS，快速实现原生感界面 |
| 状态管理 | Zustand | 轻量，避免 Context 重渲染问题 |
| 流处理 | 原生 ReadableStream | SSE 逐帧读取 |

---

## 3. 项目结构

```
fi-code/
├── src/                          # 现有 Rust 后端（零改动）
│   ├── main.rs
│   ├── agent/
│   ├── provider/
│   ├── session/
│   ├── tools/
│   ├── config/
│   ├── tui/                      # 现有 TUI（独立可运行）
│   ├── server/                   # HTTP API（桌面端共用）
│   └── ...
├── Cargo.toml
│
├── desktop/                      # 新增 Tauri 桌面端项目
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   ├── tailwind.config.js
│   │
│   ├── src/                      # React 前端源码
│   │   ├── main.tsx              # 入口
│   │   ├── App.tsx               # 根组件（模式切换路由器）
│   │   ├── components/           # UI 组件
│   │   │   ├── Header.tsx
│   │   │   ├── Sidebar.tsx       # 左侧文件树（常驻可折叠）
│   │   │   ├── ChatPanel.tsx
│   │   │   ├── MessageBubble.tsx
│   │   │   ├── InputBox.tsx
│   │   │   ├── HistoryDrawer.tsx # 右侧会话历史（抽屉式）
│   │   │   ├── ModelDropdown.tsx # 模型选择两级菜单
│   │   │   ├── LogPanel.tsx      # 日志浮窗
│   │   │   ├── ApiKeyDialog.tsx  # API Key 输入模态框
│   │   │   └── ui/               # 基础组件（Button, Dialog 等）
│   │   ├── hooks/
│   │   │   ├── useClient.ts      # HTTP 客户端 + 连接状态
│   │   │   ├── useChatStream.ts  # SSE 流式对话管理
│   │   │   ├── useTheme.ts       # 主题切换
│   │   │   └── useSidecar.ts     # sidecar 生命周期（仅独立模式）
│   │   ├── services/             # API 封装（TypeScript 版 TuiClient）
│   │   │   ├── client.ts         # HTTP 客户端基类
│   │   │   ├── chat.ts
│   │   │   ├── session.ts
│   │   │   ├── file.ts
│   │   │   ├── model.ts
│   │   │   └── command.ts
│   │   ├── types/
│   │   │   ├── api.ts
│   │   │   ├── theme.ts
│   │   │   └── events.ts
│   │   ├── stores/
│   │   │   └── appStore.ts       # Zustand 全局状态
│   │   ├── themes/
│   │   │   ├── index.ts
│   │   │   └── presets/          # 与 TUI ThemePreset 对应的 CSS 变量
│   │   └── styles/
│   │       └── index.css         # Tailwind + CSS 变量
│   │
│   └── src-tauri/                # Tauri Rust 侧
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── capabilities/
│       └── src/
│           ├── main.rs
│           ├── lib.rs
│           └── sidecar.rs          # sidecar 进程管理
│
└── docs/superpowers/specs/
    └── 2026-05-08-tauri-desktop-design.md
```

---

## 4. 前端架构

### 4.1 组件树

```
<App>                          # 模式路由器（独立/远程/连接中）
├── <ConnectionProvider>        # 管理 client + 连接状态
│   ├── <ThemeProvider>
│   │   └── <Layout>
│   │       ├── <Header>
│   │       ├── <div.main>
│   │       │   ├── <Sidebar>
│   │       │   ├── <ChatPanel>
│   │       │   │   └── <MessageBubble> × N
│   │       │   └── <InputBox>
│   │       ├── <HistoryDrawer>
│   │       └── <LogPanel>
│   └── <ModelDropdown>
│   └── <ApiKeyDialog>
```

### 4.2 TUI → React 组件映射

| TUI 组件 (Rust) | React 组件 | 变化说明 |
|-----------------|------------|----------|
| `Header` | `Header.tsx` | 一致，增加设置菜单入口 |
| `LeftDrawer` | `Sidebar.tsx` | **改为常驻可折叠**，宽度可拖拽调整 |
| `Chat` | `ChatPanel.tsx` | 一致，增加虚拟滚动优化 |
| `Input` | `InputBox.tsx` | 一致，多行 textarea + 斜杠命令 |
| `RightDrawer` | `HistoryDrawer.tsx` | 保持抽屉行为，右侧滑出 |
| `StatusBar` | ❌ 取消 | 功能分散到 Header/Sidebar |
| `LogWindow` | `LogPanel.tsx` | 保留，改为浮动面板 |
| `ApiKeyDialog` | `ApiKeyDialog.tsx` | 保持模态框 |

### 4.3 状态管理（Zustand）

```typescript
interface AppState {
  // 连接
  mode: 'standalone' | 'remote';
  connectionStatus: 'connecting' | 'connected' | 'error';
  serverUrl: string;

  // 会话
  currentSessionId: string | null;
  sessions: SessionInfo[];

  // 模型
  currentModel: string;
  providers: ProviderItem[];

  // UI
  sidebarCollapsed: boolean;
  sidebarWidth: number;        // 可拖拽宽度，默认 240
  historyOpen: boolean;
  logOpen: boolean;
  themeName: string;
  isGenerating: boolean;

  // 聊天
  messages: Message[];
}
```

---

## 5. 通信层

### 5.1 核心客户端

```typescript
class ApiClient {
  constructor(baseUrl: string = 'http://localhost:4040');
  async rpc(method: string, params?: unknown): Promise<unknown>;
  async get<T>(path: string): Promise<T>;
  async post<T>(path: string, body?: unknown): Promise<T>;
  chatStream(sessionId: string | null, message: string): ReadableStream<SseEvent>;
}
```

### 5.2 API 服务与 Rust 端对照

| Rust 方法 | TypeScript | 端点 |
|-----------|-----------|------|
| `TuiClient::get_status()` | `getStatus()` | `POST /rpc` |
| `TuiClient::chat()` | `sendChatMessage()` | `POST /chat` (SSE) |
| `TuiClient::list_sessions()` | `listSessions()` | `GET /api/sessions` |
| `TuiClient::switch_session()` | `switchSession()` | `POST /api/sessions/:id/switch` |
| `TuiClient::get_file_tree()` | `getFileTree()` | `GET /api/files?path=` |
| `TuiClient::list_models()` | `listModels()` | `GET /api/models` |
| `TuiClient::switch_model()` | `switchModel()` | `POST /api/model/switch` |
| `TuiClient::subscribe_logs()` | `subscribeLogs()` | `GET /api/logs/stream` (SSE) |

### 5.3 SSE 流处理

使用浏览器原生 `ReadableStream` + `getReader()` 逐帧读取 SSE 事件，每条事件解析为结构化 `SseEvent`，与 Rust 端的 `server::transport::sse::SseEvent` 保持一致。

生成状态 `isGenerating` 同时控制：
- 输入框禁用/启用
- 发送按钮变为停止按钮
- Header 显示生成中指示器

---

## 6. Sidecar 生命周期（独立模式）

### 6.1 启动流程

```
Tauri 启动
  ├── 读取配置：mode = standalone
  ├── 查找 sidecar 二进制
  │     开发环境 → target/debug/fi-code
  │     生产环境 → app bundle 同目录
  ├── spawn: fi-code --server --port 4040
  ├── 轮询 health check (每 500ms，最多 10s)
  └── 连接成功 → 渲染主界面
      连接失败 → 显示错误页（重试/切换远程）
```

### 6.2 进程管理

- **端口冲突**：自动尝试 4041/4042... 最多 10 个端口
- **崩溃检测**：前端通过心跳检测连接状态，断开时显示"服务已停止"提示
- **优雅关闭**：Tauri `ExitRequested` 时发送 SIGTERM，3 秒后强制 kill
- **数据目录**：使用 Tauri `app_data_dir()` 隔离 sidecar 数据

### 6.3 远程模式

用户在设置页配置：
- `remoteUrl`: 远程服务端地址
- `authToken`（可选）: Bearer Token，在请求头中传递

前端直接通过 `fetch` 连接远程 URL，不启动 sidecar。

---

## 7. 布局设计

### 7.1 整体布局

```
┌─────────────────────────────────────────────────────────┐
│ Header (高 48px)                                         │
│ [Logo] [模型下拉 ▼]                        [设置 ⚙] [?]  │
├──────────┬──────────────────────────────────────────────┤
│ Sidebar  │ ChatPanel (flex: 1)                          │
│ (宽 240px,│                                              │
│  可折叠)  │  ┌──────────────────────────────────────┐   │
│ 📁 src    │  │ 消息气泡列表                          │   │
│ 📄 main.rs│  │ ...                                  │   │
│          │  └──────────────────────────────────────┘   │
│          │  ┌──────────────────────────────────────┐   │
│          │  │ InputBox (高自适应, max 120px)        │   │
│          │  │ [发送] Shift+Enter 换行               │   │
│          │  └──────────────────────────────────────┘   │
└──────────┴──────────────────────────────────────────────┘
```

### 7.2 原 StatusBar 功能重分布

| 原快捷键 | 新位置 | 交互方式 |
|----------|--------|----------|
| `Ctrl+B` Files | Sidebar 常驻 | 点击折叠/展开，或拖拽调整宽度 |
| `Ctrl+H` History | Header 按钮 | 点击"历史"图标打开右侧抽屉 |
| `Ctrl+M` Models | Header 模型下拉 | 点击模型名展开两级菜单 |
| `Ctrl+T` Themes | Header 设置 → 主题 | 设置下拉中选择 |
| `Ctrl+N` New | Header 设置 → 新建会话 | 设置下拉中选择 |
| `Ctrl+C` Stop | InputBox 右侧 | 生成中时显示红色"停止"按钮 |
| `Ctrl+L` Logs | Header 设置 → 显示日志 | 设置下拉中开关 |

### 7.3 Sidebar 设计

- **展开状态**：宽 240px（可拖拽调整 180px~360px），显示完整文件树
- **折叠状态**：宽 48px，仅显示垂直图标栏
- **顶部标题栏**：`[≡] 项目文件树`，点击 ≡ 切换折叠
- **文件点击**：当前仅展示文件树，编辑功能后续扩展

### 7.4 Header 模型下拉（两级菜单）

```
[Claude 3.7 Sonnet ▼]
┌─────────────────────────────┐
│ 🤖 Anthropic                │
│    ├── Claude 3.7 Sonnet ✓  │
│    └── Claude 3.5 Haiku     │
│ 🌋 Volcano Ark              │
│    ├── deepseek-r1          │
│    └── doubao-pro           │
│ 🔧 Custom                   │
└─────────────────────────────┘
```

选择预设 Provider 时弹出 `ApiKeyDialog` 模态框，选择 Custom 直接切换。

---

## 8. 主题系统

### 8.1 与 TUI ThemePreset 对齐

桌面端主题名称、颜色值与 TUI 完全一致，支持从后端 `GET /api/themes` 动态拉取。

### 8.2 CSS 变量映射

| Theme 字段 | CSS 变量 | Tailwind Token |
|------------|----------|----------------|
| `bg` | `--color-bg` | `bg-bg` |
| `bg_secondary` | `--color-bg-secondary` | `bg-bg-secondary` |
| `text_primary` | `--color-text-primary` | `text-text` |
| `text_secondary` | `--color-text-secondary` | `text-text-secondary` |
| `accent` | `--color-accent` | `text-accent` |
| `error` | `--color-error` | `text-error` |

---

## 9. 错误处理

### 9.1 连接状态机

```
Idle ──启动/配置URL──→ Connecting ──成功──→ Connected
                         │                    │
                         └──失败──→ Error ←──┘（断开）
```

### 9.2 错误场景处理

| 场景 | 用户感知 | 恢复策略 |
|------|----------|----------|
| Sidecar 二进制不存在 | 启动页错误 + "选择路径"按钮 | 用户手动 |
| 端口冲突 | 自动尝试后续端口（最多10个） | 自动 |
| Sidecar 崩溃 | Toast + "重启服务"按钮 | 用户手动 |
| 远程连接超时 | 连接页"连接失败"+重试 | 用户手动 |
| SSE 流中断 | 消息区显示"中断"+重试按钮 | 用户手动 |
| 消息量 > 1000 | 自动启用虚拟滚动 | 自动 |

---

## 10. 实现顺序

| 阶段 | 内容 | 产出 |
|------|------|------|
| P0 | 基础骨架 | Tauri 窗口 + React 渲染 |
| P1 | 通信层 + 连接管理 | 连接 localhost:4040，显示状态 |
| P2 | ChatPanel + InputBox | 发送消息，接收 SSE 回复 |
| P3 | Header + Sidebar + HistoryDrawer | 完整三栏布局 |
| P4 | Sidecar 管理 | 独立模式自动启动/停止 |
| P5 | 主题系统 + 斜杠命令 | 主题切换、/models /themes |
| P6 | 远程模式 + 设置页 | 配置远程 URL、模式切换 |
| P7 | 打包发布 | 生产构建、安装包 |

---

## 11. 关键依赖版本

```json
// desktop/package.json (关键依赖)
{
  "dependencies": {
    "react": "^18.3",
    "react-dom": "^18.3",
    "zustand": "^4.5",
    "tailwindcss": "^3.4"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0",
    "@tauri-apps/api": "^2.0",
    "@types/react": "^18.3",
    "typescript": "^5.4",
    "vite": "^5.2"
  }
}
```

---

*文档结束*
