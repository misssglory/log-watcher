use crate::model::LogTab;

pub fn recompute_tab(tab: &mut LogTab) {
  tab.filtered_indices.clear();

  for (idx, entry) in tab.entries.iter().enumerate() {
    let level_ok = match tab.filters.min_level {
      Some(min) => entry.level >= min,
      None => true,
    };

    let regex_ok = match &tab.filters.include_regex {
      Some(re) => re.is_match(&entry.raw),
      None => true,
    };

    if level_ok && regex_ok {
      tab.filtered_indices.push(idx);
    }
  }

  recompute_delete_preview(tab);
  tab.rendered_lines.clear();
  tab.last_render_width = 0;

  if tab.scroll.offset >= tab.filtered_indices.len() {
    tab.scroll.offset = tab.filtered_indices.len().saturating_sub(1);
  }
}

pub fn recompute_delete_preview(tab: &mut LogTab) {
  tab.delete_preview.matches = match &tab.filters.delete_regex {
    Some(re) => tab.entries.iter().filter(|e| re.is_match(&e.raw)).count(),
    None => 0,
  };
}
