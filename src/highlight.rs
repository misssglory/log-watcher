use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::model::LogLevel;

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

pub fn render_log_line(line_no: usize, text: &str, level: LogLevel) -> Line<'static> {
    let mut spans = Vec::new();

    spans.push(Span::styled(
        format!("{:>6} ", line_no),
        Style::default().fg(Color::DarkGray),
    ));

    let accent = level_color(level);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch == '"' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            if i < chars.len() { i += 1; }
            let s: String = chars[start..i].iter().collect();

            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() { j += 1; }
            if j < chars.len() && chars[j] == ':' {
                spans.push(Span::styled(s, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
            } else {
                spans.push(Span::styled(s, Style::default().fg(Color::LightGreen)));
            }
            continue;
        }

        if ch.is_alphanumeric() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();

            let mut j = i;
            while j < chars.len() && chars[j].is_whitespace() { j += 1; }
            if j < chars.len() && chars[j] == ':' {
                spans.push(Span::styled(token, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));
            } else if token.chars().all(|c| c.is_ascii_digit()) {
                spans.push(Span::styled(token, Style::default().fg(Color::Yellow)));
            } else {
                spans.push(Span::styled(token, Style::default().fg(Color::White)));
            }
            continue;
        }

        if matches!(ch, ',' | '{' | '}' | '(' | ')' | ':') {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(accent),
            ));
            i += 1;
            continue;
        }

        spans.push(Span::raw(ch.to_string()));
        i += 1;
    }

    Line::from(spans)
}
