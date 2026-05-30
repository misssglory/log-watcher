use std::{
  io::Write,
  io::{BufRead, BufReader},
  path::PathBuf,
  process::{Command, Stdio},
  sync::{mpsc, OnceLock},
  thread,
  time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use regex::Regex;

use crate::{
  cache, file_source, filter, highlight,
  model::{
    App, CommandStream, DeletePreview, DisplayOptions, Filters, HistogramRow,
    InputMode, LogEntry, LogLevel, LogTab, ParsedPrefix, RecentItem,
    RenderedLine, ScrollState, SearchState, TabSource, ViewMode,
  },
};

impl App {
  pub fn new(paths: Vec<PathBuf>) -> Result<Self> {
    let mut app = Self {
      tabs: Vec::new(),
      selected_tab: 0,
      should_quit: false,
      input_mode: InputMode::Normal,
      input_buffer: String::new(),
      input_cursor: 0,
      status: "Ready".into(),
      recents: cache::load_recents(),
      recent_selected: 0,
    };

    for path in paths {
      if let Err(err) = app.open_file_tab(path) {
        app.status = format!("Open failed: {err}");
      }
    }

    if app.tabs.is_empty() {
      app.open_file_tab("app.log".into())?;
    }

    Ok(app)
  }

  fn make_file_tab(path: PathBuf) -> Result<LogTab> {
    let name = path
      .file_name()
      .map(|s| s.to_string_lossy().to_string())
      .unwrap_or_else(|| path.to_string_lossy().to_string());

    let (entries, paging) = file_source::load_file_entries(&path)?;
    let mut tab = Self::make_tab(name, TabSource::File(path), entries, true);
    tab.paging = Some(paging);
    Ok(tab)
  }

  fn make_folder_tab(path: PathBuf) -> Result<LogTab> {
    let name = path
      .file_name()
      .map(|s| format!("{}/", s.to_string_lossy()))
      .unwrap_or_else(|| path.to_string_lossy().to_string());

    let (entries, paging, folder) = file_source::load_folder_entries(&path)?;
    let mut tab = Self::make_tab(name, TabSource::Folder(path), entries, false);
    tab.paging = Some(paging);
    tab.folder = Some(folder);
    Ok(tab)
  }

  fn make_tab(
    name: String,
    source: TabSource,
    entries: Vec<LogEntry>,
    auto_refresh: bool,
  ) -> LogTab {
    let mut tab = LogTab {
      name,
      source,
      entries,
      filtered_indices: Vec::new(),
      rendered_lines: Vec::new(),
      histogram_rows: Vec::new(),
      last_render_width: 0,
      filters: Filters::default(),
      delete_preview: DeletePreview::default(),
      scroll: ScrollState { offset: 0, follow_bottom: true },
      last_update: Instant::now(),
      auto_refresh,
      pretty_print: true,
      view_mode: ViewMode::Logs,
      display: DisplayOptions::default(),
      search: SearchState::default(),
      folder: None,
      filter_job: None,
      paging: None,
    };

    filter::recompute_tab(&mut tab);
    tab
  }

  pub fn current_tab(&self) -> &LogTab {
    &self.tabs[self.selected_tab]
  }

  pub fn current_tab_mut(&mut self) -> &mut LogTab {
    &mut self.tabs[self.selected_tab]
  }

  pub fn open_file_tab(&mut self, path: PathBuf) -> Result<()> {
    if path.is_dir() {
      return self.open_folder_tab(path);
    }

    let tab = Self::make_file_tab(path.clone())?;
    self.tabs.push(tab);
    self.selected_tab = self.tabs.len() - 1;
    cache::remember_recent(&mut self.recents, RecentItem::File(path.clone()));
    let _ = cache::save_recents(&self.recents);
    self.status = format!("Opened {}", path.display());
    Ok(())
  }

  pub fn open_folder_tab(&mut self, path: PathBuf) -> Result<()> {
    let tab = Self::make_folder_tab(path.clone())?;
    self.tabs.push(tab);
    self.selected_tab = self.tabs.len() - 1;
    cache::remember_recent(&mut self.recents, RecentItem::Folder(path.clone()));
    let _ = cache::save_recents(&self.recents);
    self.status = format!("Opened folder {} newest to oldest", path.display());
    Ok(())
  }

  pub fn open_command_tab(&mut self, command: String) -> Result<()> {
    let mut child = Command::new("sh")
      .arg("-c")
      .arg(&command)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel();

    if let Some(stdout) = stdout {
      let tx = tx.clone();
      thread::spawn(move || stream_reader(stdout, tx, LogLevel::Info));
    }

    if let Some(stderr) = stderr {
      thread::spawn(move || stream_reader(stderr, tx, LogLevel::Error));
    }

    let short = command.chars().take(18).collect::<String>();
    let name = if command.chars().count() > 18 {
      format!("$ {short}…")
    } else {
      format!("$ {command}")
    };
    let stream =
      CommandStream { command: command.clone(), child, rx, finished: false };
    let tab =
      Self::make_tab(name, TabSource::Command(stream), Vec::new(), true);
    self.tabs.push(tab);
    self.selected_tab = self.tabs.len() - 1;
    cache::remember_recent(
      &mut self.recents,
      RecentItem::Command(command.clone()),
    );
    let _ = cache::save_recents(&self.recents);
    self.status = format!("Started command: {command}");
    Ok(())
  }

  pub fn open_recent_selected(&mut self) -> Result<()> {
    let Some(item) = self.recents.get(self.recent_selected).cloned() else {
      self.status = "No recents".into();
      return Ok(());
    };

    match item {
      RecentItem::File(path) => self.open_file_tab(path),
      RecentItem::Folder(path) => self.open_folder_tab(path),
      RecentItem::Command(command) => self.open_command_tab(command),
    }
  }

  pub fn refresh_current(&mut self) -> Result<()> {
    let tab = self.current_tab_mut();
    match &tab.source {
      TabSource::File(_) | TabSource::Folder(_) => {
        file_source::reload_tab(tab)?;
        self.status = format!("Updated {}", tab.name);
      }
      TabSource::Command(_) => {
        self.status = "Command tabs update automatically".into();
      }
    }
    Ok(())
  }

  pub fn refresh_all(&mut self) -> Result<()> {
    for tab in &mut self.tabs {
      if matches!(tab.source, TabSource::File(_) | TabSource::Folder(_)) {
        file_source::reload_tab(tab)?;
      }
    }
    self.status = "Updated all file tabs".into();
    Ok(())
  }

  pub fn poll_file_updates(&mut self) -> Result<()> {
    for tab in &mut self.tabs {
      if filter::poll_filter_job(tab) {
        self.status =
          format!("Filtered {} visible lines", tab.filtered_indices.len());
      }

      if !tab.auto_refresh {
        continue;
      }

      if matches!(tab.source, TabSource::File(_) | TabSource::Folder(_)) {
        if file_source::has_file_changed(tab)? {
          file_source::reload_tab(tab)?;
        }
        continue;
      }

      let TabSource::Command(stream) = &mut tab.source else {
        continue;
      };

      let mut changed = false;
      while let Ok(line) = stream.rx.try_recv() {
        tab.entries.push(build_entry(&line, None));
        changed = true;
      }

      if !stream.finished {
        if let Some(status) = stream.child.try_wait()? {
          stream.finished = true;
          let level =
            if status.success() { LogLevel::Info } else { LogLevel::Error };
          tab.entries.push(build_entry(
            &format_tracing_line(
              level,
              &format!("command exited with {status}"),
            ),
            None,
          ));
          changed = true;
        }
      }

      if changed {
        tab.last_update = Instant::now();
        filter::recompute_tab(tab);
        tab.rendered_lines.clear();
        tab.last_render_width = 0;
        if tab.scroll.follow_bottom {
          tab.scroll.offset = tab.filtered_indices.len().saturating_sub(1);
        }
      }
    }
    Ok(())
  }

  pub fn toggle_call_site_histogram(&mut self) {
    let tab = self.current_tab_mut();
    tab.view_mode = match tab.view_mode {
      ViewMode::Logs => ViewMode::CallSiteHistogram,
      ViewMode::CallSiteHistogram => ViewMode::Logs,
    };
    tab.scroll.offset = 0;
    tab.scroll.follow_bottom = false;
    self.status = match tab.view_mode {
      ViewMode::Logs => "Showing filtered log lines".into(),
      ViewMode::CallSiteHistogram => format!(
        "Showing {} call sites from {} filtered lines",
        tab.histogram_rows.len(),
        tab.filtered_indices.len()
      ),
    };
  }

  pub fn copy_filtered_lines_to_clipboard(&mut self) -> Result<()> {
    let tab = self.current_tab();
    let lines = tab
      .filtered_indices
      .iter()
      .map(|&idx| tab.entries[idx].raw.as_str())
      .collect::<Vec<_>>()
      .join("\n");
    let count = tab.filtered_indices.len();
    copy_to_clipboard(lines)?;
    self.status = format!("Copied {count} filtered lines to clipboard");
    Ok(())
  }

  pub fn copy_histogram_to_clipboard(&mut self) -> Result<()> {
    let tab = self.current_tab();
    let text = histogram_clipboard_text(&tab.histogram_rows);
    let count = tab.histogram_rows.len();
    copy_to_clipboard(text)?;
    self.status =
      format!("Copied {count} call-site histogram rows to clipboard");
    Ok(())
  }

  pub fn toggle_display_field(&mut self, field: char) {
    let tab = self.current_tab_mut();
    let (name, enabled) = match field {
      '1' => {
        tab.display.timestamp = !tab.display.timestamp;
        ("timestamp", tab.display.timestamp)
      }
      '2' => {
        tab.display.level = !tab.display.level;
        ("level", tab.display.level)
      }
      '3' => {
        tab.display.target = !tab.display.target;
        ("target", tab.display.target)
      }
      '4' => {
        tab.display.file = !tab.display.file;
        ("file", tab.display.file)
      }
      '5' => {
        tab.display.thread_id = !tab.display.thread_id;
        ("thread id", tab.display.thread_id)
      }
      _ => return,
    };
    tab.rendered_lines.clear();
    self.status = format!("Show {name}: {enabled}");
  }

  pub fn select_folder_picker_file(&mut self) -> Result<()> {
    let tab = self.current_tab_mut();
    let Some(folder) = &mut tab.folder else {
      self.status = "Current tab is not a folder".into();
      return Ok(());
    };
    folder.selected =
      folder.picker_selected.min(folder.files.len().saturating_sub(1));
    folder.follow_newest = false;
    file_source::reload_tab(tab)?;
    self.status = tab
      .folder
      .as_ref()
      .and_then(|folder| folder.current_file())
      .map(|file| format!("Selected {}", file.label))
      .unwrap_or_else(|| "No files in folder".into());
    Ok(())
  }

  pub fn toggle_follow_newest_file(&mut self) -> Result<()> {
    let tab = self.current_tab_mut();
    let Some(folder) = &mut tab.folder else {
      self.status = "Current tab is not a folder".into();
      return Ok(());
    };
    folder.follow_newest = !folder.follow_newest;
    if folder.follow_newest {
      folder.selected = 0;
      folder.picker_selected = 0;
      file_source::reload_tab(tab)?;
    }
    self.status = format!(
      "Follow newest file: {}",
      tab.folder.as_ref().is_some_and(|folder| folder.follow_newest)
    );
    Ok(())
  }

  pub fn set_include_regex(&mut self, pattern: &str) -> Result<()> {
    let re = Regex::new(pattern)?;
    let tab = self.current_tab_mut();
    tab.filters.include_regex = Some(re);
    filter::recompute_tab(tab);
    tab.rendered_lines.clear();
    Ok(())
  }

  pub fn set_delete_regex(&mut self, pattern: &str) -> Result<()> {
    let re = Regex::new(pattern)?;
    let tab = self.current_tab_mut();
    tab.filters.delete_regex = Some(re);
    filter::recompute_tab(tab);
    Ok(())
  }

  pub fn set_search_regex(&mut self, pattern: &str) -> Result<()> {
    let tab = self.current_tab_mut();

    if pattern.is_empty() {
      tab.search.regex = None;
      tab.search.pattern.clear();
      tab.search.active_match_line = None;
      tab.rendered_lines.clear();
      self.status = "Cleared search".into();
      return Ok(());
    }

    let re = Regex::new(pattern)?;
    tab.search.regex = Some(re);
    tab.search.pattern = pattern.to_string();
    tab.search.active_match_line = None;
    tab.rendered_lines.clear();
    self.status = format!("Search regex set: {}", pattern);
    Ok(())
  }

  pub fn ensure_rendered_lines(&mut self, width: usize) {
    let tab = self.current_tab_mut();
    if tab.last_render_width == width && !tab.rendered_lines.is_empty() {
      return;
    }

    tab.histogram_rows = build_call_site_histogram(tab);

    let mut lines = Vec::<RenderedLine>::new();

    for real_idx in tab.filtered_indices.clone() {
      let entry = &tab.entries[real_idx];
      let rendered = highlight::render_entry_lines(
        real_idx + 1,
        entry,
        width,
        tab.pretty_print,
        &tab.display,
        tab.search.regex.as_ref(),
        tab.search.active_match_line == Some(real_idx),
      );

      for (i, line) in rendered.into_iter().enumerate() {
        lines.push(RenderedLine {
          line,
          source_entry_idx: real_idx,
          source_real_line_no: real_idx + 1,
          source_file: entry.source_file.clone(),
          is_first_visual_line: i == 0,
        });
      }
    }

    tab.rendered_lines = lines;
    tab.last_render_width = width;

    if tab.scroll.follow_bottom || tab.scroll.offset >= tab.rendered_lines.len()
    {
      tab.scroll.offset = tab.rendered_lines.len().saturating_sub(1);
    }
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
    let mut start_pos = 0usize;

    if let Some(active) = tab.search.active_match_line {
      if let Some(pos) =
        tab.filtered_indices.iter().position(|&idx| idx == active)
      {
        start_pos = (pos + 1) % len;
      }
    } else if let Some(current_line) = tab.rendered_lines.get(tab.scroll.offset)
    {
      if let Some(pos) = tab
        .filtered_indices
        .iter()
        .position(|&idx| idx == current_line.source_entry_idx)
      {
        start_pos = (pos + 1) % len;
      }
    }

    for step in 0..len {
      let pos = (start_pos + step) % len;
      let real_idx = tab.filtered_indices[pos];
      let entry = &tab.entries[real_idx];

      if re.is_match(&entry.raw) {
        tab.search.active_match_line = Some(real_idx);
        if let Some(render_pos) = tab.rendered_lines.iter().position(|l| {
          l.source_entry_idx == real_idx && l.is_first_visual_line
        }) {
          tab.scroll.offset = render_pos;
          tab.scroll.follow_bottom = false;
        }
        tab.rendered_lines.clear();
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
    let mut start_pos = len.saturating_sub(1);

    if let Some(active) = tab.search.active_match_line {
      if let Some(pos) =
        tab.filtered_indices.iter().position(|&idx| idx == active)
      {
        start_pos = if pos == 0 { len - 1 } else { pos - 1 };
      }
    } else if let Some(current_line) = tab.rendered_lines.get(tab.scroll.offset)
    {
      if let Some(pos) = tab
        .filtered_indices
        .iter()
        .position(|&idx| idx == current_line.source_entry_idx)
      {
        start_pos = if pos == 0 { len - 1 } else { pos - 1 };
      }
    }

    for step in 0..len {
      let pos = (start_pos + len - step) % len;
      let real_idx = tab.filtered_indices[pos];
      let entry = &tab.entries[real_idx];

      if re.is_match(&entry.raw) {
        tab.search.active_match_line = Some(real_idx);
        if let Some(render_pos) = tab.rendered_lines.iter().position(|l| {
          l.source_entry_idx == real_idx && l.is_first_visual_line
        }) {
          tab.scroll.offset = render_pos;
          tab.scroll.follow_bottom = false;
        }
        tab.rendered_lines.clear();
        self.status = format!("Previous match at line {}", real_idx + 1);
        return;
      }
    }

    self.status = "No matches".into();
  }
}

pub fn build_call_site_histogram(tab: &LogTab) -> Vec<HistogramRow> {
  use std::collections::BTreeMap;

  let mut counts = BTreeMap::<String, usize>::new();
  for &idx in &tab.filtered_indices {
    let entry = &tab.entries[idx];
    if let Some(label) = call_site_label(entry) {
      *counts.entry(label).or_insert(0) += 1;
    }
  }

  let mut rows = counts
    .into_iter()
    .map(|(label, count)| HistogramRow { label, count })
    .collect::<Vec<_>>();
  rows
    .sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.label.cmp(&b.label)));
  rows
}

fn call_site_label(entry: &LogEntry) -> Option<String> {
  if let Some(target) = entry.parsed.target.as_deref() {
    let leaf = target.rsplit("::").next().unwrap_or(target);
    return match entry.parsed.file_line {
      Some(line) => Some(format!("{leaf}:{line}")),
      None => Some(leaf.to_string()),
    };
  }

  let file = entry.parsed.file.as_deref()?;
  match entry.parsed.file_line {
    Some(line) => Some(format!("{file}:{line}")),
    None => Some(file.to_string()),
  }
}

pub fn histogram_clipboard_text(rows: &[HistogramRow]) -> String {
  rows
    .iter()
    .map(|row| format!("{} {}", row.label, row.count))
    .collect::<Vec<_>>()
    .join("\n")
}

fn copy_to_clipboard(text: String) -> Result<()> {
  let commands: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
    &[("pbcopy", &[])]
  } else if cfg!(target_os = "windows") {
    &[("clip.exe", &[])]
  } else {
    &[
      ("wl-copy", &[]),
      ("xclip", &["-selection", "clipboard"]),
      ("xsel", &["--clipboard", "--input"]),
    ]
  };

  let mut last_error = None;
  for (program, args) in commands {
    match Command::new(program).args(*args).stdin(Stdio::piped()).spawn() {
      Ok(mut child) => {
        if let Some(mut stdin) = child.stdin.take() {
          stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
          return Ok(());
        }
        last_error = Some(anyhow::anyhow!("{program} exited with {status}"));
      }
      Err(err) => last_error = Some(err.into()),
    }
  }

  Err(
    last_error
      .unwrap_or_else(|| anyhow::anyhow!("no clipboard command available")),
  )
}

