# fi-code 测试套件

## 📂 目录结构

```
tests/
├── e2e-tui/              # TUI 端 E2E 测试（Rust）
│   ├── cli_e2e.rs        # CLI 命令 E2E 测试
│   ├── tui_e2e.rs        # TUI 基础 E2E 测试
│   └── tui_flow_e2e.rs   # TUI 完整流程 E2E 测试
│
├── e2e-web/              # Web 端 E2E 测试
│   ├── rust/            # Rust 编写的 Web API 测试（待实现）
│   └── python/          # Python + Playwright 浏览器测试
│       ├── conftest.py
│       ├── constants.py
│       ├── pytest.ini
│       ├── requirements.txt
│       ├── README.md
│       ├── utils/
│       ├── test_01_common/
│       ├── test_02_web/
│       └── test_03_desktop/
│
├── e2e-common/           # 共享测试基础设施（待实现）
│
├── bdd/                 # BDD 测试（Gherkin 格式）
│   ├── features/
│   └── steps/
│
├── Cargo.toml
└── lib.rs
```

## 🚀 运行测试

### TUI E2E 测试 (Rust)
```bash
# 运行所有 TUI E2E 测试
cargo test --test e2e_tui_cli
cargo test --test e2e_tui_basic
cargo test --test e2e_tui_flow

# 运行 TUI 完整流程测试
cargo test --test e2e_tui_flow -- --nocapture
```

### Web E2E 测试 (Python + Playwright)
```bash
cd e2e-web/python

# 安装依赖
pip install -r requirements.txt
playwright install

# Mock 模式（默认，无需 API Key）
pytest -v

# 真实模式（连接真实 API）
USE_MOCK_AI=false pytest -v
```

### BDD 测试
```bash
cargo test --test bdd
```

## 📝 测试分类

| 类别 | 位置 | 技术 |
|------|------|------|
| TUI E2E | `e2e-tui/` | Rust |
| Web E2E (Rust) | `e2e-web/rust/` | Rust + reqwest |
| Web E2E (Python) | `e2e-web/python/` | Python + Playwright |
| BDD | `bdd/` | Rust + Cucumber |
