use ratatui::{
  style::{Color, Modifier, Style},
  text::{Line, Span},
};
use regex::Regex;
use unicode_width::UnicodeWidthStr;

use crate::model::{LogEntry, LogLevel};

pub const PREFIX_WIDTH: usize = 70;
const LINE_NUMBER_WIDTH: usize = 7;
const PRETTY_INDENT_STEP: usize = 2;
const INLINE_GROUP_THRESHOLD: usize = 50;

fn text_width(s: &str) -> usize {
  UnicodeWidthStr::width(s)
}

fn level_color(level: LogLevel) -> Color {
  match level {
    LogLevel::Trace => Color::DarkGray,
    LogLevel::Debug => Color::Cyan,
    LogLevel::Info => Color::Green,
    LogLevel::Warn => Color::Yellow,
    LogLevel::Error => Color::Red,
  }
}

fn level_style(level: LogLevel) -> Style {
  Style::default().fg(level_color(level)).add_modifier(Modifier::BOLD)
}

fn file_style(level: LogLevel) -> Style {
  match level {
    LogLevel::Error => Style::default().fg(Color::LightRed),
    LogLevel::Warn => Style::default().fg(Color::LightYellow),
    LogLevel::Info => Style::default().fg(Color::LightBlue),
    LogLevel::Debug => Style::default().fg(Color::LightCyan),
    LogLevel::Trace => Style::default().fg(Color::Gray),
  }
}

fn line_number_style() -> Style {
  Style::default().fg(Color::DarkGray)
}

fn time_style() -> Style {
  Style::default().fg(Color::DarkGray)
}

fn key_style() -> Style {
  Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
}

fn number_style() -> Style {
  Style::default().fg(Color::Yellow)
}

fn string_style() -> Style {
  Style::default().fg(Color::LightGreen)
}

fn plain_style() -> Style {
  Style::default().fg(Color::White)
}

fn search_style(base: Style, active: bool) -> Style {
  if active {
    base.bg(Color::Blue).fg(Color::Black).add_modifier(Modifier::BOLD)
  } else {
    base.bg(Color::DarkGray)
  }
}