fn stream_reader<R>(reader: R, tx: mpsc::Sender<String>, level: LogLevel)
where
  R: std::io::Read,
{
  for line in BufReader::new(reader).lines().map_while(Result::ok) {
    if tx.send(format_tracing_line(level, &line)).is_err() {
      break;
    }
  }
}

fn format_tracing_line(level: LogLevel, message: &str) -> String {
  let ts =
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
  format!("1970-01-01T{ts}.000Z {} command:1: {}", level.as_str(), message)
}

pub fn build_entry(line: &str, source_file: Option<&str>) -> LogEntry {
  let parsed = parse_prefix(line);
  let level = parsed
    .level_text
    .as_deref()
    .map(detect_level)
    .unwrap_or_else(|| detect_level(line));

  LogEntry {
    raw: line.to_string(),
    level,
    parsed,
    source_file: source_file.map(str::to_string),
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
  if let Some(parsed) = parse_json_prefix(line) {
    return parsed;
  }

  static RE: OnceLock<Regex> = OnceLock::new();

  let re = RE.get_or_init(|| {
    Regex::new(
        r#"^(?P<time>\d{4}-\d{2}-\d{2}T[^\s]+)\s+(?P<level>ERROR|WARN|INFO|DEBUG|TRACE)\s+(?:(?P<thread>ThreadId\([^)]*\))\s+)?(?P<target>[^\s:]+(?:::[^\s:]+)*):?\s*(?:(?P<file>[^:\s]+):(?P<line>\d+):\s*)?(?P<msg>.*)$"#
    )
    .unwrap()
});

  if let Some(caps) = re.captures(line) {
    return ParsedPrefix {
      time: caps.name("time").map(|m| m.as_str().to_string()),
      level_text: caps.name("level").map(|m| m.as_str().to_string()),
      target: caps
        .name("target")
        .map(|m| m.as_str().trim_end_matches(':').to_string()),
      file: caps.name("file").map(|m| m.as_str().to_string()),
      file_line: caps
        .name("line")
        .and_then(|m| m.as_str().parse::<usize>().ok()),
      thread_id: caps.name("thread").map(|m| m.as_str().to_string()),
      thread_name: None,
      message: caps
        .name("msg")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default(),
    };
  }

  ParsedPrefix {
    time: None,
    level_text: None,
    target: None,
    file: None,
    file_line: None,
    thread_id: None,
    thread_name: None,
    message: line.to_string(),
  }
}

fn parse_json_prefix(line: &str) -> Option<ParsedPrefix> {
  let trimmed = line.trim();
  if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
    return None;
  }

  let time = json_string_field(trimmed, &["timestamp", "time", "ts"]);
  let level_text = json_string_field(trimmed, &["level"]);
  let target = json_string_field(trimmed, &["target"]);
  let file = json_string_field(trimmed, &["filename", "file"]);
  let file_line = json_usize_field(trimmed, &["line_number", "line"]);
  let thread_id =
    json_string_field(trimmed, &["threadId", "thread_id", "thread.id"]);
  let thread_name =
    json_string_field(trimmed, &["threadName", "thread_name", "thread.name"]);

  let message = json_object_field(trimmed, "fields")
    .map(|fields| json_fields_message(&fields))
    .or_else(|| json_string_field(trimmed, &["message"]))
    .unwrap_or_else(|| trimmed.to_string());

  Some(ParsedPrefix {
    time,
    level_text,
    target,
    file,
    file_line,
    thread_id,
    thread_name,
    message,
  })
}

