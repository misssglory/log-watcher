use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
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

    let titles = app
        .tabs
        .iter()
        .map(|t| {
            let short = if t.name.len() > 12 {
                format!("{}..{}", &t.name[..5], &t.name[t.name.len() - 5..])
            } else {
                t.name.clone()
            };
            let age = t.last_update.elapsed().as_secs();
            Line::from(format!("{short} {age}s"))
        })
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .select(app.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow));

    frame.render_widget(tabs, chunks[0]);

    let tab = app.current_tab();
    let viewport_width = chunks[1].width.saturating_sub(2) as usize;
    let viewport_height = chunks[1].height.saturating_sub(2) as usize;

    let visible_count = viewport_height.max(1);
    let current = tab.scroll.offset.min(tab.filtered_indices.len().saturating_sub(1));
    let start_idx = current.saturating_sub(visible_count.saturating_sub(1));

    let mut rendered = Vec::new();
    for filtered_pos in start_idx..tab.filtered_indices.len() {
        let real_idx = tab.filtered_indices[filtered_pos];
        let entry = &tab.entries[real_idx];
        let lines = highlight::render_entry_lines(real_idx + 1, entry, viewport_width);
        for line in lines {
            rendered.push(line);
            if rendered.len() >= visible_count {
                break;
            }
        }
        if rendered.len() >= visible_count {
            break;
        }
    }

    let paragraph = Paragraph::new(rendered)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(tab.path.to_string_lossy().to_string()),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, chunks[1]);

    let footer = match app.input_mode {
        InputMode::Normal => format!(
            "q quit | Tab tabs | j/k move | g/G top/bottom | : jump | / filter | x delete regex | D delete | r refresh | a auto={} | follow={} | delete-preview={}",
            tab.auto_refresh,
            tab.scroll.follow_bottom,
            tab.delete_preview.matches
        ),
        InputMode::FilterRegex => format!("Filter regex: {}", app.input_buffer),
        InputMode::DeleteRegex => format!(
            "Delete regex: {} | matches={}",
            app.input_buffer, tab.delete_preview.matches
        ),
        InputMode::ConfirmDelete => format!(
            "Press D to confirm deleting {} lines, Esc to cancel",
            tab.delete_preview.matches
        ),
        InputMode::JumpToLine => format!("Jump to line: {}", app.input_buffer),
        InputMode::Help => "Hotkeys: q quit, Tab switch, j/k scroll, g/G top/bottom, : jump, / include regex, x delete regex, D confirm delete".into(),
    };

    let footer = Paragraph::new(format!("{} | {}", footer, app.status))
        .block(Block::default().title("Hotkeys"));
    frame.render_widget(footer, chunks[2]);
}
