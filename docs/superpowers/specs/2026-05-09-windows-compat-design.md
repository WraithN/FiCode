# Windows 兼容性支持设计文档

**日期:** 2026-05-09
**作者:** AI Assistant
**状态:** 设计阶段

## 1. 概述

本项目需要在 Windows 平台上提供 bash 命令执行的兼容性支持。当在 Windows 环境下启动时，需要依次检测 WSL2、Git Bash、Cygwin 是否安装，设置对应标识，并在执行 bash 指令时使用对应的环境。如果没有找到兼容环境，提示用户安装相应环境。

## 2. 架构设计

### 2.1 Windows 兼容模式枚举

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsCompatMode {
    /// 非 Windows 平台（默认使用本地 sh）
    Native,
    /// WSL2 已检测到
    Wsl2,
    /// Git Bash 已检测到
    GitBash,
    /// Cygwin 已检测到
    Cygwin,
    /// Windows 平台但未找到兼容环境
    None,
}

impl Default for WindowsCompatMode {
    fn default() -> Self {
        if cfg!(windows) {
            Self::None
        } else {
            Self::Native
        }
    }
}
```

### 2.2 全局懒加载检测

使用 `once_cell::sync::Lazy` 配合 `std::sync::RwLock` 实现懒加载检测：

- 首次使用时才进行检测（不影响非 Windows 平台启动速度）
- 检测顺序：WSL2 → Git Bash → Cygwin
- 检测完成后结果永久缓存

检测逻辑：
1. WSL2: 检测 `wsl.exe --version` 或检查环境变量 `WSL_DISTRO_NAME`
2. Git Bash: 检测 `bash.exe` 是否在 Program Files/Program Files (x86) 的 Git 目录下
3. Cygwin: 检测 Cygwin 目录

```rust
use once_cell::sync::Lazy;
use std::sync::RwLock;

static WINDOWS_COMPAT_MODE: Lazy<RwLock<WindowsCompatMode>> = Lazy::new(|| {
    RwLock::new(WindowsCompatMode::default())
});
```

### 2.3 检测函数

```rust
/// 检测 Windows 兼容环境
fn detect_windows_compat_mode() -> WindowsCompatMode {
    if !cfg!(windows) {
        return WindowsCompatMode::Native;
    }

    // 检测 WSL2
    if check_wsl2() {
        return WindowsCompatMode::Wsl2;
    }

    // 检测 Git Bash
    if check_git_bash() {
        return WindowsCompatMode::GitBash;
    }

    // 检测 Cygwin
    if check_cygwin() {
        return WindowsCompatMode::Cygwin;
    }

    WindowsCompatMode::None
}

fn check_wsl2() -> bool { /* 检测 WSL2 */ }
fn check_git_bash() -> bool { /* 检测 Git Bash */ }
fn check_cygwin() -> bool { /* 检测 Cygwin */ }
```

### 2.4 命令执行修改

在 `BasicTool::run_bash` 中：

- 非 Windows 平台：使用现有的 `sh` 执行
- Windows 平台：
  - 如果检测到兼容环境（WSL2/Git Bash/Cygwin），使用对应的 shell 执行
  - 如果没有检测到兼容环境，返回友好的提示信息

```rust
pub fn run_bash(command: &str) -> String {
    // ... 获取兼容模式 ...

    match compat_mode {
        WindowsCompatMode::Native => { /* 原有的非 Windows 逻辑 */ }
        WindowsCompatMode::Wsl2 => { /* 使用 wsl.exe 执行 */ }
        WindowsCompatMode::GitBash => { /* 使用 Git Bash 的 bash.exe */ }
        WindowsCompatMode::Cygwin => { /* 使用 Cygwin 的 bash.exe */ }
        WindowsCompatMode::None => {
            "Error: 未找到兼容的 bash 环境。请安装 WSL2、Git Bash 或 Cygwin。".to_string()
        }
    }
}
```

## 3. 依赖变更

在 `Cargo.toml` 中添加依赖：

```toml
once_cell = "1.19"
```

## 4. 文件变更

- 新建: `src/tools/windows_compat.rs` - Windows 兼容性模块
- 修改: `src/tools/basic_tools.rs` - 更新 `run_bash` 方法
- 修改: `src/tools/mod.rs` - 导出新模块
- 修改: `Cargo.toml` - 添加 `once_cell` 依赖

## 5. 错误处理

- 检测失败不会导致程序崩溃，只会标记为 `WindowsCompatMode::None`
- 当检测到 `None` 时，在执行 bash 命令时给用户清晰的提示信息
- 所有平台都能正常编译和运行，条件编译确保非 Windows 平台不会有额外开销

## 6. 测试策略

- 单元测试验证检测逻辑（使用模拟环境变量）
- 在 Windows 上进行手动测试（有条件的话）
- 确保 Linux/macOS 功能不受影响
