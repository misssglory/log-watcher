use std::{sync::mpsc, thread};

use crate::model::{
  FilterJob, FilterProgress, FilterUpdate, Filters, LogEntry, LogTab,
};

const ASYNC_FILTER_THRESHOLD: usize = 50_000;
const PROGRESS_CHUNK: usize = 5_000;

pub fn recompute_tab(tab: &mut LogTab) {
  tab.filter_job = None;

  if tab.entries.len() >= ASYNC_FILTER_THRESHOLD {
    start_recompute_tab(tab);
    return;
  }

  let (filtered_indices, delete_matches) =
    compute(&tab.entries, &tab.filters, None);
  apply_filter_result(tab, filtered_indices, delete_matches);
}

pub fn start_recompute_tab(tab: &mut LogTab) {
  let entries = tab.entries.clone();
  let filters = tab.filters.clone();
  let total = entries.len();
  let (tx, rx) = mpsc::channel();

  thread::spawn(move || {
    let tx_progress = tx.clone();
    let (filtered_indices, delete_matches) = compute(
      &entries,
      &filters,
      Some(Box::new(move |done| {
        let _ = tx_progress
          .send(FilterUpdate::Progress(FilterProgress { done, total }));
      })),
    );
    let _ =
      tx.send(FilterUpdate::Complete { filtered_indices, delete_matches });
  });

  tab.filter_job =
    Some(FilterJob { rx, progress: FilterProgress { done: 0, total } });
  tab.rendered_lines.clear();
  tab.last_render_width = 0;
}

pub fn poll_filter_job(tab: &mut LogTab) -> bool {
  let Some(job) = &mut tab.filter_job else {
    return false;
  };

  let Some((filtered_indices, delete_matches)) = job.drain() else {
    return false;
  };

  tab.filter_job = None;
  apply_filter_result(tab, filtered_indices, delete_matches);
  true
}

fn compute(
  entries: &[LogEntry],
  filters: &Filters,
  mut progress: Option<Box<dyn FnMut(usize) + Send>>,
) -> (Vec<usize>, usize) {
  let mut filtered_indices = Vec::new();
  let mut delete_matches = 0usize;

  for (idx, entry) in entries.iter().enumerate() {
    let level_ok = match filters.min_level {
      Some(min) => entry.level >= min,
      None => true,
    };

    let regex_ok = match &filters.include_regex {
      Some(re) => re.is_match(&entry.raw),
      None => true,
    };

    if level_ok && regex_ok {
      filtered_indices.push(idx);
    }

    if filters.delete_regex.as_ref().is_some_and(|re| re.is_match(&entry.raw)) {
      delete_matches += 1;
    }

    if (idx + 1) % PROGRESS_CHUNK == 0 {
      if let Some(progress) = progress.as_mut() {
        progress(idx + 1);
      }
    }
  }

  if let Some(progress) = progress.as_mut() {
    progress(entries.len());
  }

  (filtered_indices, delete_matches)
}

fn apply_filter_result(
  tab: &mut LogTab,
  filtered_indices: Vec<usize>,
  delete_matches: usize,
) {
  tab.filtered_indices = filtered_indices;
  tab.delete_preview.matches = delete_matches;
  tab.rendered_lines.clear();
  tab.last_render_width = 0;

  if tab.scroll.offset >= tab.filtered_indices.len() {
    tab.scroll.offset = tab.filtered_indices.len().saturating_sub(1);
  }
}
