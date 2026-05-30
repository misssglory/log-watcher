use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
  file_source, filter,
  model::{App, InputMode, LogLevel, RecentItem},
};

fn prev_char_boundary(s: &str, idx: usize) -> usize {
  s[..idx].char_indices().last().map(|(i, _)| i).unwrap_or(0)
}

fn next_char_boundary(s: &str, idx: usize) -> usize {
  if idx >= s.len() {
    s.len()
  } else {
    let mut iter = s[idx..].char_indices();
    iter.next();
    idx + iter.next().map(|(i, _)| i).unwrap_or(s.len() - idx)
  }
}

fn is_word_char(c: char) -> bool {
  c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':')
}

fn prev_word_start(s: &str, idx: usize) -> usize {
  if idx == 0 {
    return 0;
  }

  let chars: Vec<(usize, char)> = s[..idx].char_indices().collect();
  let mut i = chars.len();

  while i > 0 && chars[i - 1].1.is_whitespace() {
    i -= 1;
  }
  while i > 0 && is_word_char(chars[i - 1].1) {
    i -= 1;
  }

  chars.get(i).map(|(pos, _)| *pos).unwrap_or(0)
}

fn next_word_end(s: &str, idx: usize) -> usize {
  if idx >= s.len() {
    return s.len();
  }

  let chars: Vec<(usize, char)> =
    s[idx..].char_indices().map(|(i, c)| (idx + i, c)).collect();

  let mut i = 0;
  while i < chars.len() && chars[i].1.is_whitespace() {
    i += 1;
  }
  while i < chars.len() && is_word_char(chars[i].1) {
    i += 1;
  }

  chars.get(i).map(|(pos, _)| *pos).unwrap_or(s.len())
}

fn insert_char(app: &mut App, c: char) {
  app.input_buffer.insert(app.input_cursor, c);
  app.input_cursor += c.len_utf8();
}

fn delete_prev_char(app: &mut App) {
  if app.input_cursor == 0 {
    return;
  }
  let prev = prev_char_boundary(&app.input_buffer, app.input_cursor);
  app.input_buffer.replace_range(prev..app.input_cursor, "");
  app.input_cursor = prev;
}

fn delete_next_char(app: &mut App) {
  if app.input_cursor >= app.input_buffer.len() {
    return;
  }
  let next = next_char_boundary(&app.input_buffer, app.input_cursor);
  app.input_buffer.replace_range(app.input_cursor..next, "");
}

fn delete_prev_word(app: &mut App) {
  let start = prev_word_start(&app.input_buffer, app.input_cursor);
  app.input_buffer.replace_range(start..app.input_cursor, "");
  app.input_cursor = start;
}

fn delete_next_word(app: &mut App) {
  let end = next_word_end(&app.input_buffer, app.input_cursor);
  app.input_buffer.replace_range(app.input_cursor..end, "");
}

fn move_left(app: &mut App) {
  app.input_cursor = prev_char_boundary(&app.input_buffer, app.input_cursor);
}

fn move_right(app: &mut App) {
  app.input_cursor = next_char_boundary(&app.input_buffer, app.input_cursor);
}

fn move_word_left(app: &mut App) {
  app.input_cursor = prev_word_start(&app.input_buffer, app.input_cursor);
}

fn move_word_right(app: &mut App) {
  app.input_cursor = next_word_end(&app.input_buffer, app.input_cursor);
}

fn handle_text_edit(app: &mut App, key: KeyEvent) -> bool {
  match (key.code, key.modifiers) {
    (KeyCode::Left, m) if m.contains(KeyModifiers::CONTROL) => {
      move_word_left(app);
      true
    }
    (KeyCode::Right, m) if m.contains(KeyModifiers::CONTROL) => {
      move_word_right(app);
      true
    }
    (KeyCode::Left, _) => {
      move_left(app);
      true
    }
    (KeyCode::Right, _) => {
      move_right(app);
      true
    }
    (KeyCode::Backspace, m)
      if m.contains(KeyModifiers::SHIFT)
        || m.contains(KeyModifiers::CONTROL) =>
    {
      delete_prev_word(app);
      true
    }
    (KeyCode::Delete, m)
      if m.contains(KeyModifiers::SHIFT)
        || m.contains(KeyModifiers::CONTROL) =>
    {
      delete_next_word(app);
      true
    }
    (KeyCode::Backspace, _) => {
      delete_prev_char(app);
      true
    }
    (KeyCode::Delete, _) => {
      delete_next_char(app);
      true
    }
    (KeyCode::Home, _) => {
      app.input_cursor = 0;
      true
    }
    (KeyCode::End, _) => {
      app.input_cursor = app.input_buffer.len();
      true
    }
    (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
      insert_char(app, c);
      true
    }
    _ => false,
  }
}

