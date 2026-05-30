use std::{
  collections::VecDeque,
  fs,
  io::{self, Read},
  path::{Path, PathBuf},
  process::Command,
  time::{Instant, SystemTime},
};

use anyhow::{bail, Context, Result};

use crate::{
  app::build_entry,
  filter,
  model::{LogEntry, LogTab, PagingState, TabSource},
};

const MAX_LINES_PER_FILE: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Compression {
  Plain,
  Lz4,
  Gzip,
  Zstd,
  Xz,
  Bzip2,
}

pub fn has_file_changed(tab: &LogTab) -> Result<bool> {
  match &tab.source {
    TabSource::File(path) => changed_recently(path),
    TabSource::Folder(path) => folder_changed_recently(path),
    TabSource::Command(_) => Ok(false),
  }
}

pub fn reload_tab(tab: &mut LogTab) -> Result<()> {
  let (entries, paging) = match &tab.source {
    TabSource::File(path) => load_file_entries(path)?,
    TabSource::Folder(path) => load_folder_entries(path)?,
    TabSource::Command(_) => return Ok(()),
  };

  tab.entries = entries;
  tab.paging = Some(paging);
  tab.last_update = Instant::now();
  filter::recompute_tab(tab);
  tab.rendered_lines.clear();
  tab.last_render_width = 0;

  if tab.scroll.follow_bottom {
    tab.scroll.offset = tab.filtered_indices.len().saturating_sub(1);
  }

  Ok(())
}

pub fn load_file_entries(path: &Path) -> Result<(Vec<LogEntry>, PagingState)> {
  let (entries, truncated) = read_log_file(path, None)?;
  Ok((
    entries,
    PagingState {
      loaded_files: 1,
      total_files: 1,
      truncated_files: usize::from(truncated),
      max_lines_per_file: MAX_LINES_PER_FILE,
    },
  ))
}

pub fn load_folder_entries(
  path: &Path,
) -> Result<(Vec<LogEntry>, PagingState)> {
  let files = sorted_files_newest_first(path)?;
  let total_files = files.len();
  let mut entries = Vec::new();
  let mut truncated_files = 0usize;

  for file in &files {
    let file_label = file
      .file_name()
      .map(|s| s.to_string_lossy().to_string())
      .unwrap_or_else(|| file.display().to_string());
    let (file_entries, truncated) = read_log_file(file, Some(&file_label))?;
    truncated_files += usize::from(truncated);
    entries.extend(file_entries);
  }

  Ok((
    entries,
    PagingState {
      loaded_files: total_files,
      total_files,
      truncated_files,
      max_lines_per_file: MAX_LINES_PER_FILE,
    },
  ))
}

pub fn delete_matching_lines(tab: &mut LogTab) -> Result<usize> {
  let Some(re) = &tab.filters.delete_regex else {
    return Ok(0);
  };

  let TabSource::File(path) = &tab.source else {
    bail!("delete is only supported for single file tabs");
  };

  let content = read_text_with_detection(path)?;
  let lines: Vec<&str> = content.lines().collect();

  let deleted = lines.iter().filter(|line| re.is_match(line)).count();
  let kept = lines
    .iter()
    .filter(|line| !re.is_match(line))
    .copied()
    .collect::<Vec<_>>()
    .join("\n");

  let backup = path.with_extension("bak");
  fs::write(&backup, content)?;
  fs::write(path, kept)?;
  reload_tab(tab)?;
  Ok(deleted)
}

fn changed_recently(path: &Path) -> Result<bool> {
  let meta = fs::metadata(path)?;
  let modified = meta.modified()?;
  let elapsed = modified.elapsed().unwrap_or_default();
  Ok(elapsed.as_millis() < 500)
}

fn folder_changed_recently(path: &Path) -> Result<bool> {
  for file in sorted_files_newest_first(path)? {
    if changed_recently(&file)? {
      return Ok(true);
    }
  }
  Ok(false)
}