fn json_string_field(input: &str, keys: &[&str]) -> Option<String> {
  keys.iter().find_map(|key| {
    let value = json_field_value(input, key)?;
    Some(json_value_to_message(value))
  })
}

fn json_usize_field(input: &str, keys: &[&str]) -> Option<usize> {
  keys.iter().find_map(|key| {
    let value = json_field_value(input, key)?.trim();
    if let Some(stripped) =
      value.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
    {
      unescape_json_string(stripped).parse().ok()
    } else {
      value
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok()
    }
  })
}

fn json_object_field(input: &str, key: &str) -> Option<String> {
  let value = json_field_value(input, key)?.trim();
  if value.starts_with('{') {
    Some(value.to_string())
  } else {
    None
  }
}

fn json_fields_message(fields: &str) -> String {
  let message = json_string_field(fields, &["message"]);
  let extra = remove_json_object_key(fields, "message");
  match (message, extra.as_deref()) {
    (Some(message), Some("{}")) | (Some(message), None) => message,
    (Some(message), Some(extra)) if !message.is_empty() => {
      format!("{message} {extra}")
    }
    (_, Some(extra)) => extra.to_string(),
    _ => fields.to_string(),
  }
}

fn json_field_value<'a>(input: &'a str, key: &str) -> Option<&'a str> {
  let pattern = format!("\"{}\"", key);
  let mut search_start = 0usize;
  while let Some(rel) = input[search_start..].find(&pattern) {
    let key_start = search_start + rel;
    let mut idx = key_start + pattern.len();
    idx = skip_json_ws(input, idx);
    if !input[idx..].starts_with(':') {
      search_start = idx;
      continue;
    }
    idx += 1;
    idx = skip_json_ws(input, idx);
    let end = json_value_end(input, idx)?;
    return Some(&input[idx..end]);
  }
  None
}

