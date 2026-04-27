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
