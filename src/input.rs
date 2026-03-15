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
        KeyCode::BackTab => app.selected_tab = (app.selected_tab + app.tabs.len() - 1) % app.tabs.len(),
        KeyCode::Down | KeyCode::Char('j') => app.current_tab_mut().scroll.offset = app.current_tab().scroll.offset.saturating_add(1),
        KeyCode::Up | KeyCode::Char('k') => app.current_tab_mut().scroll.offset = app.current_tab().scroll.offset.saturating_sub(1),
        KeyCode::PageDown | KeyCode::Char('f') => app.current_tab_mut().scroll.offset = app.current_tab().scroll.offset.saturating_add(20),
        KeyCode::PageUp | KeyCode::Char('b') => app.current_tab_mut().scroll.offset = app.current_tab().scroll.offset.saturating_sub(20),
        KeyCode::Home | KeyCode::Char('g') => app.current_tab_mut().scroll.offset = 0,
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
        }
        KeyCode::Char('l') => {
            let tab = app.current_tab_mut();
            tab.filters.min_level = match tab.filters.min_level {
                None => Some(LogLevel::Trace),
                Some(level) => level.next(),
            };
            filter::recompute_tab(tab);
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
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => { app.input_buffer.pop(); }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => app.input_buffer.push(c),
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
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => { app.input_buffer.pop(); }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => app.input_buffer.push(c),
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
