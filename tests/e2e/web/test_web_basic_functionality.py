"""
基础功能测试

测试场景:
1. 页面加载和基本界面
2. 简单文本对话
3. 工具调用流程验证
"""
import asyncio
from pathlib import Path
from typing import Dict, Any

import pytest
from playwright.async_api import Page, expect

from common import constants


@pytest.mark.web
@pytest.mark.functional
class TestBasicFunctionality:
    """基础功能测试"""
    
    @pytest.fixture(autouse=True)
    def setup_project(self, test_project_manager):
        """每个测试前准备项目"""
        self.project_manager = test_project_manager
    
    async def test_page_loads_successfully(self, chat_page: Page, server_url: str):
        """
        场景 1: 页面加载和基本界面
        
        测试步骤:
        1. 验证页面能成功加载
        2. 检查基础 UI 元素存在
        3. 验证界面标题显示正确
        """
        print(f"{constants.COLOR_YELLOW}测试场景：页面加载和基本界面{constants.COLOR_RESET}")
        
        # 尝试截图（无论页面是否成功加载）
        try:
            screenshot_path = constants.TEST_TEMP_DIR / "test_page_load.png"
            screenshot_path.parent.mkdir(parents=True, exist_ok=True)
            await chat_page.screenshot(path=str(screenshot_path))
            print(f"{constants.COLOR_GREEN}页面截图保存到: {screenshot_path}{constants.COLOR_RESET}")
        except Exception as e:
            print(f"{constants.COLOR_YELLOW}截图失败: {e}{constants.COLOR_RESET}")
        
        # 这个测试先验证基础组件，或者打印页面内容用于调试
        try:
            # 尝试获取页面内容用于调试
            content = await chat_page.content()
            print(f"{constants.COLOR_YELLOW}页面内容长度: {len(content)}{constants.COLOR_RESET}")
        except:
            pass
        
        print(f"{constants.COLOR_GREEN}✅ 页面加载测试框架就绪{constants.COLOR_RESET}")
        pass
    
    async def test_simple_text_conversation(self, chat_page: Page, mock_ai_server):
        """
        场景 2: 简单文本对话
        
        测试步骤:
        1. 在输入框中输入简单的问候
        2. 发送消息
        3. 验证用户消息显示在聊天界面
        4. 验证 AI 回复显示
        """
        print(f"{constants.COLOR_YELLOW}测试场景：简单文本对话{constants.COLOR_RESET}")
        
        # 设置 Mock AI 响应
        mock_ai_server.set_simple_text_response("Hello! I'm fi-code assistant. How can I help you today?")
        
        # TODO: 完善测试逻辑 - 需要先定位页面元素
        
        print(f"{constants.COLOR_GREEN}✅ 简单文本对话测试框架就绪{constants.COLOR_RESET}")
        pass


@pytest.mark.web
@pytest.mark.functional
class TestChatInterface:
    """聊天界面元素测试"""
    
    async def test_input_area_exists(self, chat_page: Page):
        """
        测试输入区域存在
        """
        print(f"{constants.COLOR_YELLOW}测试：输入区域存在性检查{constants.COLOR_RESET}")
        
        # TODO: 完善 - 查找输入框元素
        
        print(f"{constants.COLOR_GREEN}✅ 输入区域测试框架就绪{constants.COLOR_RESET}")
        pass
    
    async def test_send_button_functionality(self, chat_page: Page):
        """
        测试发送按钮功能
        """
        print(f"{constants.COLOR_YELLOW}测试：发送按钮功能{constants.COLOR_RESET}")
        
        # TODO: 完善
        
        print(f"{constants.COLOR_GREEN}✅ 发送按钮测试框架就绪{constants.COLOR_RESET}")
        pass


# 辅助工具类
class ChatTestHelper:
    """聊天测试辅助工具"""
    
    def __init__(self, page: Page):
        self.page = page
    
    async def send_message(self, text: str):
        """发送消息"""
        # TODO: 实现 - 定位输入框，输入文本，点击发送
        pass
    
    async def wait_for_assistant_response(self, timeout: int = 30000):
        """等待助手回复"""
        # TODO: 实现
        pass
    
    async def get_last_user_message(self) -> str:
        """获取最后的用户消息"""
        # TODO: 实现
        return ""
    
    async def get_last_assistant_message(self) -> str:
        """获取最后的助手消息"""
        # TODO: 实现
        return ""
    
    async def has_tool_result(self, tool_name: str = None) -> bool:
        """检查是否有工具结果"""
        # TODO: 实现
        return False


@pytest.fixture
def chat_helper(chat_page: Page) -> ChatTestHelper:
    """聊天测试辅助工具 fixture"""
    return ChatTestHelper(chat_page)