fn skip_json_ws(input: &str, mut idx: usize) -> usize {
  while idx < input.len() {
    let Some(ch) = input[idx..].chars().next() else {
      break;
    };
    if !ch.is_whitespace() {
      break;
    }
    idx += ch.len_utf8();
  }
  idx
}

fn json_value_end(input: &str, start: usize) -> Option<usize> {
  let first = input[start..].chars().next()?;
  match first {
    '"' => json_string_end(input, start).map(|end| end + 1),
    '{' | '[' => json_group_end(input, start).map(|end| end + 1),
    _ => Some(
      input[start..]
        .find([',', '}'])
        .map(|rel| start + rel)
        .unwrap_or(input.len()),
    ),
  }
}

fn json_string_end(input: &str, start: usize) -> Option<usize> {
  let mut escaped = false;
  for (rel, ch) in input[start + 1..].char_indices() {
    if escaped {
      escaped = false;
    } else if ch == '\\' {
      escaped = true;
    } else if ch == '"' {
      return Some(start + 1 + rel);
    }
  }
  None
}

fn json_group_end(input: &str, start: usize) -> Option<usize> {
  let open = input[start..].chars().next()?;
  let close = if open == '{' { '}' } else { ']' };
  let mut depth = 0usize;
  let mut in_string = false;
  let mut escaped = false;
  for (rel, ch) in input[start..].char_indices() {
    if in_string {
      if escaped {
        escaped = false;
      } else if ch == '\\' {
        escaped = true;
      } else if ch == '"' {
        in_string = false;
      }
      continue;
    }
    if ch == '"' {
      in_string = true;
    } else if ch == open {
      depth += 1;
    } else if ch == close {
      depth = depth.saturating_sub(1);
      if depth == 0 {
        return Some(start + rel);
      }
    }
  }
  None
}

