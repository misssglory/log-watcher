use ratatui::{
  layout::{Constraint, Direction, Layout},
  style::{Color, Modifier, Style},
  text::{Line, Span},
  widgets::{Block, Borders, Clear, Paragraph, Tabs, Wrap},
  Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::model::App;
use crate::{
  input,
  model::{HistogramRow, InputMode, ViewMode},
};

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
      spans.push(Span::styled("?", key_style()));
      spans.push(Span::styled(" commands", label_style()));
      spans.push(sep());
      spans.push(Span::styled("o", key_style()));
      spans.push(Span::styled(" open file/folder", label_style()));
      spans.push(sep());
      spans.push(Span::styled("/", key_style()));
      spans.push(Span::styled(" filter", label_style()));
      spans.push(sep());
      spans.push(Span::styled("*", key_style()));
      spans.push(Span::styled(" search", label_style()));
      spans.push(sep());
      spans.push(Span::styled("v", key_style()));
      spans.push(Span::styled(" fields", label_style()));
      spans.push(sep());
      spans.push(Span::styled("H", key_style()));
      spans.push(Span::styled(" histogram", label_style()));
      spans.push(sep());
      spans.push(Span::styled("y/Y", key_style()));
      spans.push(Span::styled(" copy lines/hist", label_style()));
      spans.push(sep());
      if tab.folder.is_some() {
        spans.push(Span::styled("F", key_style()));
        spans.push(Span::styled(" files", label_style()));
        spans.push(sep());
        spans.push(Span::styled("m", key_style()));
        spans.push(Span::styled(" newest", label_style()));
        spans.push(sep());
      }
      spans.push(Span::styled("Tab", key_style()));
      spans.push(Span::styled(" tabs", label_style()));
      spans.push(sep());
      spans.push(Span::styled("q", key_style()));
      spans.push(Span::styled(" quit", label_style()));
      spans.push(sep());

      if let Some(job) = &tab.filter_job {
        spans.push(Span::styled("filter ", label_style()));
        spans.push(progress_span(job.percent()));
        spans.push(Span::styled(format!(" {}%", job.percent()), value_style()));
        spans.push(sep());
      }

      if let Some(paging) = &tab.paging {
        spans.push(Span::styled("files=", label_style()));
        spans.push(Span::styled(
          format!("{}/{}", paging.loaded_files, paging.total_files),
          value_style(),
        ));
        if paging.truncated_files > 0 {
          spans.push(Span::styled(
            format!(
              " paged {}×{} lines",
              paging.truncated_files, paging.max_lines_per_file
            ),
            Style::default().fg(Color::Yellow),
          ));
        }
        spans.push(sep());
      }

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
      spans.push(sep());
      spans.push(Span::styled("Esc", key_style()));
      spans.push(Span::styled(" cancel", label_style()));
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
    InputMode::FieldMenu => {
      spans.push(Span::styled("1-5", key_style()));
      spans.push(Span::styled(" toggle display fields", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Esc", key_style()));
      spans.push(Span::styled(" close", label_style()));
    }
    InputMode::FolderFilePicker => {
      spans.push(Span::styled("j/k", key_style()));
      spans.push(Span::styled(" choose file", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Enter", key_style()));
      spans.push(Span::styled(" open", label_style()));
      spans.push(sep());
      spans.push(Span::styled("m", key_style()));
      spans.push(Span::styled(" follow newest", label_style()));
      spans.push(sep());
      spans.push(Span::styled("Esc", key_style()));
      spans.push(Span::styled(" close", label_style()));
    }
    InputMode::CommandOverlay => {
      spans.push(Span::styled("Command overlay", key_style()));
      spans.push(Span::styled(
        " — press any listed key to run it, Esc to close",
        label_style(),
      ));
    }
  }

  Line::from(spans)
}

fn progress_span(percent: u16) -> Span<'static> {
  let width = 12usize;
  let filled = (usize::from(percent) * width / 100).min(width);
  let bar = format!("[{}{}]", "█".repeat(filled), "░".repeat(width - filled));
  Span::styled(
    bar,
    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
  )
}

fn command_overlay_lines() -> Vec<Line<'static>> {
  vec![
    Line::from(Span::styled(
      "Commands",
      Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )),
    Line::from(""),
    legend(
      "Open",
      &[
        ("o", "file or folder tab"),
        ("!", "shell command tab"),
        ("O", "recent picker"),
        ("F", "folder file picker"),
        ("m", "follow newest folder file"),
        ("r/R", "refresh current/all"),
      ],
    ),
    legend(
      "Navigate",
      &[
        ("j/k", "line up/down"),
        ("f/b", "page down/up"),
        ("g/G", "top/bottom"),
        ("Tab", "next tab"),
        ("Shift+Tab", "previous tab"),
        (":", "jump to line"),
      ],
    ),
    legend(
      "Filter",
      &[
        ("/", "include regex"),
        ("l", "cycle min level"),
        ("x", "delete preview regex"),
        ("D", "delete matches"),
        ("c", "clear filters"),
      ],
    ),
    legend(
      "Search",
      &[
        ("*", "search regex"),
        ("n/N", "next/previous match"),
        ("C", "clear search"),
      ],
    ),
    legend(
      "View",
      &[
        ("p", "pretty print"),
        ("v", "field menu"),
        ("s", "follow bottom"),
        ("a", "auto refresh"),
        ("H", "call-site histogram"),
        ("y/Y", "copy lines/histogram"),
      ],
    ),
    legend("General", &[("? or h", "toggle this overlay"), ("q", "quit")]),
  ]
}

fn legend(
  title: &'static str,
  items: &[(&'static str, &'static str)],
) -> Line<'static> {
  let mut spans = vec![Span::styled(format!("{title:<9}"), label_style())];
  for (idx, (key, label)) in items.iter().enumerate() {
    if idx > 0 {
      spans.push(Span::styled("   ", dim_style()));
    }
    spans.push(Span::styled(*key, key_style()));
    spans.push(Span::styled(format!(" {label}"), value_style()));
  }
  Line::from(spans)
}

