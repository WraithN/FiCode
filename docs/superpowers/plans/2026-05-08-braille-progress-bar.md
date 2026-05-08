# Braille Progress Bar Implementation Plan

&gt; **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the "ready" text status in Header component with a braille progress bar using ratatui-braille-bar library, and complete the status update integration.

**Architecture:** 
- Add ratatui-braille-bar dependency
- Modify Header component to add progress ticker and replace status display with BrailleProgressBar
- Update app.rs to call header.set_status() at appropriate events
- Keep all existing patterns and architecture intact

**Tech Stack:** Rust, ratatui, ratatui-braille-bar, tokio

---

## Task 1: Add ratatui-braille-bar dependency

**Files:**
- Modify: `/home/nan/fi-code/Cargo.toml`

- [ ] **Step 1: Add dependency to Cargo.toml**

Add this line to the `[dependencies]` section:
```toml
ratatui-braille-bar = "0.2.2"
```

- [ ] **Step 2: Verify dependency can be fetched**

Run: `cargo check`
Expected: Compiles successfully (may have warnings about unused imports, that's okay)

- [ ] **Step 3: Commit**

```bash
cd /home/nan/fi-code
git add Cargo.toml
git commit -m "feat: add ratatui-braille-bar dependency"
```

---

## Task 2: Update Header component to add progress ticker

**Files:**
- Modify: `/home/nan/fi-code/src/tui/components/header.rs`

- [ ] **Step 1: Add progress_tick field to Header struct**

Find the Header struct definition (around line 52-60) and add the `progress_tick` field:
```rust
pub struct Header {
    current_model: String,
    session_id: Option<String>,
    menu_state: MenuState,
    providers: Vec<ProviderItem>,
    provider_selected: usize,
    model_selected: Vec<usize>,
    status: HeaderStatus,
    progress_tick: u64, // NEW: for animation
}
```

- [ ] **Step 2: Initialize progress_tick in new()**

Update the `new()` method (around line 63-72) to initialize `progress_tick`:
```rust
pub fn new() -> Self {
    Self {
        current_model: "unknown".to_string(),
        session_id: None,
        menu_state: MenuState::Closed,
        providers: vec![],
        provider_selected: 0,
        model_selected: vec![],
        status: HeaderStatus::Ready,
        progress_tick: 0, // NEW
    }
}
```

- [ ] **Step 3: Implement on_tick() to increment progress_tick**

Update the `on_tick()` method (around line 111):
```rust
pub fn on_tick(&mut self) {
    self.progress_tick = self.progress_tick.wrapping_add(1);
}
```

- [ ] **Step 4: Run tests to verify they still pass**

Run: `cargo test --test header`
Expected: All existing tests pass

- [ ] **Step 5: Commit**

```bash
cd /home/nan/fi-code
git add src/tui/components/header.rs
git commit -m "feat: add progress_tick to Header component"
```

---

## Task 3: Replace status text display with BrailleProgressBar

**Files:**
- Modify: `/home/nan/fi-code/src/tui/components/header.rs`

- [ ] **Step 1: Add necessary imports at the top**

After existing imports, add:
```rust
use ratatui_braille_bar::BrailleProgressBar;
```

- [ ] **Step 2: Rewrite the draw method's status display section**

Find the status display code (around line 158-176). Replace:
```rust
let (status_icon, status_color) = match self.status {
    HeaderStatus::Ready => ("●", theme.success),
    HeaderStatus::Generating => ("⟳", theme.warning),
    HeaderStatus::Streaming => ("⚡", theme.brand),
};
let status = Span::styled(
    format!("{} ready", status_icon),
    Style::default().fg(status_color),
);
```

With this:
```rust
// Calculate progress percentage for animation
let progress = match self.status {
    HeaderStatus::Ready => 1.0,
    HeaderStatus::Generating | HeaderStatus::Streaming => {
        ((self.progress_tick % 100) as f64) / 100.0
    }
};

// Get appropriate color and label
let (status_color, status_label) = match self.status {
    HeaderStatus::Ready => (theme.success, "Ready"),
    HeaderStatus::Generating => (theme.warning, "Generating..."),
    HeaderStatus::Streaming => (theme.brand, "Streaming..."),
};

// Create braille progress bar
let progress_bar = BrailleProgressBar::default()
    .progress(progress)
    .style(Style::default().fg(status_color));
```

- [ ] **Step 3: Update the line construction to use progress bar**

Also replace the line construction part (around line 168-176). Change from:
```rust
let line = Line::from(vec![
    logo,
    Span::raw(" │ "),
    model,
    Span::raw(" │ "),
    status,
]);
```

To this new approach that renders the progress bar separately. First render the text line, then render the progress bar to the right of it:

Replace the entire paragraph rendering block with:
```rust
// Render the main header line (logo | model | [status area placeholder])
let status_placeholder = Span::styled(status_label, Style::default().fg(status_color));
let line = Line::from(vec![
    logo,
    Span::raw(" │ "),
    model,
    Span::raw(" │ "),
    status_placeholder,
]);

let paragraph = Paragraph::new(line).alignment(Alignment::Left);
frame.render_widget(paragraph, inner);

// Calculate area for progress bar - place it at the right of the status label
let status_width = status_label.chars().count() as u16;
let progress_area = Rect {
    x: inner.x + 14 + status_width, // Logo(6) + │(3) + model_width(approx)
    y: inner.y,
    width: 10,
    height: 1,
};
frame.render_widget(progress_bar, progress_area);
```

Wait - let's simplify and do it cleaner. Actually, let's use a better approach. Let's create a horizontal layout:

```rust
// Split the inner area into left (text) and right (progress bar)
let chunks = ratatui::layout::Layout::horizontal([
    ratatui::layout::Constraint::Min(0),
    ratatui::layout::Constraint::Length(12),
])
.split(inner);

// Render the text part on the left
let line = Line::from(vec![
    logo,
    Span::raw(" │ "),
    model,
    Span::raw(" │ "),
    Span::styled(status_label, Style::default().fg(status_color)),
]);
let paragraph = Paragraph::new(line).alignment(Alignment::Left);
frame.render_widget(paragraph, chunks[0]);

// Render the progress bar on the right
frame.render_widget(progress_bar, chunks[1]);
```

Yes, use the chunk approach.

- [ ] **Step 4: Run cargo check to verify compilation**

Run: `cargo check`
Expected: No compile errors

- [ ] **Step 5: Commit**

```bash
cd /home/nan/fi-code
git add src/tui/components/header.rs
git commit -m "feat: replace status text with BrailleProgressBar"
```

---

## Task 4: Integrate status updates in app.rs

**Files:**
- Modify: `/home/nan/fi-code/src/tui/app.rs`

- [ ] **Step 1: Update SubmitMessage event handler**

Find the `AppEvent::SubmitMessage` arm (around line 684-688) and add the status update:
```rust
AppEvent::SubmitMessage(ref msg) => {
    self.is_generating = true;
    self.header.set_status(HeaderStatus::Generating); // NEW
    self.chat.add_user_message(msg);
    self.start_chat_stream(msg.clone()).await;
}
```

- [ ] **Step 2: Update SseEvent handler**

Find the `AppEvent::SseEvent` arm (around line 690-696). Update it to set Streaming status when content arrives:
```rust
AppEvent::SseEvent(ref sse_event) => {
    self.chat.handle_sse_event(sse_event);
    // NEW: Set Streaming status when content is being received
    if let SseEvent::Content { .. } = sse_event {
        self.header.set_status(HeaderStatus::Streaming);
    }
    if let SseEvent::Done { session_id } = sse_event {
        self.header.set_session_id(session_id.clone());
        self.input.set_session_id(Some(session_id.clone()));
    }
}
```

- [ ] **Step 3: Update ChatComplete handler**

Find the `AppEvent::ChatComplete` arm (around line 697-699) and add status update:
```rust
AppEvent::ChatComplete => {
    self.is_generating = false;
    self.header.set_status(HeaderStatus::Ready); // NEW
}
```

- [ ] **Step 4: Update StopGeneration handler**

Find the `AppEvent::StopGeneration` arm (around line 700-702) and add status update:
```rust
AppEvent::StopGeneration => {
    self.is_generating = false;
    self.header.set_status(HeaderStatus::Ready); // NEW
}
```

- [ ] **Step 5: Ensure HeaderStatus is imported**

Check that `HeaderStatus` is imported. At the top of app.rs, imports should include:
```rust
use crate::tui::components::header::HeaderStatus;
```

If not, add it.

- [ ] **Step 6: Run cargo check to verify compilation**

Run: `cargo check`
Expected: No compile errors

- [ ] **Step 7: Commit**

```bash
cd /home/nan/fi-code
git add src/tui/app.rs
git commit -m "feat: integrate header status updates"
```

---

## Task 5: Test and verify the implementation

**Files:** None - just testing

- [ ] **Step 1: Run the full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run the application (if possible) to visually verify**

(Optional - if in an interactive environment)
Run: `cargo run`
Expected: App starts, Header shows braille progress bar

- [ ] **Step 3: Commit the design doc if not already committed**

```bash
cd /home/nan/fi-code
git commit -m "docs: add braille progress bar design doc"
```

(Note: The design doc was already staged earlier)

---

## Plan Complete!

All tasks are defined with exact code, file paths, and commands.
