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

use ratatui::style::{Color, Modifier, Style};

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

    pub fn github_dark() -> Self {
        Self {
            brand: Color::from_u32(0x58a6ff),
            ..Self::deep_ocean()
        }
    }

    pub fn style_primary(&self) -> Style {
        Style::default().fg(self.text_primary).bg(self.bg_base)
    }

    pub fn style_brand(&self) -> Style {
        Style::default().fg(self.brand)
    }

    pub fn style_user(&self) -> Style {
        Style::default().fg(self.user)
    }

    pub fn style_success(&self) -> Style {
        Style::default().fg(self.success)
    }

    pub fn style_error(&self) -> Style {
        Style::default().fg(self.error)
    }

    pub fn style_selection(&self) -> Style {
        Style::default().fg(self.selection_fg).bg(self.selection_bg)
    }

    pub fn style_muted(&self) -> Style {
        Style::default().fg(self.text_muted)
    }

    pub fn header_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    pub fn drawer_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

    pub fn input_style(&self) -> Style {
        self.style_primary().bg(self.bg_surface)
    }

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
