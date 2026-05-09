# Windows 兼容性支持实现计划

&gt; **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**目标:** 在 Windows 平台上添加 bash 命令执行的兼容性支持，依次检测 WSL2、Git Bash、Cygwin，设置标识并在执行 bash 指令时使用对应环境，无兼容环境时提示用户安装。

**架构:** 
- 使用 `once_cell::sync::Lazy` 实现懒加载检测
- 创建独立的 `WindowsCompat` 模块
- 修改 `BasicTool::run_bash` 以使用检测到的兼容环境

**技术栈:** Rust, `once_cell` crate

---

### Task 1: 添加 `once_cell` 依赖

**Files:**
- Modify: `/home/nan/fi-code/Cargo.toml`

- [ ] **Step 1: 添加依赖到 Cargo.toml**

在 `[dependencies]` 部分添加：
```toml
once_cell = "1.19"
```

- [ ] **Step 2: 验证依赖可以下载**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
cd /home/nan/fi-code
git add Cargo.toml
git commit -m "feat: add once_cell dependency"
```

---

### Task 2: 创建 Windows 兼容性模块

**Files:**
- Create: `/home/nan/fi-code/src/tools/windows_compat.rs`
- Modify: `/home/nan/fi-code/src/tools/mod.rs`

- [ ] **Step 1: 创建 `windows_compat.rs`**

Write the complete module:
```rust
// MIT License
// Copyright (c) 2025 fi-code contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use once_cell::sync::Lazy;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsCompatMode {
    Native,
    Wsl2,
    GitBash,
    Cygwin,
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

static WINDOWS_COMPAT_MODE: Lazy<RwLock<WindowsCompatMode>> = Lazy::new(|| {
    RwLock::new(detect_windows_compat_mode())
});

pub fn get_compat_mode() -> WindowsCompatMode {
    *WINDOWS_COMPAT_MODE.read().unwrap()
}

fn detect_windows_compat_mode() -> WindowsCompatMode {
    if !cfg!(windows) {
        return WindowsCompatMode::Native;
    }

    if check_wsl2() {
        return WindowsCompatMode::Wsl2;
    }

    if check_git_bash() {
        return WindowsCompatMode::GitBash;
    }

    if check_cygwin() {
        return WindowsCompatMode::Cygwin;
    }

    WindowsCompatMode::None
}

fn check_wsl2() -> bool {
    if std::env::var("WSL_DISTRO_NAME").is_ok() {
        return true;
    }

    if let Ok(output) = std::process::Command::new("wsl.exe")
        .arg("--version")
        .output()
    {
        return output.status.success();
    }

    false
}

fn check_git_bash() -> bool {
    let possible_paths = vec![
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];

    for path in possible_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    false
}

fn check_cygwin() -> bool {
    let possible_paths = vec![
        r"C:\cygwin64\bin\bash.exe",
        r"C:\cygwin\bin\bash.exe",
    ];

    for path in possible_paths {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }

    false
}

pub fn get_bash_path() -> Option<String> {
    match get_compat_mode() {
        WindowsCompatMode::Native => None,
        WindowsCompatMode::Wsl2 => Some("wsl.exe".to_string()),
        WindowsCompatMode::GitBash => {
            let possible_paths = vec![
                r"C:\Program Files\Git\bin\bash.exe",
                r"C:\Program Files (x86)\Git\bin\bash.exe",
            ];
            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return Some(path.to_string());
                }
            }
            None
        }
        WindowsCompatMode::Cygwin => {
            let possible_paths = vec![
                r"C:\cygwin64\bin\bash.exe",
                r"C:\cygwin\bin\bash.exe",
            ];
            for path in possible_paths {
                if std::path::Path::new(path).exists() {
                    return Some(path.to_string());
                }
            }
            None
        }
        WindowsCompatMode::None => None,
    }
}
```

- [ ] **Step 2: 修改 `tools/mod.rs` 导出新模块**

Open `/home/nan/fi-code/src/tools/mod.rs` and add the export:
```rust
// ... existing code ...
pub mod basic_tools;
pub mod tools_registry;
pub mod tools_type;
// Add this line:
pub mod windows_compat;
```

- [ ] **Step 3: 验证编译**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
cd /home/nan/fi-code
git add src/tools/windows_compat.rs src/tools/mod.rs
git commit -m "feat: add Windows compatibility module"
```