fn centered_rect(
  percent_x: u16,
  percent_y: u16,
  area: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
  let vertical = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
      Constraint::Percentage((100 - percent_y) / 2),
      Constraint::Percentage(percent_y),
      Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
  Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
      Constraint::Percentage((100 - percent_x) / 2),
      Constraint::Percentage(percent_x),
      Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

fn bool_label(value: bool) -> &'static str {
  if value {
    "on"
  } else {
    "off"
  }
}

fn field_menu_lines(tab: &crate::model::LogTab) -> Vec<Line<'static>> {
  let display = &tab.display;
  vec![
    Line::from(Span::styled(
      "Display fields",
      Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )),
    Line::from(""),
    menu_line("1", "timestamp", bool_label(display.timestamp)),
    menu_line("2", "level", bool_label(display.level)),
    menu_line("3", "target", bool_label(display.target)),
    menu_line("4", "file/line", bool_label(display.file)),
    menu_line("5", "thread id/name", bool_label(display.thread_id)),
  ]
}

fn menu_line(
  key: &'static str,
  name: &'static str,
  value: &'static str,
) -> Line<'static> {
  Line::from(vec![
    Span::styled(format!("{key} "), key_style()),
    Span::styled(format!("{name:<14}"), label_style()),
    Span::styled(value, value_style()),
  ])
}

fn folder_picker_lines(tab: &crate::model::LogTab) -> Vec<Line<'static>> {
  let Some(folder) = &tab.folder else {
    return vec![Line::from(Span::styled(
      "Current tab is not a folder",
      dim_style(),
    ))];
  };
  if folder.files.is_empty() {
    return vec![Line::from(Span::styled("No files in folder", dim_style()))];
  }
  let mut lines = vec![Line::from(vec![
    Span::styled("Files by modification time ", label_style()),
    Span::styled(
      format!("follow newest: {}", bool_label(folder.follow_newest)),
      value_style(),
    ),
  ])];
  for (idx, file) in folder.files.iter().enumerate() {
    let marker = if idx == folder.picker_selected { "> " } else { "  " };
    let suffix = if idx == folder.selected { " (current)" } else { "" };
    let style = if idx == folder.picker_selected {
      Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
      value_style()
    };
    lines.push(Line::from(vec![
      Span::styled(marker, key_style()),
      Span::styled(format!("{}{}", file.label, suffix), style),
    ]));
  }
  lines
}