fn sorted_files_newest_first(path: &Path) -> Result<Vec<PathBuf>> {
  let mut files = Vec::new();
  for entry in
    fs::read_dir(path).with_context(|| format!("reading {}", path.display()))?
  {
    let entry = entry?;
    let file_type = entry.file_type()?;
    if !file_type.is_file() {
      continue;
    }
    let modified = entry
      .metadata()
      .and_then(|m| m.modified())
      .unwrap_or(SystemTime::UNIX_EPOCH);
    files.push((modified, entry.path()));
  }

  files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
  Ok(files.into_iter().map(|(_, path)| path).collect())
}

fn read_log_file(
  path: &Path,
  prefix: Option<&str>,
) -> Result<(Vec<LogEntry>, bool)> {
  let text = read_text_with_detection(path)?;
  let mut lines = VecDeque::new();
  let mut truncated = false;

  for line in text.lines() {
    if lines.len() == MAX_LINES_PER_FILE {
      lines.pop_front();
      truncated = true;
    }
    lines.push_back(line.to_string());
  }

  let entries = lines
    .into_iter()
    .map(|line| {
      let raw = match prefix {
        Some(prefix) => format!("[{prefix}] {line}"),
        None => line,
      };
      build_entry(&raw)
    })
    .collect();

  Ok((entries, truncated))
}

fn read_text_with_detection(path: &Path) -> Result<String> {
  let mut file = match fs::File::open(path) {
    Ok(file) => file,
    Err(err) if err.kind() == io::ErrorKind::NotFound => {
      return Ok(String::new())
    }
    Err(err) => return Err(err.into()),
  };
  let mut header = [0u8; 8];
  let read = file.read(&mut header)?;
  drop(file);

  let compression = detect_compression(path, &header[..read]);
  let bytes = match compression {
    Compression::Plain => fs::read(path)?,
    other => match decompress_with_command(path, other) {
      Ok(bytes) => bytes,
      Err(_) if other != Compression::Lz4 => {
        decompress_with_command(path, Compression::Lz4)?
      }
      Err(err) => return Err(err),
    },
  };

  Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn detect_compression(path: &Path, header: &[u8]) -> Compression {
  if header.starts_with(&[0x04, 0x22, 0x4d, 0x18]) || has_ext(path, "lz4") {
    Compression::Lz4
  } else if header.starts_with(&[0x1f, 0x8b]) || has_ext(path, "gz") {
    Compression::Gzip
  } else if header.starts_with(&[0x28, 0xb5, 0x2f, 0xfd])
    || has_ext(path, "zst")
  {
    Compression::Zstd
  } else if header.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00])
    || has_ext(path, "xz")
  {
    Compression::Xz
  } else if header.starts_with(b"BZh") || has_ext(path, "bz2") {
    Compression::Bzip2
  } else if looks_like_plain_text(header) {
    Compression::Plain
  } else {
    Compression::Lz4
  }
}

fn looks_like_plain_text(header: &[u8]) -> bool {
  header.iter().all(|b| b.is_ascii_graphic() || b.is_ascii_whitespace())
}

fn has_ext(path: &Path, ext: &str) -> bool {
  path
    .extension()
    .is_some_and(|value| value.to_string_lossy().eq_ignore_ascii_case(ext))
}

fn decompress_with_command(
  path: &Path,
  compression: Compression,
) -> Result<Vec<u8>> {
  let (program, args): (&str, &[&str]) = match compression {
    Compression::Lz4 => ("lz4", &["-dc"]),
    Compression::Gzip => ("gzip", &["-dc"]),
    Compression::Zstd => ("zstd", &["-dc"]),
    Compression::Xz => ("xz", &["-dc"]),
    Compression::Bzip2 => ("bzip2", &["-dc"]),
    Compression::Plain => return Ok(fs::read(path)?),
  };

  let output = Command::new(program).args(args).arg(path).output()?;
  if !output.status.success() {
    bail!("{program} failed for {}", path.display());
  }
  Ok(output.stdout)
}
