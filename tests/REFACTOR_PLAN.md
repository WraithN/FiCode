# E2E 测试重构计划

## 当前结构
```
tests/
├── e2e/
│   ├── cli_e2e.rs          → e2e-tui/
│   ├── tui_e2e.rs          → e2e-tui/
│   └── tui_flow_e2e.rs     → e2e-tui/
├── bdd/                   → e2e-tui/ (或保留在根)
├── web/                   → e2e-web/python/
└── Cargo.toml
```

## 目标结构
```
tests/
├── e2e-tui/               # TUI 相关 E2E 测试
│   ├── cli_e2e.rs
│   ├── tui_e2e.rs
│   └── tui_flow_e2e.rs
├── e2e-web/               # Web 相关 E2E 测试
│   ├── rust/              # Rust 编写的 Web 测试
│   └── python/            # Python + Playwright 测试 (之前的 web/)
├── e2e-common/            # 共享基础设施
│   └── (待添加)
├── bdd/                   # BDD 测试（暂时保留）
└── Cargo.toml             # 更新测试配置
```

## 执行步骤
1. 创建目录
2. 移动 TUI E2E 测试到 e2e-tui/
3. 移动 Python Web 测试到 e2e-web/python/
4. 更新 Cargo.toml
5. 验证测试能运行
