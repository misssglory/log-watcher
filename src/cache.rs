use std::{
  fs,
  io::{self, Write},
  path::PathBuf,
};

use crate::model::RecentItem;

const MAX_RECENTS: usize = 50;

fn cache_dir() -> PathBuf {
  if let Ok(cache_home) = std::env::var("XDG_CACHE_HOME") {
    return PathBuf::from(cache_home).join("log-watcher");
  }

  if let Ok(home) = std::env::var("HOME") {
    return PathBuf::from(home).join(".cache").join("log-watcher");
  }

  PathBuf::from(".log-watcher-cache")
}

fn recents_path() -> PathBuf {
  cache_dir().join("recents.tsv")
}

fn encode(value: &str) -> String {
  value.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn decode(value: &str) -> String {
  let mut out = String::new();
  let mut escaped = false;

  for c in value.chars() {
    if escaped {
      match c {
        't' => out.push('\t'),
        'n' => out.push('\n'),
        '\\' => out.push('\\'),
        _ => {
          out.push('\\');
          out.push(c);
        }
      }
      escaped = false;
    } else if c == '\\' {
      escaped = true;
    } else {
      out.push(c);
    }
  }

  if escaped {
    out.push('\\');
  }

  out
}

pub fn load_recents() -> Vec<RecentItem> {
  let Ok(content) = fs::read_to_string(recents_path()) else {
    return Vec::new();
  };

  content
    .lines()
    .filter_map(|line| {
      let (kind, value) = line.split_once('\t')?;
      match kind {
        "file" => Some(RecentItem::File(PathBuf::from(decode(value)))),
        "folder" => Some(RecentItem::Folder(PathBuf::from(decode(value)))),
        "command" => Some(RecentItem::Command(decode(value))),
        _ => None,
      }
    })
    .collect()
}

pub fn save_recents(recents: &[RecentItem]) -> io::Result<()> {
  fs::create_dir_all(cache_dir())?;
  let mut file = fs::File::create(recents_path())?;

  for item in recents.iter().take(MAX_RECENTS) {
    match item {
      RecentItem::File(path) => {
        writeln!(file, "file\t{}", encode(&path.to_string_lossy()))?;
      }
      RecentItem::Folder(path) => {
        writeln!(file, "folder\t{}", encode(&path.to_string_lossy()))?;
      }
      RecentItem::Command(command) => {
        writeln!(file, "command\t{}", encode(command))?;
      }
    }
  }

  Ok(())
}

pub fn remember_recent(recents: &mut Vec<RecentItem>, item: RecentItem) {
  recents.retain(|existing| existing != &item);
  recents.insert(0, item);
  recents.truncate(MAX_RECENTS);
}