fn json_value_to_message(value: &str) -> String {
  let trimmed = value.trim();
  if let Some(stripped) =
    trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"'))
  {
    unescape_json_string(stripped)
  } else {
    trimmed.to_string()
  }
}

fn unescape_json_string(input: &str) -> String {
  let mut out = String::with_capacity(input.len());
  let mut chars = input.chars();
  while let Some(ch) = chars.next() {
    if ch != '\\' {
      out.push(ch);
      continue;
    }
    match chars.next() {
      Some('"') => out.push('"'),
      Some('\\') => out.push('\\'),
      Some('/') => out.push('/'),
      Some('n') => out.push('\n'),
      Some('r') => out.push('\r'),
      Some('t') => out.push('\t'),
      Some('b') => out.push('\u{0008}'),
      Some('f') => out.push('\u{000c}'),
      Some('u') => {
        let hex: String = chars.by_ref().take(4).collect();
        if let Ok(code) = u32::from_str_radix(&hex, 16) {
          if let Some(decoded) = char::from_u32(code) {
            out.push(decoded);
          }
        }
      }
      Some(other) => out.push(other),
      None => break,
    }
  }
  out
}

fn remove_json_object_key(input: &str, key: &str) -> Option<String> {
  let inner = input.trim().strip_prefix('{')?.strip_suffix('}')?;
  let mut pieces = Vec::new();
  let mut start = 0usize;
  let mut depth = 0usize;
  let mut in_string = false;
  let mut escaped = false;

  for (rel, ch) in inner.char_indices() {
    if in_string {
      if escaped {
        escaped = false;
      } else if ch == '\\' {
        escaped = true;
      } else if ch == '"' {
        in_string = false;
      }
      continue;
    }
    match ch {
      '"' => in_string = true,
      '{' | '[' => depth += 1,
      '}' | ']' => depth = depth.saturating_sub(1),
      ',' if depth == 0 => {
        push_json_piece_without_key(&inner[start..rel], key, &mut pieces);
        start = rel + 1;
      }
      _ => {}
    }
  }
  push_json_piece_without_key(&inner[start..], key, &mut pieces);
  Some(format!("{{{}}}", pieces.join(",")))
}

