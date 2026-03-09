mod api;
mod app;
mod ui;

use app::{App, ComposeField, Mode};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use tokio::sync::mpsc;

// ── Config ──────────────────────────────────────────────────────────

fn get_config() -> (String, String) {
    let url = std::env::var("BMS_URL")
        .or_else(|_| std::env::var("FLEET_MAIL_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:8025".to_string());
    let token = std::env::var("BMS_TOKEN")
        .expect("Set BMS_TOKEN to your bearer token");
    (url, token)
}

// ── Event Types ─────────────────────────────────────────────────────

enum AppEvent {
    Key(KeyEvent),
    WsMessage(api::WsEvent),
    Tick,
}

// ── Main ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (url, token) = get_config();
    let client = api::ApiClient::new(&url, &token);

    // Get account info
    let me = client.get_me().await?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new(me.name, me.id);

    // Initial data load
    app.refresh_labels(&client).await;
    app.refresh_threads(&client).await;

    // Event channel
    let (tx, mut rx) = mpsc::unbounded_channel();

    // Spawn terminal event reader
    let tx_key = tx.clone();
    tokio::task::spawn_blocking(move || {
        loop {
            if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if tx_key.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Spawn WebSocket listener (reconnects on failure)
    let tx_ws = tx.clone();
    let ws_url = url.clone();
    let ws_token = token.clone();
    tokio::spawn(async move {
        loop {
            match api::connect_ws(&ws_url, &ws_token).await {
                Ok(mut read) => {
                    while let Some(Ok(msg)) = read.next().await {
                        if let Some(event) = api::parse_ws_event(&msg) {
                            if tx_ws.send(AppEvent::WsMessage(event)).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(_) => {}
            }
            // Reconnect delay
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    });

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        match rx.recv().await {
            Some(AppEvent::Key(key)) => {
                handle_key(&mut app, key, &client, &mut terminal).await?;
            }
            Some(AppEvent::WsMessage(event)) => {
                if event.event_type == "new_message" {
                    app.refresh_threads(&client).await;
                    app.refresh_labels(&client).await;
                    if let (Some(from), Some(subject)) = (&event.from, &event.subject) {
                        app.status = format!("New: {} — {}", app.display_name(from), subject);
                    }
                }
            }
            Some(AppEvent::Tick) => {}
            None => break,
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

// ── Key Handling ────────────────────────────────────────────────────

async fn handle_key(
    app: &mut App,
    key: KeyEvent,
    api: &api::ApiClient,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> anyhow::Result<()> {
    match app.mode {
        Mode::Normal => handle_normal_key(app, key, api).await,
        Mode::Reading => handle_reading_key(app, key, api).await,
        Mode::Compose => handle_compose_key(app, key, api, terminal).await?,
        Mode::Search => handle_search_key(app, key, api).await,
    }
    Ok(())
}

async fn handle_normal_key(app: &mut App, key: KeyEvent, api: &api::ApiClient) {
    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            if app.thread_index + 1 < app.threads.len() {
                app.thread_index += 1;
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.thread_index = app.thread_index.saturating_sub(1);
        }
        KeyCode::Char('g') => {
            // gg = go to top (simplified: just go to top)
            app.thread_index = 0;
        }
        KeyCode::Char('G') => {
            if !app.threads.is_empty() {
                app.thread_index = app.threads.len() - 1;
            }
        }
        KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
            app.open_thread(api).await;
        }
        KeyCode::Char('c') => app.start_compose(),
        KeyCode::Char('/') => {
            app.search_input.clear();
            app.mode = Mode::Search;
        }
        KeyCode::Char('r') => {
            app.refresh_threads(api).await;
            app.refresh_labels(api).await;
            app.status = "Refreshed".to_string();
        }
        // Quick label switching with number keys
        KeyCode::Char('1') => app.switch_label("INBOX", api).await,
        KeyCode::Char('2') => app.switch_label("SENT", api).await,
        KeyCode::Char('3') => app.switch_label("STARRED", api).await,
        KeyCode::Char('4') => app.switch_label("TRASH", api).await,
        KeyCode::Char('5') => app.switch_label("UNREAD", api).await,
        _ => {}
    }
}

async fn handle_reading_key(app: &mut App, key: KeyEvent, api: &api::ApiClient) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
            app.mode = Mode::Normal;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.scroll_offset = app.scroll_offset.saturating_add(1);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.scroll_offset = app.scroll_offset.saturating_sub(1);
        }
        KeyCode::Char('r') => app.start_reply(),
        KeyCode::Char('e') => app.archive_selected(api).await,
        KeyCode::Char('s') => app.star_selected(api).await,
        KeyCode::Char('#') => app.trash_selected(api).await,
        KeyCode::Char('c') => app.start_compose(),
        KeyCode::Char('g') => app.scroll_offset = 0,
        KeyCode::Char('G') => app.scroll_offset = 999,
        _ => {}
    }
}

async fn handle_compose_key(
    app: &mut App,
    key: KeyEvent,
    api: &api::ApiClient,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> anyhow::Result<()> {
    // Ctrl-G: open editor
    if key.code == KeyCode::Char('g') && key.modifiers.contains(KeyModifiers::CONTROL) {
        // Restore terminal for editor
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        app.open_editor()?;

        // Re-enter TUI
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        enable_raw_mode()?;
        terminal.clear()?;
        return Ok(());
    }

    // Ctrl-S: send
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.send_compose(api).await;
        return Ok(());
    }

    match key.code {
        KeyCode::Esc => {
            app.compose.clear();
            app.mode = if app.thread_detail.is_some() { Mode::Reading } else { Mode::Normal };
        }
        KeyCode::Tab => {
            app.compose.field = match app.compose.field {
                ComposeField::To => ComposeField::Cc,
                ComposeField::Cc => ComposeField::Subject,
                ComposeField::Subject => ComposeField::Body,
                ComposeField::Body => ComposeField::To,
            };
        }
        KeyCode::BackTab => {
            app.compose.field = match app.compose.field {
                ComposeField::To => ComposeField::Body,
                ComposeField::Cc => ComposeField::To,
                ComposeField::Subject => ComposeField::Cc,
                ComposeField::Body => ComposeField::Subject,
            };
        }
        KeyCode::Char(c) => {
            if app.compose.field == ComposeField::Body {
                let line = app.compose.cursor_line;
                if line < app.compose.body.len() {
                    let col = app.compose.cursor_col.min(app.compose.body[line].len());
                    app.compose.body[line].insert(col, c);
                    app.compose.cursor_col = col + 1;
                }
            } else if let Some(field) = app.compose.active_field_mut() {
                field.push(c);
            }
        }
        KeyCode::Backspace => {
            if app.compose.field == ComposeField::Body {
                let line = app.compose.cursor_line;
                if app.compose.cursor_col > 0 && line < app.compose.body.len() {
                    app.compose.cursor_col -= 1;
                    app.compose.body[line].remove(app.compose.cursor_col);
                } else if line > 0 {
                    // Merge with previous line
                    let current = app.compose.body.remove(line);
                    app.compose.cursor_line -= 1;
                    app.compose.cursor_col = app.compose.body[app.compose.cursor_line].len();
                    app.compose.body[app.compose.cursor_line].push_str(&current);
                }
            } else if let Some(field) = app.compose.active_field_mut() {
                field.pop();
            }
        }
        KeyCode::Enter => {
            if app.compose.field == ComposeField::Body {
                let line = app.compose.cursor_line;
                let col = app.compose.cursor_col.min(app.compose.body[line].len());
                let rest = app.compose.body[line][col..].to_string();
                app.compose.body[line].truncate(col);
                app.compose.body.insert(line + 1, rest);
                app.compose.cursor_line += 1;
                app.compose.cursor_col = 0;
            } else {
                // Enter in header field = move to next field
                app.compose.field = match app.compose.field {
                    ComposeField::To => ComposeField::Cc,
                    ComposeField::Cc => ComposeField::Subject,
                    ComposeField::Subject => ComposeField::Body,
                    ComposeField::Body => ComposeField::Body,
                };
            }
        }
        KeyCode::Up => {
            if app.compose.field == ComposeField::Body && app.compose.cursor_line > 0 {
                app.compose.cursor_line -= 1;
                let max_col = app.compose.body[app.compose.cursor_line].len();
                app.compose.cursor_col = app.compose.cursor_col.min(max_col);
            }
        }
        KeyCode::Down => {
            if app.compose.field == ComposeField::Body
                && app.compose.cursor_line + 1 < app.compose.body.len()
            {
                app.compose.cursor_line += 1;
                let max_col = app.compose.body[app.compose.cursor_line].len();
                app.compose.cursor_col = app.compose.cursor_col.min(max_col);
            }
        }
        KeyCode::Left => {
            if app.compose.field == ComposeField::Body && app.compose.cursor_col > 0 {
                app.compose.cursor_col -= 1;
            }
        }
        KeyCode::Right => {
            if app.compose.field == ComposeField::Body {
                let max = app.compose.body.get(app.compose.cursor_line).map(|l| l.len()).unwrap_or(0);
                if app.compose.cursor_col < max {
                    app.compose.cursor_col += 1;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

async fn handle_search_key(app: &mut App, key: KeyEvent, api: &api::ApiClient) {
    match key.code {
        KeyCode::Esc => {
            app.search_input.clear();
            app.mode = Mode::Normal;
        }
        KeyCode::Enter => {
            app.execute_search(api).await;
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
        }
        KeyCode::Backspace => {
            app.search_input.pop();
        }
        _ => {}
    }
}
