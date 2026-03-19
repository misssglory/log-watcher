use std::{fs, path::PathBuf, sync::OnceLock, time::Instant};

use anyhow::Result;
use regex::Regex;

use crate::{
  file_source, filter,
  model::{
    App, DeletePreview, Filters, InputMode, LogEntry, LogLevel, LogTab,
    ParsedPrefix, ScrollState, SearchState,
  },
};

impl App {
  pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
    let mut tabs = Vec::new();

    for path in paths {
      let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

      let content = fs::read_to_string(&path).unwrap_or_default();
      let entries = content.lines().map(build_entry).collect::<Vec<_>>();

      let mut tab = LogTab {
        name,
        path,
        entries,
        filtered_indices: Vec::new(),
        filters: Filters::default(),
        delete_preview: DeletePreview::default(),
        scroll: ScrollState { offset: 0, follow_bottom: true },
        last_update: Instant::now(),
        auto_refresh: true,
        pretty_print: true,
        search: SearchState::default(),
      };

      filter::recompute_tab(&mut tab);
      tabs.push(tab);
    }

    Ok(Self {
      tabs,
      selected_tab: 0,
      should_quit: false,
      input_mode: InputMode::Normal,
      input_buffer: String::new(),
      status: "Ready".into(),
    })
  }

  pub fn current_tab(&self) -> &LogTab {
    &self.tabs[self.selected_tab]
  }

  pub fn current_tab_mut(&mut self) -> &mut LogTab {
    &mut self.tabs[self.selected_tab]
  }

  pub fn refresh_current(&mut self) -> Result<()> {
    let tab = self.current_tab_mut();
    file_source::reload_tab(tab)?;
    self.status = format!("Updated {}", tab.name);
    Ok(())
  }

  pub fn refresh_all(&mut self) -> Result<()> {
    for tab in &mut self.tabs {
      file_source::reload_tab(tab)?;
    }
    self.status = "Updated all tabs".into();
    Ok(())
  }

  pub fn poll_file_updates(&mut self) -> Result<()> {
    for tab in &mut self.tabs {
      if tab.auto_refresh && file_source::has_file_changed(tab)? {
        file_source::reload_tab(tab)?;
      }
    }
    Ok(())
  }

  pub fn set_include_regex(&mut self, pattern: &str) -> Result<()> {
    let re = Regex::new(pattern)?;
    let tab = self.current_tab_mut();
    tab.filters.include_regex = Some(re);
    filter::recompute_tab(tab);
    Ok(())
  }

  pub fn set_delete_regex(&mut self, pattern: &str) -> Result<()> {
    let re = Regex::new(pattern)?;
    let tab = self.current_tab_mut();
    tab.filters.delete_regex = Some(re);
    filter::recompute_delete_preview(tab);
    Ok(())
  }

  pub fn set_search_regex(&mut self, pattern: &str) -> Result<()> {
    let tab = self.current_tab_mut();

    if pattern.is_empty() {
      tab.search.regex = None;
      tab.search.pattern.clear();
      tab.search.active_match_line = None;
      self.status = "Cleared search".into();
      return Ok(());
    }

    let re = Regex::new(pattern)?;
    tab.search.regex = Some(re);
    tab.search.pattern = pattern.to_string();
    tab.search.active_match_line = None;
    self.status = format!("Search regex set: {}", pattern);
    Ok(())
  }

  pub fn goto_next_search_match(&mut self) {
    let tab = self.current_tab_mut();
    let Some(re) = tab.search.regex.clone() else {
      self.status = "No active search".into();
      return;
    };

    if tab.filtered_indices.is_empty() {
      self.status = "No visible lines".into();
      return;
    }

    let len = tab.filtered_indices.len();
    let start = tab.scroll.offset.saturating_add(1) % len;

    for step in 0..len {
      let pos = (start + step) % len;
      let real_idx = tab.filtered_indices[pos];
      let entry = &tab.entries[real_idx];

      if re.is_match(&entry.raw) {
        tab.scroll.offset = pos;
        tab.scroll.follow_bottom = false;
        tab.search.active_match_line = Some(real_idx);
        self.status = format!("Next match at line {}", real_idx + 1);
        return;
      }
    }

    self.status = "No matches".into();
  }

  pub fn goto_prev_search_match(&mut self) {
    let tab = self.current_tab_mut();
    let Some(re) = tab.search.regex.clone() else {
      self.status = "No active search".into();
      return;
    };

    if tab.filtered_indices.is_empty() {
      self.status = "No visible lines".into();
      return;
    }

    let len = tab.filtered_indices.len();
    let start =
      if tab.scroll.offset == 0 { len - 1 } else { tab.scroll.offset - 1 };

    for step in 0..len {
      let pos = (start + len - step) % len;
      let real_idx = tab.filtered_indices[pos];
      let entry = &tab.entries[real_idx];

      if re.is_match(&entry.raw) {
        tab.scroll.offset = pos;
        tab.scroll.follow_bottom = false;
        tab.search.active_match_line = Some(real_idx);
        self.status = format!("Previous match at line {}", real_idx + 1);
        return;
      }
    }

    self.status = "No matches".into();
  }
}

pub fn build_entry(line: &str) -> LogEntry {
  LogEntry {
    raw: line.to_string(),
    level: detect_level(line),
    parsed: parse_prefix(line),
  }
}

pub fn detect_level(line: &str) -> LogLevel {
  let upper = line.to_ascii_uppercase();
  if upper.contains("ERROR") {
    LogLevel::Error
  } else if upper.contains("WARN") {
    LogLevel::Warn
  } else if upper.contains("INFO") {
    LogLevel::Info
  } else if upper.contains("DEBUG") {
    LogLevel::Debug
  } else if upper.contains("TRACE") {
    LogLevel::Trace
  } else {
    LogLevel::Info
  }
}

pub fn parse_prefix(line: &str) -> ParsedPrefix {
  static RE: OnceLock<Regex> = OnceLock::new();

  let re = RE.get_or_init(|| {
        Regex::new(
            r#"^(?P<time>\d{4}-\d{2}-\d{2}T[^\s]+)\s+(?P<level>ERROR|WARN|INFO|DEBUG|TRACE)\s+(?P<file>[^:\s]+):(?P<line>\d+):\s*(?P<msg>.*)$"#
        )
        .unwrap()
    });

  if let Some(caps) = re.captures(line) {
    return ParsedPrefix {
      time: caps.name("time").map(|m| m.as_str().to_string()),
      level_text: caps.name("level").map(|m| m.as_str().to_string()),
      file: caps.name("file").map(|m| m.as_str().to_string()),
      file_line: caps
        .name("line")
        .and_then(|m| m.as_str().parse::<usize>().ok()),
      message: caps
        .name("msg")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default(),
    };
  }

  ParsedPrefix {
    time: None,
    level_text: None,
    file: None,
    file_line: None,
    message: line.to_string(),
  }
}
