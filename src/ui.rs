use ratatui::{
  layout::{Constraint, Direction, Layout},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
  Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::model::App;
use crate::{input, model::InputMode};

fn key_style() -> Style {
  Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

fn label_style() -> Style {
  Style::default().fg(Color::Cyan)
}

fn value_style() -> Style {
  Style::default().fg(Color::White)
}

fn dim_style() -> Style {
  Style::default().fg(Color::DarkGray)
}

fn bool_style(v: bool) -> Style {
  if v {
    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
  } else {
    Style::default().fg(Color::DarkGray)
  }
}

fn sep() -> Span<'static> {
  Span::styled(" | ", dim_style())
}

fn is_input_mode(mode: InputMode) -> bool {
  matches!(
    mode,
    InputMode::FilterRegex
      | InputMode::DeleteRegex
      | InputMode::SearchRegex
      | InputMode::JumpToLine
      | InputMode::OpenFile
      | InputMode::OpenCommand
  )
}

fn input_prefix(mode: InputMode) -> &'static str {
  match mode {
    InputMode::FilterRegex => "/ ",
    InputMode::DeleteRegex => "x ",
    InputMode::SearchRegex => "* ",
    InputMode::JumpToLine => ": ",
    InputMode::OpenFile => "open file> ",
    InputMode::OpenCommand => "cmd> ",
    _ => "",
  }
}

