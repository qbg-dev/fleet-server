use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use crate::app::{App, ComposeField, Mode};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),     // main area
            Constraint::Length(1),  // status bar
        ])
        .split(f.area());

    if app.mode == Mode::Compose {
        draw_compose(f, app, chunks[0]);
    } else {
        draw_main(f, app, chunks[0]);
    }
    draw_status_bar(f, app, chunks[1]);
}

fn draw_main(f: &mut Frame, app: &App, area: Rect) {
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(area);

    draw_thread_list(f, app, panes[0]);
    draw_right_pane(f, app, panes[1]);
}

fn draw_thread_list(f: &mut Frame, app: &App, area: Rect) {
    let unread = app.label_unread(&app.current_label);
    let title = if unread > 0 {
        format!(" {} ({}) ", app.current_label, unread)
    } else {
        format!(" {} ", app.current_label)
    };

    let focused = app.mode == Mode::Normal;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = app
        .threads
        .iter()
        .enumerate()
        .map(|(i, thread)| {
            let selected = i == app.thread_index;
            let participants: Vec<&str> = thread.participants.iter()
                .map(|p| app.display_name(p))
                .collect();
            let who = participants.join(", ");
            let age = format_age(&thread.last_message_at);
            let count = if thread.message_count > 1 {
                format!(" ({})", thread.message_count)
            } else {
                String::new()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{}{}", who, count),
                    if selected {
                        Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(format!("  {}", age), Style::default().fg(Color::DarkGray)),
            ]);

            let snippet_line = Line::from(vec![
                Span::styled(
                    truncate(&thread.subject, area.width.saturating_sub(4) as usize),
                    Style::default().fg(Color::Yellow),
                ),
            ]);

            let content = vec![line, snippet_line];

            ListItem::new(content).style(
                if selected && focused {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                },
            )
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title),
        );

    f.render_widget(list, area);
}