fn special_word_style(token: &str) -> Option<Style> {
  match token {
    "ERROR" => {
      Some(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    }
    "WARN" => {
      Some(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    }
    "INFO" => {
      Some(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    }
    "DEBUG" => {
      Some(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    }
    "TRACE" => {
      Some(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))
    }
    "Buy" | "buy" => {
      Some(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD))
    }
    "Sell" | "sell" => {
      Some(Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD))
    }
    "Some" => {
      Some(Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD))
    }
    "None" => {
      Some(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))
    }
    "Ok" => {
      Some(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    }
    "Err" => Some(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    _ => None,
  }
}

fn matching_close(ch: char) -> Option<char> {
  match ch {
    '{' => Some('}'),
    '(' => Some(')'),
    '[' => Some(']'),
    _ => None,
  }
}

fn find_matching_bracket(chars: &[char], start: usize) -> Option<usize> {
  let open = *chars.get(start)?;
  let close = matching_close(open)?;
  let mut depth = 0usize;
  let mut i = start;
  let mut in_string = false;
  let mut escape = false;

  while i < chars.len() {
    let ch = chars[i];

    if in_string {
      if escape {
        escape = false;
      } else if ch == '\\' {
        escape = true;
      } else if ch == '"' {
        in_string = false;
      }
      i += 1;
      continue;
    }

    if ch == '"' {
      in_string = true;
      i += 1;
      continue;
    }

    if ch == open {
      depth += 1;
    } else if ch == close {
      depth = depth.saturating_sub(1);
      if depth == 0 {
        return Some(i);
      }
    }

    i += 1;
  }

  None
}

fn should_inline_group(chars: &[char], start: usize, threshold: usize) -> bool {
  let Some(end) = find_matching_bracket(chars, start) else {
    return false;
  };

  if end <= start + 1 {
    return true;
  }

  let inner: String = chars[start + 1..end].iter().collect();
  let trimmed = inner.trim();

  if trimmed.is_empty() {
    return true;
  }

  trimmed.len() < threshold && !trimmed.contains('\n')
}

fn pretty_format_range(
  chars: &[char],
  start: usize,
  end: usize,
  indent: usize,
  indent_step: usize,
  out: &mut String,
) {
  let mut i = start;
  let mut line_start = out.is_empty() || out.ends_with('\n');

  while i < end {
    let ch = chars[i];

    if ch == '"' {
      if line_start {
        out.push_str(&" ".repeat(indent));
        line_start = false;
      }

      out.push(ch);
      i += 1;
      let mut escape = false;
      while i < end {
        let c = chars[i];
        out.push(c);
        if escape {
          escape = false;
        } else if c == '\\' {
          escape = true;
        } else if c == '"' {
          i += 1;
          break;
        }
        i += 1;
      }
      continue;
    }

    if let Some(close) = matching_close(ch) {
      if let Some(group_end) = find_matching_bracket(chars, i) {
        if should_inline_group(chars, i, INLINE_GROUP_THRESHOLD) {
          if line_start {
            out.push_str(&" ".repeat(indent));
            line_start = false;
          }
          let group: String = chars[i..=group_end].iter().collect();
          out.push_str(&group);
          i = group_end + 1;
          continue;
        }

        if line_start {
          out.push_str(&" ".repeat(indent));
        }

        out.push(ch);
        out.push('\n');
        pretty_format_range(
          chars,
          i + 1,
          group_end,
          indent + indent_step,
          indent_step,
          out,
        );

        while out.ends_with(' ') || out.ends_with('\t') {
          out.pop();
        }
        if !out.ends_with('\n') {
          out.push('\n');
        }

        out.push_str(&" ".repeat(indent));
        out.push(close);
        line_start = false;
        i = group_end + 1;
        continue;
      }
    }

    match ch {
      ',' => {
        out.push(',');
        out.push('\n');
        line_start = true;
      }
      ' ' | '\t' if line_start => {}
      '\n' => {
        out.push('\n');
        line_start = true;
      }
      _ => {
        if line_start {
          out.push_str(&" ".repeat(indent));
          line_start = false;
        }
        out.push(ch);
      }
    }

    i += 1;
  }
}

fn pretty_format_message(input: &str, indent_step: usize) -> String {
  let chars: Vec<char> = input.chars().collect();
  let mut out = String::with_capacity(input.len() + input.len() / 3);
  pretty_format_range(&chars, 0, chars.len(), 0, indent_step, &mut out);
  out
}

fn is_ident_char(ch: char) -> bool {
  ch.is_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/')
}

fn search_ranges(text: &str, re: Option<&Regex>) -> Vec<(usize, usize)> {
  match re {
    Some(re) => re.find_iter(text).map(|m| (m.start(), m.end())).collect(),
    None => Vec::new(),
  }
}

fn byte_in_ranges(byte_idx: usize, ranges: &[(usize, usize)]) -> bool {
  ranges.iter().any(|(s, e)| byte_idx >= *s && byte_idx < *e)
}

fn styled_char(
  spans: &mut Vec<Span<'static>>,
  ch: char,
  mut style: Style,
  highlighted: bool,
  active_match_line: bool,
) {
  if highlighted {
    style = search_style(style, active_match_line);
  }
  spans.push(Span::styled(ch.to_string(), style));
}

fn colorize_message(
  message: &str,
  level: LogLevel,
  search_re: Option<&Regex>,
  active_match_line: bool,
) -> Vec<Span<'static>> {
  let mut spans = Vec::new();
  let chars: Vec<(usize, char)> = message.char_indices().collect();
  let ranges = search_ranges(message, search_re);
  let mut i = 0usize;

  while i < chars.len() {
    let (byte_idx, ch) = chars[i];

    if ch == '"' {
      let start_i = i;
      i += 1;
      let mut escaped = false;
      while i < chars.len() {
        let (_, c) = chars[i];
        if escaped {
          escaped = false;
        } else if c == '\\' {
          escaped = true;
        } else if c == '"' {
          i += 1;
          break;
        }
        i += 1;
      }

      let start_byte = chars[start_i].0;
      let end_byte = if i < chars.len() { chars[i].0 } else { message.len() };
      let token = message[start_byte..end_byte].to_string();

      let mut j = i;
      while j < chars.len() && chars[j].1.is_whitespace() {
        j += 1;
      }

      let style = if j < chars.len() && chars[j].1 == ':' {
        key_style()
      } else {
        string_style()
      };

      for (rel, c) in token.char_indices() {
        let highlighted = byte_in_ranges(start_byte + rel, &ranges);
        styled_char(&mut spans, c, style, highlighted, active_match_line);
      }
      continue;
    }

    if is_ident_char(ch) {
      let start_i = i;
      i += 1;
      while i < chars.len() && is_ident_char(chars[i].1) {
        i += 1;
      }

      let start_byte = chars[start_i].0;
      let end_byte = if i < chars.len() { chars[i].0 } else { message.len() };
      let token = message[start_byte..end_byte].to_string();

      let mut j = i;
      while j < chars.len() && chars[j].1.is_whitespace() {
        j += 1;
      }
      let key_candidate = j < chars.len() && chars[j].1 == ':';

      let base_style = if let Some(style) = special_word_style(&token) {
        style
      } else if key_candidate {
        key_style()
      } else if token.chars().all(|c| c.is_ascii_digit())
        || token.parse::<f64>().is_ok()
      {
        number_style()
      } else {
        plain_style()
      };

      for (rel, c) in token.char_indices() {
        let highlighted = byte_in_ranges(start_byte + rel, &ranges);
        styled_char(&mut spans, c, base_style, highlighted, active_match_line);
      }
      continue;
    }

    let base_style =
      if matches!(ch, ',' | '{' | '}' | '(' | ')' | '[' | ']' | ':') {
        Style::default().fg(level_color(level))
      } else if ch == '\n' {
        spans.push(Span::raw("\n"));
        i += 1;
        continue;
      } else {
        plain_style()
      };

    let highlighted = byte_in_ranges(byte_idx, &ranges);
    styled_char(&mut spans, ch, base_style, highlighted, active_match_line);
    i += 1;
  }

  spans
}

