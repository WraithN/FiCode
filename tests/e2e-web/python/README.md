# fi-code Web E2E 测试

基于 pytest + playwright 的 fi-code Web 端端到端测试套件。

## 目录结构

```
tests/web/
├── README.md                # 本文件
├── pytest.ini              # pytest 配置
├── requirements.txt        # Python 依赖
├── conftest.py            # 共享 fixtures
├── constants.py           # 测试常量
├── utils/
│   ├── server.py          # fi-code 服务器管理
│   ├── mock_ai.py        # Mock AI 服务器
│   └── project.py        # 测试项目管理
├── test_01_common/        # 共享测试（如果有）
├── test_02_web/           # Web 端测试
│   ├── test_web_single_tools.py
│   ├── test_web_workflows.py
│   ├── test_web_special_features.py
│   └── test_web_performance.py
└── test_03_desktop/       # Tauri 桌面端测试（如果有差异）
```

## 前置条件

1. **Rust 工具链** - 已安装 Rust，项目能成功构建
2. **Python 3.8+** - Python 环境
3. **Playwright 浏览器** - 已安装浏览器二进制

## 安装依赖

```bash
cd tests/web

# 创建虚拟环境（推荐）
python -m venv venv
source venv/bin/activate  # Linux/Mac
# 或
venv\Scripts\activate  # Windows

# 安装依赖
pip install -r requirements.txt

# 安装 Playwright 浏览器
playwright install
```

## 构建项目

确保先构建 fi-code 项目：

```bash
cd /home/nan/fi-code
cargo build
```

## 运行测试

### 🚀 快速开始

```bash
cd tests/web

# 安装依赖（首次）
pip install -r requirements.txt
playwright install
```

### Mock 模式（默认）

默认使用 Mock AI，无需真实 API：

```bash
pytest -v
```

### 真实模式（调用真实 AI）

通过环境变量关闭 Mock：

```bash
USE_MOCK_AI=false pytest -v
```

### 所有测试

```bash
cd tests/web
pytest -v
```

### 仅 Web 端测试

```bash
pytest -v -m web
```

### 仅功能测试

```bash
pytest -v -m functional
```

### 仅性能测试

```bash
pytest -v -m performance
```

### 运行特定测试文件

```bash
pytest -v test_02_web/test_web_single_tools.py
```

### 运行特定测试场景

```bash
pytest -v test_02_web/test_web_single_tools.py::TestSingleTools::test_bash_tool
```

## 配置选项

### Mock 开关

| 环境变量 | 值 | 说明 |
|---------|----|------|
| `USE_MOCK_AI` | `true` (默认) | 使用 Mock AI |
| `USE_MOCK_AI` | `false` | 调用真实 API |

**示例：**

```bash
# 开发调试 - 使用 Mock AI（默认）
pytest -v

# 端到端测试 - 连接真实 API
USE_MOCK_AI=false pytest -v
```

### 浏览器选择

默认使用 Chromium。可以修改 `conftest.py` 中的浏览器启动代码使用 Firefox 或 WebKit。

### 超时配置

### 无头模式

编辑 `conftest.py`，将 `headless=False` 改为 `headless=True`。

### 超时配置

可以通过环境变量修改超时：

```bash
TEST_TIMEOUT=600 pytest -v  # 10分钟超时
```

## 测试场景说明

### 功能测试

1. **单个工具调用** (`test_web_single_tools.py`)
   - Bash 工具
   - Read 工具
   - Write 工具
   - Edit 工具
   - Glob 工具
   - Grep 工具
   - Git 工具

2. **工具流程** (`test_web_workflows.py`)
   - 完整的软件开发工作流
   - 文件编辑和 git 提交流程
   - 多个工具组合使用
   - 错误恢复流程

3. **特殊功能** (`test_web_special_features.py`)
   - Task 任务拆分功能
   - Task plan 处理功能
   - Ask for question 功能
   - Compact 上下文压缩功能
   - Skills 功能
   - Slash 命令功能

### 性能测试

1. **简单查询响应性能**
2. **长响应性能**
3. **多轮对话性能**
4. **工具调用性能**
5. **浏览器资源占用**
6. **SSE 流式输出性能**

## 调试

### 查看测试输出

默认 pytest 会捕获输出。使用 `-s` 参数查看输出：

```bash
pytest -v -s
```

### 查看截图

失败时会自动截图保存在 `/tmp/fi_code_test/` 目录下。

### 查看服务器日志

测试运行时会保存服务器日志，可以通过 test_project_manager 访问。

## 开发指南

### 添加新测试

1. 在对应目录下创建新的测试文件
2. 继承 pytest 的测试类
3. 使用已有的 fixtures
4. 添加详细的场景说明注释

### 扩展 Mock AI

修改 `utils/mock_ai.py` 来添加更多场景的模拟响应。

## 注意事项

1. **测试隔离** - 每个测试都有独立的浏览器上下文和测试项目
2. **清理机制** - 测试前后会自动清理临时目录
3. **性能标记** - 性能测试有 `@pytest.mark.slow` 标记
4. **真实网络** - 当前设计使用 Mock AI，不调用真实 API

## 故障排除

### 端口占用

如果提示端口被占用，可以修改 `constants.py` 中的端口号，或者结束占用进程。

### 浏览器启动失败

确保已运行 `playwright install`。

### 服务器启动失败

确保项目已成功构建，`target/debug/fi-code-server` 存在。