pub fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
  match app.input_mode {
    InputMode::Normal => handle_normal(app, key),
    InputMode::FilterRegex => handle_filter_input(app, key),
    InputMode::DeleteRegex => handle_delete_input(app, key),
    InputMode::SearchRegex => handle_search_input(app, key),
    InputMode::ConfirmDelete => handle_confirm_delete(app, key),
    InputMode::JumpToLine => handle_jump_input(app, key),
    InputMode::OpenFile => handle_open_file_input(app, key),
    InputMode::OpenCommand => handle_open_command_input(app, key),
    InputMode::RecentPicker => handle_recent_picker(app, key),
    InputMode::Help => {
      app.input_mode = InputMode::Normal;
      Ok(())
    }
  }
}

fn handle_normal(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
    KeyCode::Tab => app.selected_tab = (app.selected_tab + 1) % app.tabs.len(),
    KeyCode::BackTab => {
      app.selected_tab =
        (app.selected_tab + app.tabs.len() - 1) % app.tabs.len()
    }
    KeyCode::Down | KeyCode::Char('j') => {
      let len = app.current_tab().rendered_lines.len();
      let tab = app.current_tab_mut();
      tab.scroll.offset = (tab.scroll.offset + 1).min(len.saturating_sub(1));
      tab.scroll.follow_bottom = false;
    }
    KeyCode::Up | KeyCode::Char('k') => {
      let tab = app.current_tab_mut();
      tab.scroll.offset = tab.scroll.offset.saturating_sub(1);
      tab.scroll.follow_bottom = false;
    }
    KeyCode::PageDown | KeyCode::Char('f') => {
      let len = app.current_tab().rendered_lines.len();
      let tab = app.current_tab_mut();
      tab.scroll.offset = (tab.scroll.offset + 20).min(len.saturating_sub(1));
      tab.scroll.follow_bottom = false;
    }
    KeyCode::PageUp | KeyCode::Char('b') => {
      let tab = app.current_tab_mut();
      tab.scroll.offset = tab.scroll.offset.saturating_sub(20);
      tab.scroll.follow_bottom = false;
    }
    KeyCode::Home | KeyCode::Char('g') => {
      let tab = app.current_tab_mut();
      tab.scroll.offset = 0;
      tab.scroll.follow_bottom = false;
    }
    KeyCode::End | KeyCode::Char('G') => {
      let len = app.current_tab().rendered_lines.len();
      let tab = app.current_tab_mut();
      tab.scroll.offset = len.saturating_sub(1);
      tab.scroll.follow_bottom = true;
    }
    KeyCode::Char('s') => {
      let tab = app.current_tab_mut();
      tab.scroll.follow_bottom = !tab.scroll.follow_bottom;
    }
    KeyCode::Char('a') => {
      let tab = app.current_tab_mut();
      tab.auto_refresh = !tab.auto_refresh;
    }
    KeyCode::Char('p') => {
      let tab = app.current_tab_mut();
      tab.pretty_print = !tab.pretty_print;
      tab.rendered_lines.clear();
      app.status = format!("Pretty print: {}", tab.pretty_print);
    }
    KeyCode::Char('r') => app.refresh_current()?,
    KeyCode::Char('R') => app.refresh_all()?,
    KeyCode::Char('o') => {
      app.input_buffer.clear();
      app.input_cursor = 0;
      app.input_mode = InputMode::OpenFile;
      app.status = "Open file: type a path, then Enter".into();
    }
    KeyCode::Char('!') => {
      app.input_buffer.clear();
      app.input_cursor = 0;
      app.input_mode = InputMode::OpenCommand;
      app.status = "Open command: type a shell command, then Enter".into();
    }
    KeyCode::Char('O') => {
      app.recent_selected = 0;
      app.input_mode = InputMode::RecentPicker;
    }
    KeyCode::Char('/') => {
      app.input_buffer.clear();
      app.input_cursor = 0;
      app.input_mode = InputMode::FilterRegex;
    }
    KeyCode::Char('x') => {
      app.input_buffer.clear();
      app.input_cursor = 0;
      app.input_mode = InputMode::DeleteRegex;
    }
    KeyCode::Char('*') => {
      app.input_buffer = app.current_tab().search.pattern.clone();
      app.input_cursor = app.input_buffer.len();
      app.input_mode = InputMode::SearchRegex;
    }
    KeyCode::Char('n') => {
      app.goto_next_search_match();
    }
    KeyCode::Char('N') => {
      app.goto_prev_search_match();
    }
    KeyCode::Char(':') => {
      app.input_buffer.clear();
      app.input_cursor = 0;
      app.input_mode = InputMode::JumpToLine;
    }
    KeyCode::Char('D') => {
      if app.current_tab().delete_preview.matches > 0 {
        app.input_mode = InputMode::ConfirmDelete;
      }
    }
    KeyCode::Char('c') => {
      let tab = app.current_tab_mut();
      tab.filters.include_regex = None;
      tab.filters.delete_regex = None;
      tab.filters.min_level = None;
      filter::recompute_tab(tab);
      app.status = "Cleared filters".into();
    }
    KeyCode::Char('C') => {
      let tab = app.current_tab_mut();
      tab.search.regex = None;
      tab.search.pattern.clear();
      tab.search.active_match_line = None;
      tab.rendered_lines.clear();
      app.status = "Cleared search".into();
    }
    KeyCode::Char('l') => {
      let tab = app.current_tab_mut();
      tab.filters.min_level = match tab.filters.min_level {
        None => Some(LogLevel::Trace),
        Some(level) => level.next(),
      };
      filter::recompute_tab(tab);
      app.status = format!("Level filter: {:?}", tab.filters.min_level);
    }
    KeyCode::Char('?') | KeyCode::Char('h') => app.input_mode = InputMode::Help,
    _ => {}
  }
  Ok(())
}

