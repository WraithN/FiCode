Feature: Agent 类型与工具过滤
  Build Agent 可以使用所有工具，Plan Agent 只能使用只读和任务规划工具。
  当 Plan Agent 尝试调用被禁止的工具时，系统应返回 ToolError。

  Background:
    Given 一个配置了 Mock Provider 的后端服务

  Scenario: Build Agent 可以调用 bash 工具
    When 用户以 Build Agent 发送消息 "运行 ls 命令"
    Then Agent 应该调用 bash 工具
    And 用户应该收到命令执行结果

  Scenario: Plan Agent 禁止调用 bash 工具
    When 用户以 Plan Agent 发送消息 "运行 ls 命令"
    Then Agent 应该收到 ToolError 事件，内容为 "not allowed in Plan Agent"

  Scenario: Plan Agent 可以调用 read 工具
    Given 工作目录下存在文件 "test.txt"，内容为 "Hello World"
    When 用户以 Plan Agent 发送消息 "读取 test.txt"
    Then Agent 应该调用 read 工具
    And 用户应该收到包含 "Hello World" 的响应

  Scenario: Build Agent 收到 AgentInfo 事件
    When 用户以 Build Agent 发送消息 "你好"
    Then Agent 应该收到 AgentInfo 事件，类型为 Build

  Scenario: Plan Agent 收到 AgentInfo 事件
    When 用户以 Plan Agent 发送消息 "你好"
    Then Agent 应该收到 AgentInfo 事件，类型为 Plan
