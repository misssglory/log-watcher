use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::model::{LogEntry, LogLevel};

pub const PREFIX_WIDTH: usize = 70;
const LINE_NUMBER_WIDTH: usize = 7;

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
        LogLevel::Unknown => Color::White,
    }
}

fn level_style(level: LogLevel) -> Style {
    Style::default()
        .fg(level_color(level))
        .add_modifier(Modifier::BOLD)
}

fn file_style(level: LogLevel) -> Style {
    match level {
        LogLevel::Error => Style::default().fg(Color::LightRed),
        LogLevel::Warn => Style::default().fg(Color::LightYellow),
        LogLevel::Info => Style::default().fg(Color::LightBlue),
        LogLevel::Debug => Style::default().fg(Color::LightCyan),
        LogLevel::Trace => Style::default().fg(Color::Gray),
        LogLevel::Unknown => Style::default().fg(Color::White),
    }
}

fn special_word_style(token: &str) -> Option<Style> {
    match token {
        "ERROR" => Some(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        "WARN" => Some(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        "INFO" => Some(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        "DEBUG" => Some(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        "Buy" | "buy" => Some(Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)),
        "Sell" | "sell" => Some(Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD)),
        _ => None,
    }
}

fn push_token(spans: &mut Vec<Span<'static>>, token: String, level: LogLevel, key_candidate: bool) {
    if let Some(style) = special_word_style(&token) {
        spans.push(Span::styled(token, style));
    } else if key_candidate {
        spans.push(Span::styled(
            token,
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    } else if token.chars().all(|c| c.is_ascii_digit()) {
        spans.push(Span::styled(token, Style::default().fg(Color::Yellow)));
    } else {
        spans.push(Span::styled(token, Style::default().fg(Color::White)));
    }
}

fn colorize_message(message: &str, level: LogLevel) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = message.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '"' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '"' && chars[i - 1] != '\\' {
                    i += 1;
                    break;
                }
                i += 1;
            }

            let s: String = chars[start..i].iter().collect();
            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }

            if j < chars.len() && chars[j] == ':' {
                spans.push(Span::styled(
                    s,
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(s, Style::default().fg(Color::LightGreen)));
            }
            continue;
        }

        if ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.' || ch == '/' {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_alphanumeric() || matches!(chars[i], '_' | '-' | '.' | '/'))
            {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();

            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let key_candidate = j < chars.len() && chars[j] == ':';
            push_token(&mut spans, token, level, key_candidate);
            continue;
        }

        if matches!(ch, ',' | '{' | '}' | '(' | ')' | ':' | '[' | ']') {
            spans.push(Span::styled(ch.to_string(), Style::default().fg(level_color(level))));
            i += 1;
            continue;
        }

        spans.push(Span::raw(ch.to_string()));
        i += 1;
    }

    spans
}

fn build_prefix_spans(line_no: usize, entry: &LogEntry) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut width = 0usize;

    let line_no_part = format!("{:>6} ", line_no);
    width += text_width(&line_no_part);
    spans.push(Span::styled(
        line_no_part,
        Style::default().fg(Color::DarkGray),
    ));

    if let Some(time) = &entry.parsed.time {
        let s = format!("{time}  ");
        width += text_width(&s);
        spans.push(Span::styled(s, Style::default().fg(Color::DarkGray)));
    }

    let level_text = entry
        .parsed
        .level_text
        .clone()
        .unwrap_or_else(|| entry.level.as_str().to_string());
    let level_part = format!("{:<5} ", level_text);
    width += text_width(&level_part);
    spans.push(Span::styled(level_part, level_style(entry.level)));

    if let (Some(file), Some(file_line)) = (&entry.parsed.file, entry.parsed.file_line) {
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

fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
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

            lines.last_mut().unwrap().push(Span::styled(s, style));
            current_width += w;
        }
    }

    lines
}

pub fn render_entry_lines(
    line_no: usize,
    entry: &LogEntry,
    total_width: usize,
) -> Vec<Line<'static>> {
    let prefix = build_prefix_spans(line_no, entry);
    let body_width = total_width.saturating_sub(LINE_NUMBER_WIDTH + PREFIX_WIDTH).max(1);
    let message = &entry.parsed.message;
    let wrapped_body = wrap_spans(colorize_message(message, entry.level), body_width);

    let continuation_indent = Span::raw(" ".repeat(LINE_NUMBER_WIDTH + PREFIX_WIDTH));
    let mut out = Vec::new();

    for (idx, body) in wrapped_body.into_iter().enumerate() {
        let mut spans = if idx == 0 {
            prefix.clone()
        } else {
            vec![continuation_indent.clone()]
        };
        spans.extend(body);
        out.push(Line::from(spans));
    }

    if out.is_empty() {
        out.push(Line::from(prefix));
    }

    out
}