fn footer_line(app: &App) -> Line<'static> {
  let tab = app.current_tab();
  let mut spans = Vec::new();

  match app.input_mode {
    InputMode::Normal => {
      spans.push(Span::styled("q", key_style()));
      spans.push(Span::styled(" quit", label_style()));
      spans.push(sep());

      spans.push(Span::styled("Tab", key_style()));
      spans.push(Span::styled(" tabs", label_style()));
      spans.push(sep());

      spans.push(Span::styled("o", key_style()));
      spans.push(Span::styled(" open", label_style()));
      spans.push(sep());

      spans.push(Span::styled("!", key_style()));
      spans.push(Span::styled(" cmd", label_style()));
      spans.push(sep());

      spans.push(Span::styled("O", key_style()));
      spans.push(Span::styled(" recents", label_style()));
      spans.push(sep());

      spans.push(Span::styled("j/k", key_style()));
      spans.push(Span::styled(" move", label_style()));
      spans.push(sep());

      spans.push(Span::styled("g/G", key_style()));
      spans.push(Span::styled(" top/bot", label_style()));
      spans.push(sep());

      spans.push(Span::styled(":", key_style()));
      spans.push(Span::styled(" jump", label_style()));
      spans.push(sep());

      spans.push(Span::styled("/", key_style()));
      spans.push(Span::styled(" filter", label_style()));
      spans.push(sep());

      spans.push(Span::styled("*", key_style()));
      spans.push(Span::styled(" search", label_style()));
      spans.push(sep());

      spans.push(Span::styled("n/N", key_style()));
      spans.push(Span::styled(" next/prev", label_style()));
      spans.push(sep());

      spans.push(Span::styled("p", key_style()));
      spans.push(Span::styled(" pretty=", label_style()));
      spans.push(Span::styled(
        format!("{}", tab.pretty_print),
        bool_style(tab.pretty_print),
      ));
      spans.push(sep());

      spans.push(Span::styled("a", key_style()));
      spans.push(Span::styled(" auto=", label_style()));
      spans.push(Span::styled(
        format!("{}", tab.auto_refresh),
        bool_style(tab.auto_refresh),
      ));
      spans.push(sep());

      spans.push(Span::styled("s", key_style()));
      spans.push(Span::styled(" follow=", label_style()));
      spans.push(Span::styled(
        format!("{}", tab.scroll.follow_bottom),
        bool_style(tab.scroll.follow_bottom),
      ));
      spans.push(sep());

      spans.push(Span::styled("search=", label_style()));
      if tab.search.pattern.is_empty() {
        spans.push(Span::styled("-", dim_style()));
      } else {
        spans.push(Span::styled(tab.search.pattern.clone(), value_style()));
      }
      spans.push(sep());

      spans.push(Span::styled("dp=", label_style()));
      spans.push(Span::styled(
        tab.delete_preview.matches.to_string(),
        value_style(),
      ));
      spans.push(sep());

      spans.push(Span::styled("status=", label_style()));
      spans.push(Span::styled(app.status.clone(), value_style()));
    }
    InputMode::FilterRegex
    | InputMode::DeleteRegex
    | InputMode::SearchRegex
    | InputMode::JumpToLine
    | InputMode::OpenFile
    | InputMode::OpenCommand => {
      spans.push(Span::styled(input_prefix(app.input_mode), key_style()));
      spans.push(Span::styled(app.input_buffer.clone(), value_style()));
    }
    InputMode::ConfirmDelete => {
      spans.push(Span::styled("D", key_style()));
      spans.push(Span::styled(" confirm delete ", label_style()));
      spans.push(Span::styled(
        tab.delete_preview.matches.to_string(),
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
      ));
      spans.push(Span::styled(" lines", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Esc", key_style()));
      spans.push(Span::styled(" cancel", label_style()));
    }
    InputMode::RecentPicker => {
      spans.push(Span::styled("j/k", key_style()));
      spans.push(Span::styled(" pick recent", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Enter", key_style()));
      spans.push(Span::styled(" open", label_style()));
      spans.push(sep());
      spans.push(Span::styled("d", key_style()));
      spans.push(Span::styled(" remove", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Esc", key_style()));
      spans.push(Span::styled(" cancel", label_style()));
    }
    InputMode::Help => {
      spans.push(Span::styled("q", key_style()));
      spans.push(Span::styled(" quit", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Tab", key_style()));
      spans.push(Span::styled(" tabs", label_style()));
      spans.push(sep());

      spans.push(Span::styled("o", key_style()));
      spans.push(Span::styled(" open", label_style()));
      spans.push(sep());

      spans.push(Span::styled("!", key_style()));
      spans.push(Span::styled(" cmd", label_style()));
      spans.push(sep());

      spans.push(Span::styled("O", key_style()));
      spans.push(Span::styled(" recents", label_style()));
      spans.push(sep());
      spans.push(Span::styled("j/k", key_style()));
      spans.push(Span::styled(" move", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Ctrl+←/→", key_style()));
      spans.push(Span::styled(" word", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Shift+Bksp/Del", key_style()));
      spans.push(Span::styled(" chunk-del", label_style()));
    }
  }

  Line::from(spans)
}

pub fn render(frame: &mut Frame, app: &mut App) {
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

  let viewport_width = chunks[1].width.saturating_sub(2) as usize;
  let viewport_height = chunks[1].height.saturating_sub(2) as usize;

  app.ensure_rendered_lines(viewport_width);

  let tab = app.current_tab();
  let visible_count = viewport_height.max(1);
  let current =
    tab.scroll.offset.min(tab.rendered_lines.len().saturating_sub(1));
  let start_idx = current.saturating_sub(visible_count.saturating_sub(1));
  let end_idx = (start_idx + visible_count).min(tab.rendered_lines.len());

  let rendered = if app.input_mode == InputMode::RecentPicker {
    if app.recents.is_empty() {
      vec![Line::from(Span::styled("No recents yet", dim_style()))]
    } else {
      app
        .recents
        .iter()
        .enumerate()
        .map(|(idx, item)| {
          let marker = if idx == app.recent_selected { "> " } else { "  " };
          let style = if idx == app.recent_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
          } else {
            value_style()
          };
          Line::from(vec![
            Span::styled(marker, key_style()),
            Span::styled(input::recent_label(item), style),
          ])
        })
        .collect::<Vec<_>>()
    }
  } else {
    tab.rendered_lines[start_idx..end_idx]
      .iter()
      .map(|rl| rl.line.clone())
      .collect::<Vec<_>>()
  };

  let paragraph = Paragraph::new(rendered)
    .block(Block::default().borders(Borders::ALL).title(tab.title()))
    .wrap(Wrap { trim: false });

  frame.render_widget(paragraph, chunks[1]);

  let footer = Paragraph::new(footer_line(app));
  frame.render_widget(footer, chunks[2]);

  if is_input_mode(app.input_mode) {
    let prefix = input_prefix(app.input_mode);
    let prefix_width = UnicodeWidthStr::width(prefix) as u16;
    let cursor_width =
      UnicodeWidthStr::width(&app.input_buffer[..app.input_cursor]) as u16;
    frame.set_cursor_position((
      chunks[2].x + prefix_width + cursor_width,
      chunks[2].y,
    ));
  }
}
