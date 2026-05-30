use regex::Regex;
use std::{
  path::{Path, PathBuf},
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
  pub target: Option<String>,
  pub file: Option<String>,
  pub file_line: Option<usize>,
  pub thread_id: Option<String>,
  pub thread_name: Option<String>,
  pub message: String,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
  pub raw: String,
  pub level: LogLevel,
  pub parsed: ParsedPrefix,
  pub source_file: Option<String>,
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

#[derive(Debug, Clone)]
pub struct DisplayOptions {
  pub timestamp: bool,
  pub level: bool,
  pub target: bool,
  pub file: bool,
  pub thread_id: bool,
}

impl Default for DisplayOptions {
  fn default() -> Self {
    Self {
      timestamp: true,
      level: true,
      target: true,
      file: true,
      thread_id: false,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct FolderFile {
  pub path: PathBuf,
  pub label: String,
}

#[derive(Debug, Clone, Default)]
pub struct FolderState {
  pub files: Vec<FolderFile>,
  pub selected: usize,
  pub picker_selected: usize,
  pub follow_newest: bool,
}

impl FolderState {
  pub fn current_file(&self) -> Option<&FolderFile> {
    self.files.get(self.selected)
  }

  pub fn select_by_path(&mut self, path: &Path) {
    if let Some(pos) = self.files.iter().position(|file| file.path == path) {
      self.selected = pos;
    } else {
      self.selected = 0;
    }
    self.picker_selected = self.selected;
  }
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
  pub source_file: Option<String>,
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
  pub display: DisplayOptions,
  pub search: SearchState,
  pub folder: Option<FolderState>,
  pub filter_job: Option<FilterJob>,
  pub paging: Option<PagingState>,
}

impl LogTab {
  pub fn title(&self) -> String {
    match &self.source {
      TabSource::File(path) => path.to_string_lossy().to_string(),
      TabSource::Folder(path) => {
        let current = self
          .folder
          .as_ref()
          .and_then(|folder| folder.current_file())
          .map(|file| file.label.as_str())
          .unwrap_or("no files");
        format!("folder {} — {current}", path.to_string_lossy())
      }
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
  FieldMenu,
  FolderFilePicker,
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
