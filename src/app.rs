use std::{fs, path::PathBuf, time::Instant};

use anyhow::Result;
use regex::Regex;

use crate::{
    file_source,
    filter,
    model::{App, DeletePreview, Filters, InputMode, LogEntry, LogLevel, LogTab, ScrollState},
};

impl App {
    pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
        let mut tabs = Vec::new();

        for path in paths {
            let name = path.file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            let content = fs::read_to_string(&path).unwrap_or_default();
            let entries = content.lines()
                .map(|line| LogEntry {
                    raw: line.to_string(),
                    level: detect_level(line),
                })
                .collect::<Vec<_>>();

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
                dirty: false,
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
}

pub fn detect_level(line: &str) -> LogLevel {
    let upper = line.to_ascii_uppercase();
    if upper.contains("ERROR") { LogLevel::Error }
    else if upper.contains("WARN") { LogLevel::Warn }
    else if upper.contains("INFO") { LogLevel::Info }
    else if upper.contains("DEBUG") { LogLevel::Debug }
    else if upper.contains("TRACE") { LogLevel::Trace }
    else { LogLevel::Info }
}
