# Slash 指令二级菜单设计文档

## 需求概述

为 `/themes`、`/skills`、`/models` 指令增加二级菜单，参考 TUI 实现。用户输入 `/themes` 或从一级菜单选择后，弹出二级选择列表。

## 核心决策

- 覆盖指令：`/themes`（主题列表 + 实时预览）、`/skills`（Skill 列表）、`/models`（两步：Provider → Model）
- `/themes` 选择时实时预览（悬停/键盘选择立即切换主题）
- `/skills` 数据来自后端 `/api/skills`（新增 API）
- `/models` 数据来自已有 `/api/models`

## 后端改动

### 新增 `GET /api/skills`

```rust
// crates/core/src/server/api/skill_api.rs
#[derive(Debug, serde::Serialize)]
pub struct SkillItem {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub async fn list_skills() -> Json<ApiResponse<Vec<SkillItem>>> {
    let registry = fi_code_core::skills::get_registry();
    let items: Vec<SkillItem> = registry
        .entries
        .iter()
        .map(|e| SkillItem {
            id: e.id.clone(),
            name: e.metadata.name.clone(),
            description: e.metadata.description.clone(),
        })
        .collect();
    Json(ApiResponse::success(items))
}
```

注册到 `server.rs`：`/api/skills`

## 前端架构

```
InputBox
├── 一级菜单（已有）：/clear, /models, /themes, /skills, /init
└── 二级菜单（新增）：
    ├── /themes → themePresets[]
    │   └── hover/键盘选择 → applyTheme(preset) 实时预览
    ├── /skills → GET /api/skills
    │   └── Enter → 加载 skill（execute_command "skills"）
    └── /models → GET /api/models
        ├── Step 1: Provider 列表
        └── Step 2: 选中 Provider 的 Model 列表
            └── Enter → POST /api/model/switch
```

## 组件改动

### 1. 新增 `frontend/src/types/skill.ts`
```typescript
export interface SkillItem {
  id: string;
  name: string;
  description: string;
}
```

### 2. `apiClient.ts` — 新增方法
```typescript
async getSkills(): Promise<{ id: string; name: string; description: string }[]> {
  return this.get('/api/skills');
}
```

### 3. `InputBox.tsx` — 核心改动

**新增状态：**
```typescript
type SubmenuKind = 'theme' | 'skill' | 'model_provider' | 'model_list' | null;

const [submenuKind, setSubmenuKind] = useState<SubmenuKind>(null);
const [submenuItems, setSubmenuItems] = useState<Array<{ key: string; display: string; desc: string }>>([]);
const [submenuIndex, setSubmenuIndex] = useState(0);
const [submenuContext, setSubmenuContext] = useState<string>(''); // 存储当前 provider key
const [previewThemeBackup, setPreviewThemeBackup] = useState<string | null>(null);
```

**一级菜单确认时判断：**
```typescript
const confirmCommand = (cmd: CommandMeta) => {
  if (cmd.name === 'themes') {
    setPreviewThemeBackup(themeName);
    setSubmenuKind('theme');
    setSubmenuItems(themePresets.map(p => ({ key: p.name, display: p.name, desc: p.description })));
    setSubmenuIndex(0);
    setShowMenu(false);
    return;
  }
  if (cmd.name === 'skills') {
    loadSkillsSubmenu();
    return;
  }
  if (cmd.name === 'models') {
    loadModelProvidersSubmenu();
    return;
  }
  // 其他命令：填充到输入框（保持原有逻辑）
  setInput(`/${cmd.name} `);
  setShowMenu(false);
};
```

**二级菜单键盘事件：**
- ↑↓：切换高亮
- Enter：确认选择
- Esc：关闭二级菜单，恢复主题预览（如果是 themes）

**二级菜单渲染：**
- 样式与一级菜单一致，但显示在输入框上方更大区域
- 每项显示：`display`（加粗）+ `desc`（灰色小字）

### 4. `AppLayout.tsx` — 加载 Provider 数据

AppLayout 已加载 `/api/commands`，二级菜单的 `/api/models` 和 `/api/skills` 在 InputBox 中按需加载。

## 交互细节

| 场景 | 行为 |
|------|------|
| 输入 `/themes` + Enter | 弹出主题二级菜单 |
| 主题菜单 ↑↓ | 实时切换预览主题 |
| 主题菜单 Enter | 确认主题，关闭菜单 |
| 主题菜单 Esc | 恢复之前主题，关闭菜单 |
| 输入 `/skills` + Enter | 弹出 Skill 二级菜单 |
| Skill 菜单 Enter | 调用后端加载 skill，关闭菜单 |
| 输入 `/models` + Enter | 弹出 Provider 二级菜单 |
| Provider 菜单 Enter | 弹出该 Provider 的 Model 二级菜单 |
| Model 菜单 Enter | 调用 `/api/model/switch` 切换模型，关闭菜单 |

## 样式

- 二级菜单宽度 320px，最大高度 240px，可滚动
- 背景 `bg-bg-secondary`，边框 `border-border`
- 高亮项 `bg-bg-overlay text-brand`
- 步骤指示（如 Model 切换）：菜单顶部显示 "Select Provider → Select Model"