fn push_json_piece_without_key(
  piece: &str,
  key: &str,
  pieces: &mut Vec<String>,
) {
  let trimmed = piece.trim();
  if trimmed.is_empty() || trimmed.starts_with(&format!("\"{key}\"")) {
    return;
  }
  pieces.push(trimmed.to_string());
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parses_tracing_json_log_metadata_and_fields() {
    let parsed = parse_prefix(
      r#"{"timestamp":"2026-05-30T13:33:23.601120Z","level":"INFO","fields":{"message":"resolved PoolKey","pool_id":"0xe500","chain":"ethereum"},"target":"eth_mint_server::eth::pool_manager","filename":"state.rs","line_number":570,"threadId":"ThreadId(14)"}"#,
    );

    assert_eq!(parsed.time.as_deref(), Some("2026-05-30T13:33:23.601120Z"));
    assert_eq!(parsed.level_text.as_deref(), Some("INFO"));
    assert_eq!(
      parsed.target.as_deref(),
      Some("eth_mint_server::eth::pool_manager")
    );
    assert_eq!(parsed.file.as_deref(), Some("state.rs"));
    assert_eq!(parsed.file_line, Some(570));
    assert_eq!(parsed.thread_id.as_deref(), Some("ThreadId(14)"));
    assert!(parsed.message.starts_with("resolved PoolKey "));
    assert!(parsed.message.contains(r#""pool_id":"0xe500""#));
  }

  #[test]
  fn formats_call_site_labels_and_histogram_clipboard_text() {
    let entry = build_entry(
      "2026-05-30T13:33:23.601120Z INFO crate_name::main: main.rs:55: hello",
      None,
    );
    assert_eq!(call_site_label(&entry).as_deref(), Some("main:55"));

    let rows = vec![HistogramRow { label: "main:55".into(), count: 123 }];
    assert_eq!(histogram_clipboard_text(&rows), "main:55 123");
  }
}
