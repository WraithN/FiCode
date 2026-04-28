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

use crate::server::sse::SseEvent;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    Resize(u16, u16),
    ToggleLeftDrawer,
    ToggleRightDrawer,
    CloseDrawers,
    FocusNext,
    FocusPrev,
    SetFocus(FocusArea),
    ToggleModelDropdown,
    ToggleThemeDropdown,
    SelectModel(String),
    SelectTheme(usize),
    NewSession,
    NewSessionWithName(String),
    NewSessionFromTemplate(SessionTemplate),
    SubmitMessage(String),
    InputChanged(String),
    ScrollUp,
    ScrollDown,
    CopyLastCode,
    StopGeneration,
    SseEvent(SseEvent),
    ChatComplete,
    ExecuteComplete(String),
    SwitchSession(String),
    DeleteSession(String),
    RenameSession(String, String),
    ToggleFolder(String),
    SelectFile(String),
    OpenFile(String),
    PreviewFile(String),
    AddToContext(String),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    Header,
    Main,
    Input,
    LeftDrawer,
    RightDrawer,
}

#[derive(Debug, Clone)]
pub enum SessionTemplate {
    Empty,
    FromLastContext,
    CodeReview,
    Debug,
}
