# fi-code E2E 测试

## 运行全部测试

```bash
pytest tests/e2e -v
```

## 运行指定模块

```bash
pytest tests/e2e/cli -v
pytest tests/e2e/tui -v
pytest tests/e2e/web -v
```

## 运行单个用例

```bash
pytest tests/e2e/cli/test_cli_help.py -v
```

## 通过 Cargo 运行

```bash
cargo test --test e2e_all
```

## 前置条件

1. 编译 fi-code 二进制：
   ```bash
   cargo build
   ```

2. 安装 Python 依赖：
   ```bash
   cd tests/e2e
   pip install -r requirements.txt
   ```

3. 安装 Playwright 浏览器（仅 Web 测试需要）：
   ```bash
   playwright install chromium
   ```