fn build_prefix_spans(line_no: usize, entry: &LogEntry) -> Vec<Span<'static>> {
  let mut spans = Vec::new();
  let mut width = 0usize;

  let line_no_part = format!("{:>6} ", line_no);
  width += text_width(&line_no_part);
  spans.push(Span::styled(line_no_part, line_number_style()));

  if let Some(time) = &entry.parsed.time {
    let s = format!("{time}  ");
    width += text_width(&s);
    spans.push(Span::styled(s, time_style()));
  }

  let level_text = entry
    .parsed
    .level_text
    .clone()
    .unwrap_or_else(|| entry.level.as_str().to_string());
  let level_part = format!("{:<5} ", level_text);
  width += text_width(&level_part);
  spans.push(Span::styled(level_part, level_style(entry.level)));

  if let (Some(file), Some(file_line)) =
    (&entry.parsed.file, entry.parsed.file_line)
  {
    let s = format!("{file}:{file_line}:");
    width += text_width(&s);
    spans.push(Span::styled(s, file_style(entry.level)));
  }

  let target_width = LINE_NUMBER_WIDTH + PREFIX_WIDTH;
  if width < target_width {
    spans.push(Span::raw(" ".repeat(target_width - width)));
  } else {
    spans.push(Span::raw(" "));
  }

  spans
}

fn wrap_spans(
  spans: Vec<Span<'static>>,
  width: usize,
) -> Vec<Vec<Span<'static>>> {
  if width == 0 {
    return vec![vec![]];
  }

  let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
  let mut current_width = 0usize;

  for span in spans {
    let content = span.content.to_string();
    let style = span.style;

    for ch in content.chars() {
      if ch == '\n' {
        lines.push(Vec::new());
        current_width = 0;
        continue;
      }

      let s = ch.to_string();
      let w = text_width(&s);

      if current_width + w > width && current_width > 0 {
        lines.push(Vec::new());
        current_width = 0;
      }

      lines
        .last_mut()
        .expect("at least one line exists")
        .push(Span::styled(s, style));
      current_width += w;
    }
  }

  if lines.is_empty() {
    vec![vec![]]
  } else {
    lines
  }
}

pub fn render_entry_lines(
  line_no: usize,
  entry: &LogEntry,
  total_width: usize,
  pretty_print: bool,
  search_re: Option<&Regex>,
  active_match_line: bool,
) -> Vec<Line<'static>> {
  let prefix = build_prefix_spans(line_no, entry);
  let body_width =
    total_width.saturating_sub(LINE_NUMBER_WIDTH + PREFIX_WIDTH).max(1);

  let source_message = if pretty_print {
    pretty_format_message(&entry.parsed.message, PRETTY_INDENT_STEP)
  } else {
    entry.parsed.message.clone()
  };

  let body_spans = colorize_message(
    &source_message,
    entry.level,
    search_re,
    active_match_line,
  );
  let wrapped_body = wrap_spans(body_spans, body_width);

  let continuation_indent =
    Span::raw(" ".repeat(LINE_NUMBER_WIDTH + PREFIX_WIDTH));
  let mut out = Vec::new();

  for (idx, body) in wrapped_body.into_iter().enumerate() {
    let mut spans =
      if idx == 0 { prefix.clone() } else { vec![continuation_indent.clone()] };
    spans.extend(body);
    out.push(Line::from(spans));
  }

  if out.is_empty() {
    out.push(Line::from(prefix));
  }

  out
}
