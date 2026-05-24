# Web E2E 测试 - Rust 版本

## 目的
使用 Rust 和 reqwest 测试 fi-code 的 HTTP API，进行完整的端到端测试。

## 计划测试范围
- [ ] 服务器启动和健康检查
- [ ] 会话创建和管理
- [ ] 消息发送和流式响应
- [ ] 工具调用执行
- [ ] 文件 API 测试

## 运行
（待添加 Cargo.toml 配置后）
```bash
cargo test --test e2e_web_api
```