---

### Task 3: 修改 `BasicTool::run_bash` 使用兼容环境

**Files:**
- Modify: `/home/nan/fi-code/src/tools/basic_tools.rs`

- [ ] **Step 1: 添加导入**

在 `basic_tools.rs` 顶部添加导入：
```rust
use crate::tools::windows_compat::{get_compat_mode, get_bash_path, WindowsCompatMode};
```

- [ ] **Step 2: 修改 `run_bash` 方法**

替换 `run_bash` 方法（第 110-165 行）:
```rust
pub fn run_bash(command: &str) -> String {
    log_trace!("run_bash | command={}", command);
    let command = command.to_string();
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let compat_mode = get_compat_mode();
        
        let result = match compat_mode {
            WindowsCompatMode::Native => {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&command)
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .env(
                        "HOME",
                        std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()),
                    )
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
            }
            WindowsCompatMode::Wsl2 => {
                std::process::Command::new("wsl.exe")
                    .arg("sh")
                    .arg("-c")
                    .arg(&command)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
            }
            WindowsCompatMode::GitBash | WindowsCompatMode::Cygwin => {
                if let Some(bash_path) = get_bash_path() {
                    std::process::Command::new(bash_path)
                        .arg("-c")
                        .arg(&command)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output()
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Bash executable not found",
                    ))
                }
            }
            WindowsCompatMode::None => {
                let error_msg = "Error: 未找到兼容的 bash 环境。请安装 WSL2、Git Bash 或 Cygwin。";
                return tx.send(Err(std::io::Error::new(std::io::ErrorKind::Other, error_msg))).unwrap();
            }
        };

        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(Ok(output)) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let combined = format!("{}{}", stdout, stderr).trim().to_string();
            log_trace!(
                "run_bash result | len={} | preview={}",
                combined.len(),
                combined.chars().take(200).collect::<String>()
            );

            if combined.is_empty() {
                "(no output)".to_string()
            } else {
                combined.chars().take(50000).collect()
            }
        }
        Ok(Err(e)) => format!("Error: {}", e),
        Err(_) => "Error: Timeout (120s)".to_string(),
    }
}
```

- [ ] **Step 3: 运行 cargo check**

Run: `cargo check`
Expected: Compiles successfully

- [ ] **Step 4: 运行所有测试**

Run: `cargo test`
Expected: All 112 tests pass

- [ ] **Step 5: Commit**

```bash
cd /home/nan/fi-code
git add src/tools/basic_tools.rs
git commit -m "feat: modify run_bash to use Windows compatibility mode"
```

---

### Task 4: 添加单元测试

**Files:**
- Modify: `/home/nan/fi-code/src/tools/windows_compat.rs` (add test section)

- [ ] **Step 1: 添加测试模块**

在 `windows_compat.rs` 末尾添加：
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode() {
        // 默认应该是 Native 或 None（根据平台）
        let mode = WindowsCompatMode::default();
        assert!(matches!(mode, WindowsCompatMode::Native | WindowsCompatMode::None));
    }

    #[test]
    fn test_get_compat_mode() {
        let mode = get_compat_mode();
        assert!(matches!(mode, WindowsCompatMode::Native | WindowsCompatMode::None));
    }
}
```

- [ ] **Step 2: 运行所有测试**

Run: `cargo test`
Expected: All 113+ tests pass

- [ ] **Step 3: Commit**

```bash
cd /home/nan/fi-code
git add src/tools/windows_compat.rs
git commit -m "test: add Windows compatibility tests"
```

---

### Plan Complete!

All tasks are defined, and this feature is ready to be implemented!
