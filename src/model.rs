use regex::Regex;
use std::{path::PathBuf, time::Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Unknown,
}

impl LogLevel {
    pub fn next(self) -> Option<Self> {
        use LogLevel::*;
        match self {
            Trace => Some(Debug),
            Debug => Some(Info),
            Info => Some(Warn),
            Warn => Some(Error),
            Error => None,
            Unknown => Some(Trace),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "TRACE",
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
            LogLevel::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParsedPrefix {
    pub time: Option<String>,
    pub level_text: Option<String>,
    pub file: Option<String>,
    pub file_line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub raw: String,
    pub level: LogLevel,
    pub parsed: ParsedPrefix,
}

#[derive(Debug, Clone, Default)]
pub struct Filters {
    pub min_level: Option<LogLevel>,
    pub include_regex: Option<Regex>,
    pub delete_regex: Option<Regex>,
}

#[derive(Debug, Clone, Default)]
pub struct DeletePreview {
    pub matches: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset: usize,
    pub follow_bottom: bool,
}

#[derive(Debug)]
pub struct LogTab {
    pub name: String,
    pub path: PathBuf,
    pub entries: Vec<LogEntry>,
    pub filtered_indices: Vec<usize>,
    pub filters: Filters,
    pub delete_preview: DeletePreview,
    pub scroll: ScrollState,
    pub last_update: Instant,
    pub auto_refresh: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    FilterRegex,
    DeleteRegex,
    ConfirmDelete,
    JumpToLine,
    Help,
}

pub struct App {
    pub tabs: Vec<LogTab>,
    pub selected_tab: usize,
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub status: String,
}
