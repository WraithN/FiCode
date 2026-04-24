pub mod app;
pub mod client;
pub mod ui;

use app::TuiApp;

pub async fn run_tui() -> anyhow::Result<()> {
    let mut terminal = ratatui::init();
    terminal.clear()?;

    let mut app = TuiApp::new();
    let result = app.run(&mut terminal).await;

    ratatui::restore();
    result
}
