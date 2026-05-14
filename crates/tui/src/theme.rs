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

use ratatui::style::{Color, Style};

use fi_code_shared::dto::ThemePreset;

/// 配色主题，定义 TUI 所有组件的颜色方案。
///
/// 采用语义化命名（如 `bg_base`、`text_primary`），便于在不同主题间保持一致性。
/// 当前内置两套预设：`deep_ocean`（深蓝海洋）和 `github_dark`（GitHub 暗色）。
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub bg_base: Color,
    pub bg_surface: Color,
    pub bg_overlay: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub text_placeholder: Color,
    pub brand: Color,
    pub user: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub accent_hover: Color,
}

impl Theme {
    /// 深蓝海洋主题：低饱和深色背景，搭配青色品牌色，适合长时间编码。
    pub fn deep_ocean() -> Self {
        Self {
            bg_base: Color::from_u32(0x0d1117),
            bg_surface: Color::from_u32(0x161b22),
            bg_overlay: Color::from_u32(0x1a2332),
            border: Color::from_u32(0x30363d),
            text_primary: Color::from_u32(0xc9d1d9),
            text_secondary: Color::from_u32(0x8b949e),
            text_muted: Color::from_u32(0x484f58),
            text_placeholder: Color::from_u32(0x6e7681),
            brand: Color::from_u32(0x39d0d8),
            user: Color::from_u32(0xf0883e),
            success: Color::from_u32(0x3fb950),
            warning: Color::from_u32(0xd29922),
            error: Color::from_u32(0xf85149),
            selection_bg: Color::from_u32(0x264f78),
            selection_fg: Color::White,
            accent_hover: Color::from_u32(0x58a6ff),
        }
    }

    /// GitHub 暗色主题：基于 deep_ocean 修改品牌色为更亮的蓝色。
    pub fn github_dark() -> Self {
        Self {
            brand: Color::from_u32(0x58a6ff),
            ..Self::deep_ocean()
        }
    }

    /// 从共享的 ThemePreset 构建 Theme。
    pub fn from_preset(preset: &ThemePreset) -> Self {
        Self {
            bg_base: Color::from_u32(preset.bg_base),
            bg_surface: Color::from_u32(preset.bg_surface),
            bg_overlay: Color::from_u32(preset.bg_overlay),
            border: Color::from_u32(preset.border),
            text_primary: Color::from_u32(preset.text_primary),
            text_secondary: Color::from_u32(preset.text_secondary),
            text_muted: Color::from_u32(preset.text_muted),
            text_placeholder: Color::from_u32(preset.text_placeholder),
            brand: Color::from_u32(preset.brand),
            user: Color::from_u32(preset.user),
            success: Color::from_u32(preset.success),
            warning: Color::from_u32(preset.warning),
            error: Color::from_u32(preset.error),
            selection_bg: Color::from_u32(preset.selection_bg),
            selection_fg: Color::from_u32(preset.selection_fg),
            accent_hover: Color::from_u32(preset.accent_hover),
        }
    }

    /// 基础文本样式：主文字色 + 基础背景色。
    pub fn style_primary(&self) -> Style {
        Style::default().fg(self.text_primary).bg(self.bg_base)
    }

    /// 品牌色样式：用于 AI 标识、当前会话高亮等。
    pub fn style_brand(&self) -> Style {
        Style::default().fg(self.brand)
    }

    /// 用户消息样式：橙色，与 AI 品牌色区分。
    pub fn style_user(&self) -> Style {
        Style::default().fg(self.user)
    }

    /// 成功状态样式：绿色。
    pub fn style_success(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// 错误状态样式：红色。
    pub fn style_error(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// 选中高亮样式：反色显示，用于列表选中项。
    pub fn style_selection(&self) -> Style {
        Style::default().fg(self.selection_fg).bg(self.selection_bg)
    }

    /// 弱化文本样式：用于次要信息、占位符。
    pub fn style_muted(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    /// 标题栏区域样式：使用表面背景色区分层级。
    pub fn header_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    /// 抽屉区域样式：与标题栏一致，形成侧边栏视觉。
    pub fn drawer_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    /// 输入框区域样式：表面背景色。
    pub fn input_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    /// 状态栏区域样式：弱化文字，保持底部不突兀。
    pub fn status_bar_style(&self) -> Style {
        self.style_muted().bg(self.bg_base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deep_ocean_colors() {
        let theme = Theme::deep_ocean();
        assert_eq!(theme.brand, Color::from_u32(0x39d0d8));
        assert_eq!(theme.user, Color::from_u32(0xf0883e));
        assert_eq!(theme.success, Color::from_u32(0x3fb950));
    }

    #[test]
    fn test_style_construction() {
        let theme = Theme::deep_ocean();
        let style = theme.style_brand();
        assert_eq!(style.fg, Some(theme.brand));
    }

    #[test]
    fn test_theme_presets() {
        let t1 = Theme::deep_ocean();
        let t2 = Theme::github_dark();
        assert_ne!(t1.brand, t2.brand);
    }
}
