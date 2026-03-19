use std::{fs, time::Instant};

use anyhow::Result;

use crate::{app::build_entry, filter, model::LogTab};

pub fn has_file_changed(tab: &LogTab) -> Result<bool> {
  let meta = fs::metadata(&tab.path)?;
  let modified = meta.modified()?;
  let elapsed = modified.elapsed().unwrap_or_default();
  Ok(elapsed.as_millis() < 500)
}

pub fn reload_tab(tab: &mut LogTab) -> Result<()> {
  let content = fs::read_to_string(&tab.path)?;
  tab.entries = content.lines().map(build_entry).collect();
  tab.last_update = Instant::now();
  filter::recompute_tab(tab);
  tab.rendered_lines.clear();
  tab.last_render_width = 0;

  if tab.scroll.follow_bottom {
    tab.scroll.offset = tab.filtered_indices.len().saturating_sub(1);
  }

  Ok(())
}

pub fn delete_matching_lines(tab: &mut LogTab) -> Result<usize> {
  let Some(re) = &tab.filters.delete_regex else {
    return Ok(0);
  };

  let content = fs::read_to_string(&tab.path)?;
  let lines: Vec<&str> = content.lines().collect();

  let deleted = lines.iter().filter(|line| re.is_match(line)).count();
  let kept = lines
    .iter()
    .filter(|line| !re.is_match(line))
    .copied()
    .collect::<Vec<_>>()
    .join("\n");

  let backup = tab.path.with_extension("bak");
  fs::write(&backup, content)?;
  fs::write(&tab.path, kept)?;
  reload_tab(tab)?;
  Ok(deleted)
}
