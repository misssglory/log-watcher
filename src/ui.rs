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
      Constraint::Length(1),
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
  let current =
    tab.scroll.offset.min(tab.filtered_indices.len().saturating_sub(1));
  let start_idx = current.saturating_sub(visible_count.saturating_sub(1));

  let mut rendered = Vec::new();
  for filtered_pos in start_idx..tab.filtered_indices.len() {
    let real_idx = tab.filtered_indices[filtered_pos];
    let entry = &tab.entries[real_idx];

    let lines = highlight::render_entry_lines(
      real_idx + 1,
      entry,
      viewport_width,
      tab.pretty_print,
      tab.search.regex.as_ref(),
      tab.search.active_match_line == Some(real_idx),
    );

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

  let footer_text = match app.input_mode {
        InputMode::Normal => format!(
            "q quit Tab tabs j/k move g/G top/bot : jump / filter * search n/N next/prev p pretty={} x del D apply r/R refresh a auto={} s follow={} search={} dp={}",
            tab.pretty_print,
            tab.auto_refresh,
            tab.scroll.follow_bottom,
            if tab.search.pattern.is_empty() { "-" } else { &tab.search.pattern },
            tab.delete_preview.matches
        ),
        InputMode::FilterRegex => format!("/ {}", app.input_buffer),
        InputMode::DeleteRegex => format!("x {} ({})", app.input_buffer, tab.delete_preview.matches),
        InputMode::SearchRegex => format!("* {}", app.input_buffer),
        InputMode::ConfirmDelete => {
            format!("D confirm delete {} lines Esc cancel", tab.delete_preview.matches)
        }
        InputMode::JumpToLine => format!(": {}", app.input_buffer),
        InputMode::Help => {
            "q quit Tab tabs j/k move g/G top/bot : jump / filter * search n/N nav p pretty x del D apply".into()
        }
    };

  let footer =
    Paragraph::new(format!("Hotkeys | {} | {}", footer_text, app.status));

  frame.render_widget(footer, chunks[2]);
}
