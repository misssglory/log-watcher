use regex::Regex;
use std::{
  path::PathBuf,
  process::Child,
  sync::mpsc::{Receiver, TryRecvError},
  time::Instant,
};

use ratatui::text::Line;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
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
    }
  }

  pub fn as_str(self) -> &'static str {
    match self {
      LogLevel::Trace => "TRACE",
      LogLevel::Debug => "DEBUG",
      LogLevel::Info => "INFO",
      LogLevel::Warn => "WARN",
      LogLevel::Error => "ERROR",
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

#[derive(Debug, Clone, Default)]
pub struct SearchState {
  pub regex: Option<Regex>,
  pub pattern: String,
  pub active_match_line: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct RenderedLine {
  pub line: Line<'static>,
  pub source_entry_idx: usize,
  pub source_real_line_no: usize,
  pub is_first_visual_line: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecentItem {
  File(PathBuf),
  Folder(PathBuf),
  Command(String),
}

#[derive(Debug)]
pub struct CommandStream {
  pub command: String,
  pub child: Child,
  pub rx: Receiver<String>,
  pub finished: bool,
}

#[derive(Debug)]
pub enum TabSource {
  File(PathBuf),
  Folder(PathBuf),
  Command(CommandStream),
}

#[derive(Debug)]
pub struct FilterProgress {
  pub done: usize,
  pub total: usize,
}

pub enum FilterUpdate {
  Progress(FilterProgress),
  Complete { filtered_indices: Vec<usize>, delete_matches: usize },
}

pub struct FilterJob {
  pub rx: Receiver<FilterUpdate>,
  pub progress: FilterProgress,
}

impl std::fmt::Debug for FilterJob {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FilterJob")
      .field(
        "progress",
        &format_args!("{}/{}", self.progress.done, self.progress.total),
      )
      .finish_non_exhaustive()
  }
}

pub struct PagingState {
  pub loaded_files: usize,
  pub total_files: usize,
  pub truncated_files: usize,
  pub max_lines_per_file: usize,
}

pub struct LogTab {
  pub name: String,
  pub source: TabSource,
  pub entries: Vec<LogEntry>,
  pub filtered_indices: Vec<usize>,
  pub rendered_lines: Vec<RenderedLine>,
  pub last_render_width: usize,
  pub filters: Filters,
  pub delete_preview: DeletePreview,
  pub scroll: ScrollState,
  pub last_update: Instant,
  pub auto_refresh: bool,
  pub pretty_print: bool,
  pub search: SearchState,
  pub filter_job: Option<FilterJob>,
  pub paging: Option<PagingState>,
}

impl LogTab {
  pub fn title(&self) -> String {
    match &self.source {
      TabSource::File(path) => path.to_string_lossy().to_string(),
      TabSource::Folder(path) => format!("folder {}", path.to_string_lossy()),
      TabSource::Command(stream) => format!("$ {}", stream.command),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
  Normal,
  FilterRegex,
  DeleteRegex,
  SearchRegex,
  ConfirmDelete,
  JumpToLine,
  OpenFile,
  OpenCommand,
  RecentPicker,
  CommandOverlay,
}

pub struct App {
  pub tabs: Vec<LogTab>,
  pub selected_tab: usize,
  pub should_quit: bool,
  pub input_mode: InputMode,
  pub input_buffer: String,
  pub input_cursor: usize,
  pub status: String,
  pub recents: Vec<RecentItem>,
  pub recent_selected: usize,
}

impl FilterJob {
  pub fn drain(&mut self) -> Option<(Vec<usize>, usize)> {
    let mut completed = None;
    loop {
      match self.rx.try_recv() {
        Ok(FilterUpdate::Progress(progress)) => self.progress = progress,
        Ok(FilterUpdate::Complete { filtered_indices, delete_matches }) => {
          self.progress.done = self.progress.total;
          completed = Some((filtered_indices, delete_matches));
        }
        Err(TryRecvError::Empty) => break,
        Err(TryRecvError::Disconnected) => break,
      }
    }
    completed
  }

  pub fn percent(&self) -> u16 {
    if self.progress.total == 0 {
      return 100;
    }
    ((self.progress.done.saturating_mul(100) / self.progress.total).min(100))
      as u16
  }
}