fn histogram_count_style(count: usize, max_count: usize) -> Style {
  let color = if max_count == 0 {
    Color::DarkGray
  } else {
    let pct = count.saturating_mul(100) / max_count;
    match pct {
      80..=100 => Color::Red,
      55..=79 => Color::Yellow,
      30..=54 => Color::Green,
      _ => Color::Cyan,
    }
  };
  Style::default().fg(color).add_modifier(Modifier::BOLD)
}

fn histogram_lines(rows: &[HistogramRow], width: usize) -> Vec<Line<'static>> {
  if rows.is_empty() {
    return vec![Line::from(Span::styled(
      "No call sites in filtered lines",
      dim_style(),
    ))];
  }

  let max_label = rows.iter().map(|row| row.label.len()).max().unwrap_or(0);
  let max_count = rows.iter().map(|row| row.count).max().unwrap_or(0);
  let count_width = max_count.to_string().len().max(1);
  let bar_width =
    width.saturating_sub(max_label + count_width + 3).min(40).max(1);

  rows
    .iter()
    .map(|row| {
      let style = histogram_count_style(row.count, max_count);
      let filled = if max_count == 0 {
        0
      } else {
        (row.count.saturating_mul(bar_width) / max_count).max(1)
      };
      Line::from(vec![
        Span::styled(format!("{:<max_label$} ", row.label), value_style()),
        Span::styled(format!("{:>count_width$}", row.count), style),
        Span::styled(" ", dim_style()),
        Span::styled("█".repeat(filled), style),
      ])
    })
    .collect::<Vec<_>>()
}

fn recent_picker_lines(app: &App) -> Vec<Line<'static>> {
  if app.recents.is_empty() {
    return vec![Line::from(Span::styled("No recents yet", dim_style()))];
  }

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
  let view_len = match tab.view_mode {
    ViewMode::Logs => tab.rendered_lines.len(),
    ViewMode::CallSiteHistogram => tab.histogram_rows.len(),
  };
  let current = tab.scroll.offset.min(view_len.saturating_sub(1));
  let start_idx = current.saturating_sub(visible_count.saturating_sub(1));
  let end_idx = (start_idx + visible_count).min(view_len);

  let rendered = match app.input_mode {
    InputMode::RecentPicker => recent_picker_lines(app),
    InputMode::FieldMenu => field_menu_lines(tab),
    InputMode::FolderFilePicker => folder_picker_lines(tab),
    _ => match tab.view_mode {
      ViewMode::Logs => tab.rendered_lines[start_idx..end_idx]
        .iter()
        .map(|rl| rl.line.clone())
        .collect::<Vec<_>>(),
      ViewMode::CallSiteHistogram => {
        histogram_lines(&tab.histogram_rows[start_idx..end_idx], viewport_width)
      }
    },
  };

  let title = match tab.view_mode {
    ViewMode::Logs => tab
      .rendered_lines
      .get(current)
      .and_then(|line| line.source_file.as_deref())
      .map(|source_file| format!("{} — {source_file}", tab.name))
      .unwrap_or_else(|| tab.title()),
    ViewMode::CallSiteHistogram => format!(
      "{} — call-site histogram ({} sites, {} lines)",
      tab.name,
      tab.histogram_rows.len(),
      tab.filtered_indices.len()
    ),
  };

  let paragraph = Paragraph::new(rendered)
    .block(Block::default().borders(Borders::ALL).title(title))
    .wrap(Wrap { trim: false });

  frame.render_widget(paragraph, chunks[1]);

  if app.input_mode == InputMode::CommandOverlay {
    let area = centered_rect(92, 55, frame.area());
    let overlay = Paragraph::new(command_overlay_lines())
      .block(Block::default().borders(Borders::ALL).title(" Command palette "))
      .wrap(Wrap { trim: false });
    frame.render_widget(Clear, area);
    frame.render_widget(overlay, area);
  }

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