fn handle_filter_input(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Enter => {
      let pattern = app.input_buffer.clone();
      app.set_include_regex(&pattern)?;
      app.status = format!("Filter regex set: {}", pattern);
      app.input_mode = InputMode::Normal;
    }
    _ => {
      handle_text_edit(app, key);
    }
  }
  Ok(())
}

fn handle_delete_input(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Enter => {
      let pattern = app.input_buffer.clone();
      app.set_delete_regex(&pattern)?;
      app.status = format!(
        "Delete preview regex set: {} ({} matches)",
        pattern,
        app.current_tab().delete_preview.matches
      );
      app.input_mode = InputMode::Normal;
    }
    _ => {
      handle_text_edit(app, key);
    }
  }
  Ok(())
}

fn handle_search_input(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Enter => {
      let pattern = app.input_buffer.clone();
      app.set_search_regex(&pattern)?;
      if !pattern.is_empty() {
        app.goto_next_search_match();
      }
      app.input_mode = InputMode::Normal;
    }
    _ => {
      handle_text_edit(app, key);
    }
  }
  Ok(())
}

fn handle_confirm_delete(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Char('D') => {
      let deleted = file_source::delete_matching_lines(app.current_tab_mut())?;
      app.status = format!("Deleted {} lines", deleted);
      app.input_mode = InputMode::Normal;
    }
    _ => {}
  }
  Ok(())
}

fn handle_jump_input(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Enter => {
      if let Ok(target) = app.input_buffer.parse::<usize>() {
        jump_to_real_line(app, target);
      } else {
        app.status = "Invalid line number".into();
      }
      app.input_mode = InputMode::Normal;
    }
    _ => {
      handle_text_edit(app, key);
    }
  }
  Ok(())
}

fn jump_to_real_line(app: &mut App, target: usize) {
  let tab = app.current_tab_mut();

  if let Some(render_pos) = tab
    .rendered_lines
    .iter()
    .position(|l| l.source_real_line_no == target && l.is_first_visual_line)
  {
    tab.scroll.offset = render_pos;
    tab.scroll.follow_bottom = false;
    app.status = format!("Jumped to line {}", target);
    return;
  }

  if let Some(render_pos) = tab
    .rendered_lines
    .iter()
    .position(|l| l.source_real_line_no >= target && l.is_first_visual_line)
  {
    tab.scroll.offset = render_pos;
    tab.scroll.follow_bottom = false;
    app.status = format!("Jumped near line {}", target);
  } else if !tab.rendered_lines.is_empty() {
    tab.scroll.offset = tab.rendered_lines.len() - 1;
    tab.scroll.follow_bottom = false;
    app.status = format!("Line {} is after end of file", target);
  } else {
    app.status = "No visible lines".into();
  }
}

fn handle_open_file_input(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Enter => {
      let path = app.input_buffer.trim();
      if path.is_empty() {
        app.status = "No file path entered".into();
      } else {
        app.open_file_tab(PathBuf::from(path))?;
      }
      app.input_mode = InputMode::Normal;
    }
    _ => {
      handle_text_edit(app, key);
    }
  }
  Ok(())
}

fn handle_open_command_input(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Enter => {
      let command = app.input_buffer.trim().to_string();
      if command.is_empty() {
        app.status = "No command entered".into();
      } else {
        app.open_command_tab(command)?;
      }
      app.input_mode = InputMode::Normal;
    }
    _ => {
      handle_text_edit(app, key);
    }
  }
  Ok(())
}

fn handle_recent_picker(app: &mut App, key: KeyEvent) -> Result<()> {
  match key.code {
    KeyCode::Esc => app.input_mode = InputMode::Normal,
    KeyCode::Down | KeyCode::Char('j') => {
      if !app.recents.is_empty() {
        app.recent_selected =
          (app.recent_selected + 1).min(app.recents.len() - 1);
      }
    }
    KeyCode::Up | KeyCode::Char('k') => {
      app.recent_selected = app.recent_selected.saturating_sub(1);
    }
    KeyCode::Enter => {
      app.open_recent_selected()?;
      app.input_mode = InputMode::Normal;
    }
    KeyCode::Char('d') => {
      if app.recent_selected < app.recents.len() {
        app.recents.remove(app.recent_selected);
        app.recent_selected =
          app.recent_selected.min(app.recents.len().saturating_sub(1));
        if let Err(err) = crate::cache::save_recents(&app.recents) {
          app.status = format!("Failed to save recents: {err}");
        }
      }
    }
    _ => {}
  }
  Ok(())
}

pub fn recent_label(item: &RecentItem) -> String {
  match item {
    RecentItem::File(path) => format!("file {}", path.display()),
    RecentItem::Command(command) => format!("cmd  {command}"),
  }
}
