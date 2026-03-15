use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::{
    highlight,
    model::{App, InputMode},
};

pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let titles = app.tabs.iter().map(|t| {
        let short = if t.name.len() > 12 {
            format!("{}..{}", &t.name[..5], &t.name[t.name.len()-5..])
        } else {
            t.name.clone()
        };
        let age = t.last_update.elapsed().as_secs();
        Line::from(format!("{short} {age}s"))
    }).collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .select(app.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow));

    frame.render_widget(tabs, chunks[0]);

    let tab = app.current_tab();
    let height = chunks[1].height.saturating_sub(2) as usize;
    let start = tab.scroll.offset.saturating_sub(height.saturating_sub(1));
    let end = (start + height).min(tab.filtered_indices.len());

    let mut lines = Vec::new();
    for idx in start..end {
        let real_idx = tab.filtered_indices[idx];
        let entry = &tab.entries[real_idx];
        lines.push(highlight::render_log_line(real_idx + 1, &entry.raw, entry.level));
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(tab.path.to_string_lossy().to_string()))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, chunks[1]);

    let footer = match app.input_mode {
        InputMode::Normal => format!(
            "q quit | Tab next tab | j/k scroll | G bottom | s follow={} | / filter | x delete regex | D delete | r refresh | a auto={} | delete preview={}",
            tab.scroll.follow_bottom,
            tab.auto_refresh,
            tab.delete_preview.matches,
        ),
        InputMode::FilterRegex => format!("Filter regex: {}", app.input_buffer),
        InputMode::DeleteRegex => format!("Delete regex: {} | matches={}", app.input_buffer, tab.delete_preview.matches),
        InputMode::ConfirmDelete => format!("Press D to confirm deleting {} lines, Esc to cancel", tab.delete_preview.matches),
        InputMode::Help => "Help: q quit, Tab switch, j/k scroll, g/G top/bottom, / include regex, x delete regex, D confirm delete".into(),
    };

    let footer = Paragraph::new(footer)
        .block(Block::default().borders(Borders::empty()).title(Span::styled("Hotkeys", Style::default().fg(Color::Cyan))));
    frame.render_widget(footer, chunks[2]);
}
