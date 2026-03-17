use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    file_source,
    filter,
    model::{App, InputMode, LogLevel},
};

pub fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match app.input_mode {
        InputMode::Normal => handle_normal(app, key),
        InputMode::FilterRegex => handle_filter_input(app, key),
        InputMode::DeleteRegex => handle_delete_input(app, key),
        InputMode::ConfirmDelete => handle_confirm_delete(app, key),
        InputMode::JumpToLine => handle_jump_input(app, key),
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
            app.selected_tab = (app.selected_tab + app.tabs.len() - 1) % app.tabs.len()
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = app.current_tab().filtered_indices.len();
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
            let len = app.current_tab().filtered_indices.len();
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
            let len = app.current_tab().filtered_indices.len();
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
        KeyCode::Char('r') => app.refresh_current()?,
        KeyCode::Char('R') => app.refresh_all()?,
        KeyCode::Char('/') => {
            app.input_buffer.clear();
            app.input_mode = InputMode::FilterRegex;
        }
        KeyCode::Char('x') => {
            app.input_buffer.clear();
            app.input_mode = InputMode::DeleteRegex;
        }
        KeyCode::Char(':') => {
            app.input_buffer.clear();
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
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_buffer.push(c)
        }
        _ => {}
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
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input_buffer.push(c)
        }
        _ => {}
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
        KeyCode::Backspace => {
            app.input_buffer.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            app.input_buffer.push(c);
        }
        _ => {}
    }
    Ok(())
}

fn jump_to_real_line(app: &mut App, target: usize) {
    let tab = app.current_tab_mut();

    if let Some(pos) = tab.filtered_indices.iter().position(|idx| *idx + 1 == target) {
        tab.scroll.offset = pos;
        tab.scroll.follow_bottom = false;
        app.status = format!("Jumped to line {}", target);
        return;
    }

    if let Some(pos) = tab.filtered_indices.iter().position(|idx| *idx + 1 >= target) {
        tab.scroll.offset = pos;
        tab.scroll.follow_bottom = false;
        app.status = format!("Jumped near line {}", target);
    } else if !tab.filtered_indices.is_empty() {
        tab.scroll.offset = tab.filtered_indices.len() - 1;
        tab.scroll.follow_bottom = false;
        app.status = format!("Line {} is after end of file", target);
    } else {
        app.status = "No visible lines".into();
    }
}