fn draw_right_pane(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.mode == Mode::Reading;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    if let Some(detail) = &app.thread_detail {
        let mut lines: Vec<Line> = Vec::new();

        // Thread header
        lines.push(Line::from(Span::styled(
            &detail.subject,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )));
        let participants: Vec<&str> = detail.participants.iter()
            .map(|p| app.display_name(p))
            .collect();
        lines.push(Line::from(Span::styled(
            format!("{} messages · {}", detail.message_count, participants.join(", ")),
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));

        // Messages
        for msg in &detail.messages {
            // Separator
            lines.push(Line::from(Span::styled(
                "────────────────────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));

            // Header
            let from_display = app.display_name(&msg.from);
            let age = format_age(&msg.internal_date);
            lines.push(Line::from(vec![
                Span::styled(from_display, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", age), Style::default().fg(Color::DarkGray)),
            ]));

            // Labels
            if !msg.label_ids.is_empty() {
                let label_spans: Vec<Span> = msg.label_ids.iter()
                    .map(|l| Span::styled(format!(" {} ", l), Style::default().fg(Color::Black).bg(label_color(l))))
                    .collect();
                lines.push(Line::from(label_spans));
            }

            // Reply deadline
            if let Some(ref reply_by) = msg.reply_by {
                lines.push(Line::from(Span::styled(
                    format!("Reply by: {}", reply_by),
                    Style::default().fg(Color::Red),
                )));
            }

            lines.push(Line::from(""));

            // Body
            for body_line in msg.body.lines() {
                lines.push(Line::from(body_line.to_string()));
            }
            lines.push(Line::from(""));
        }

        // Apply scroll
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(app.scroll_offset as usize)
            .collect();

        let para = Paragraph::new(visible_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(" Thread "),
            )
            .wrap(Wrap { trim: false });
        f.render_widget(para, area);
    } else {
        let help = vec![
            Line::from(""),
            Line::from(Span::styled("  Select a thread to read", Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from(Span::styled("  j/k    move up/down", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  Enter  open thread", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  c      compose", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  /      search", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  1-5    switch label", Style::default().fg(Color::DarkGray))),
            Line::from(Span::styled("  q      quit", Style::default().fg(Color::DarkGray))),
        ];
        let para = Paragraph::new(help)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(" boring-mail "),
            );
        f.render_widget(para, area);
    }
}

fn draw_compose(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // To
            Constraint::Length(3), // Cc
            Constraint::Length(3), // Subject
            Constraint::Min(5),   // Body
            Constraint::Length(1), // Help
        ])
        .split(area);

    let c = &app.compose;
    let active = |field: ComposeField| -> Style {
        if c.field == field {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    };

    // To
    let to_block = Block::default().borders(Borders::ALL).border_style(active(ComposeField::To)).title(" To ");
    let to_para = Paragraph::new(c.to.as_str()).block(to_block);
    f.render_widget(to_para, chunks[0]);

    // Cc
    let cc_block = Block::default().borders(Borders::ALL).border_style(active(ComposeField::Cc)).title(" Cc ");
    let cc_para = Paragraph::new(c.cc.as_str()).block(cc_block);
    f.render_widget(cc_para, chunks[1]);

    // Subject
    let sub_block = Block::default().borders(Borders::ALL).border_style(active(ComposeField::Subject)).title(" Subject ");
    let sub_para = Paragraph::new(c.subject.as_str()).block(sub_block);
    f.render_widget(sub_para, chunks[2]);

    // Body
    let body_text: String = c.body.join("\n");
    let body_block = Block::default().borders(Borders::ALL).border_style(active(ComposeField::Body)).title(" Body ");
    let body_para = Paragraph::new(body_text).block(body_block).wrap(Wrap { trim: false });
    f.render_widget(body_para, chunks[3]);

    // Help line
    let help = Paragraph::new(Line::from(vec![
        Span::styled(" Tab", Style::default().fg(Color::Cyan)),
        Span::raw(":next  "),
        Span::styled("Ctrl-G", Style::default().fg(Color::Cyan)),
        Span::raw(":editor  "),
        Span::styled("Ctrl-S", Style::default().fg(Color::Cyan)),
        Span::raw(":send  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(":cancel"),
    ]));
    f.render_widget(help, chunks[4]);

    // Show cursor in active field
    if c.field == ComposeField::Body {
        let x = chunks[3].x + 1 + c.cursor_col as u16;
        let y = chunks[3].y + 1 + c.cursor_line as u16;
        f.set_cursor_position((x.min(chunks[3].right() - 2), y.min(chunks[3].bottom() - 2)));
    } else {
        let (chunk, text_len) = match c.field {
            ComposeField::To => (chunks[0], c.to.len()),
            ComposeField::Cc => (chunks[1], c.cc.len()),
            ComposeField::Subject => (chunks[2], c.subject.len()),
            _ => unreachable!(),
        };
        f.set_cursor_position((chunk.x + 1 + text_len as u16, chunk.y + 1));
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        Mode::Normal => "NORMAL",
        Mode::Reading => "READING",
        Mode::Compose => "COMPOSE",
        Mode::Search => "SEARCH",
    };

    let search_display = if app.mode == Mode::Search {
        format!("  /{}█", app.search_input)
    } else {
        String::new()
    };

    let left = format!(
        " {} │ {} │ {} unread{}",
        app.display_name(&app.account_name),
        mode_str,
        app.unread_count(),
        search_display,
    );

    let right = if !app.status.is_empty() {
        format!("{} ", app.status)
    } else {
        String::new()
    };

    let padding = area.width as usize - left.len().min(area.width as usize) - right.len().min(area.width as usize);
    let bar = format!("{}{:>pad$}{}", left, "", right, pad = padding.max(0));

    let para = Paragraph::new(bar)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    f.render_widget(para, area);
}

// ── Helpers ─────────────────────────────────────────────────────────

fn format_age(iso_date: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(iso_date)
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(iso_date, "%Y-%m-%dT%H:%M:%S%.fZ")
            .map(|n| n.and_utc().fixed_offset()))
    else {
        return iso_date.to_string();
    };
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(dt);
    if diff.num_minutes() < 1 { "now".to_string() }
    else if diff.num_minutes() < 60 { format!("{}m", diff.num_minutes()) }
    else if diff.num_hours() < 24 { format!("{}h", diff.num_hours()) }
    else if diff.num_days() < 7 { format!("{}d", diff.num_days()) }
    else { format!("{}w", diff.num_weeks()) }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { s.to_string() }
    else {
        let end = s.char_indices().nth(max.saturating_sub(1)).map(|(i, _)| i).unwrap_or(s.len());
        format!("{}…", &s[..end])
    }
}

fn label_color(label: &str) -> Color {
    match label {
        "INBOX" => Color::Blue,
        "UNREAD" => Color::Cyan,
        "STARRED" => Color::Yellow,
        "SENT" => Color::Green,
        "TRASH" => Color::Red,
        "OVERDUE" => Color::Red,
        "P0" | "P1" => Color::Red,
        "P2" | "P3" => Color::Yellow,
        "ISSUE" | "OPEN" | "IN_PROGRESS" => Color::Magenta,
        "TASK" => Color::Cyan,
        _ => Color::Gray,
    }
}
