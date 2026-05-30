use std::{fs, time::Instant};

use anyhow::{bail, Result};

use crate::{
  app::build_entry,
  filter,
  model::{LogTab, TabSource},
};

pub fn has_file_changed(tab: &LogTab) -> Result<bool> {
  let TabSource::File(path) = &tab.source else {
    return Ok(false);
  };

  let meta = fs::metadata(path)?;
  let modified = meta.modified()?;
  let elapsed = modified.elapsed().unwrap_or_default();
  Ok(elapsed.as_millis() < 500)
}

pub fn reload_tab(tab: &mut LogTab) -> Result<()> {
  let TabSource::File(path) = &tab.source else {
    return Ok(());
  };

  let content = fs::read_to_string(path)?;
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

  let TabSource::File(path) = &tab.source else {
    bail!("delete is only supported for file tabs");
  };

  let content = fs::read_to_string(path)?;
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
