//! Incognito layout: passes for a text editor above a log pane, in grays.

use std::time::Instant;

use idk_shared::protocol::{LogKind, LogLine};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::map::{self, MapStyle};
use super::theme::INCOGNITO;
use crate::app::{App, Mode, Status};
use crate::config::GlyphSet;
use crate::state::unix_now;

/// Log pane height including its header row.
const LOG_HEIGHT: u16 = 8;
/// Line-number columns, so the gutter does not jitter around 99 -> 100.
const MIN_GUTTER: usize = 3;
const OTHER_TABS: [&str; 2] = ["world.log", "Cargo.toml"];

pub fn draw(f: &mut Frame, app: &App) {
    let log_height = (f.area().height / 3).min(LOG_HEIGHT);
    let [tabs, editor, log, bar] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(log_height),
        Constraint::Length(1),
    ])
    .areas(f.area());
    draw_tabs(f, app, tabs);
    draw_editor(f, app, editor);
    draw_log(f, app, log);
    draw_bar(f, app, bar);
}

fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let t = &INCOGNITO;
    let paint = |c| app.config.colors.paint(c);
    let s = &app.state;
    let biome = s.my_pos().and_then(|p| s.biome(p)).map(|b| b.def().name.replace(' ', "_"));
    let file = biome.map_or_else(|| "main.rs".to_string(), |b| format!("{b}.rs"));
    // The open tab shares the editor's background (the terminal's), the others sit on the bar.
    let mut spans =
        vec![Span::styled(format!(" {file}  × "), Style::new().fg(paint(t.text)).bg(Color::Reset))];
    spans.extend(OTHER_TABS.map(|name| Span::styled(format!(" {name} "), Style::new().fg(paint(t.dim)))));
    f.render_widget(Paragraph::new(Line::from(spans)).style(Style::new().bg(paint(t.border))), area);
}

fn draw_editor(f: &mut Frame, app: &App, area: Rect) {
    let t = &INCOGNITO;
    let paint = |c| app.config.colors.paint(c);
    let Some(me) = app.state.my_pos() else { return };
    let first = me.y - area.height as i32 / 2;
    let width = gutter_width(first, first + area.height as i32 - 1);
    let [gutter, text] = Layout::horizontal([Constraint::Length(width), Constraint::Min(0)]).areas(area);
    let lines: Vec<Line> = (first..first + area.height as i32)
        .map(|y| {
            let color = if y == me.y { t.text } else { t.dim };
            Line::styled(gutter_line(y, width), Style::new().fg(paint(color)))
        })
        .collect();
    f.render_widget(Paragraph::new(lines), gutter);
    let style = MapStyle { glyphs: GlyphSet::Ascii, colors: app.config.colors, theme: t };
    map::render(f.buffer_mut(), text, &app.state, &style, Instant::now());
}

/// Total gutter width (digits and a trailing space) for lines `first..=last`.
fn gutter_width(first: i32, last: i32) -> u16 {
    let digits = first.to_string().len().max(last.to_string().len());
    digits.max(MIN_GUTTER) as u16 + 1
}

/// Line number right-aligned in a gutter of total width `width`.
fn gutter_line(y: i32, width: u16) -> String {
    format!("{y:>w$} ", w = width as usize - 1)
}

fn draw_log(f: &mut Frame, app: &App, area: Rect) {
    let t = &INCOGNITO;
    let paint = |c| app.config.colors.paint(c);
    let [header, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    f.render_widget(
        Paragraph::new(" OUTPUT").style(Style::new().fg(paint(t.dim)).bg(paint(t.border))),
        header,
    );
    let lines: Vec<Line> = app
        .state
        .log
        .iter()
        .rev()
        .take(body.height as usize)
        .rev()
        .map(|e| {
            let level = level(&e.line);
            let level_style = match level {
                Level::Debug => Style::new().fg(paint(t.dim)),
                Level::Info => Style::new().fg(paint(t.system)),
                Level::Warn => Style::new().fg(paint(t.text)).add_modifier(Modifier::BOLD),
            };
            Line::from(vec![
                Span::styled(format!("{} ", iso_time(e.time)), Style::new().fg(paint(t.dim))),
                Span::styled(format!("{} ", level.label()), level_style),
                Span::styled(message(&e.line), Style::new().fg(paint(t.text))),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(lines), body);
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Level {
    Debug,
    Info,
    Warn,
}

impl Level {
    /// Padded to the width of the longest label.
    fn label(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO ",
            Level::Warn => "WARN ",
        }
    }
}

/// Disconnections and departures are warnings. The texts come from `on_net` and the server's
/// `leave` notice; `LogKind` has no finer category and the wire protocol is not worth widening for this.
fn level(line: &LogLine) -> Level {
    match line.kind {
        LogKind::Animal => Level::Debug,
        LogKind::System
            if line.text.starts_with("Disconnected") || line.text.ends_with("left the world.") =>
        {
            Level::Warn
        }
        _ => Level::Info,
    }
}

fn message(line: &LogLine) -> String {
    match &line.kind {
        LogKind::Chat { from } => format!("[chat] <{from}> {}", line.text),
        _ => line.text.clone(),
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` from seconds since the Unix epoch.
fn iso_time(secs: u64) -> String {
    let (y, m, d) = civil_from_days((secs / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}T{}Z", super::clock(secs % 86_400))
}

/// Gregorian date of a day count since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (month <= 2) as i64, month, day)
}

fn draw_bar(f: &mut Frame, app: &App, area: Rect) {
    let t = &INCOGNITO;
    let paint = |c| app.config.colors.paint(c);
    let bar = Style::new().fg(paint(t.text)).bg(paint(t.border));
    let typing = matches!(app.mode, Mode::Chat(_));
    let mode = if typing { " INSERT " } else { " NORMAL " };
    let right = format!(" {} · {} ", users(app.state.online.len()), &super::clock(unix_now() % 86_400)[..5]);
    let [mode_area, mid, right_area] = Layout::horizontal([
        Constraint::Length(mode.len() as u16),
        Constraint::Min(0),
        Constraint::Length(right.chars().count() as u16),
    ])
    .areas(area);
    let mode_style = Style::new().fg(paint(t.border)).bg(paint(t.text)).add_modifier(Modifier::BOLD);
    f.render_widget(Paragraph::new(mode).style(mode_style), mode_area);
    f.render_widget(Paragraph::new(right).style(bar), right_area);

    let middle = match &app.mode {
        Mode::Chat(text) => command_line(text, mid.width as usize),
        Mode::Play => match notice(app) {
            Some(n) => format!(" {n}"),
            None => app.state.my_pos().map(|p| format!(" Ln {}, Col {}", p.y, p.x)).unwrap_or_default(),
        },
    };
    if typing && mid.width > 0 {
        f.set_cursor_position(Position::new(mid.x + Line::from(middle.as_str()).width() as u16, mid.y));
    }
    f.render_widget(Paragraph::new(middle).style(bar), mid);
}

fn users(n: usize) -> String {
    format!("{n} user{}", if n == 1 { "" } else { "s" })
}

/// Vim-style command line showing the end of `text`, leaving one cell for the cursor.
fn command_line(text: &str, width: usize) -> String {
    let budget = width.saturating_sub(2);
    let mut used = 0;
    let mut tail: Vec<char> = Vec::new();
    for c in text.chars().rev() {
        used += Line::from(c.to_string()).width();
        if used > budget {
            break;
        }
        tail.push(c);
    }
    format!(":{}", tail.iter().rev().collect::<String>())
}

/// Connection state, shown in the status bar instead of a popup.
fn notice(app: &App) -> Option<String> {
    match &app.status {
        Status::Online if app.state.me().is_some() => None,
        Status::Online => Some("[joining…]".into()),
        Status::Connecting => Some(format!("[connecting to {}…]", app.config.server)),
        Status::Offline(_) => Some("[disconnected — retrying]".into()),
        Status::Rejected(reason) => Some(format!("[rejected: {reason} — esc to quit]")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::LogEntry;

    fn entry(time: u64, kind: LogKind, text: &str) -> LogEntry {
        LogEntry { time, line: LogLine::new(kind, text) }
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_time(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_time(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_time(ISO_REF), "2026-11-20T10:01:28Z");
    }

    /// e.g. `2026-11-20T10:01:28Z INFO  alice joined the world.`
    fn log_text(e: &LogEntry) -> String {
        format!("{} {} {}", iso_time(e.time), level(&e.line).label(), message(&e.line))
    }

    const ISO_REF: u64 = 1_795_168_888;

    #[test]
    fn log_lines_look_like_server_logs() {
        let join = entry(ISO_REF, LogKind::System, "alice joined the world.");
        assert_eq!(log_text(&join), "2026-11-20T10:01:28Z INFO  alice joined the world.");
        let leave = entry(ISO_REF, LogKind::System, "bob left the world.");
        assert!(log_text(&leave).contains(" WARN  bob left"));
        let lost = entry(ISO_REF, LogKind::System, "Disconnected: eof");
        assert_eq!(level(&lost.line), Level::Warn);
        let cow = entry(ISO_REF, LogKind::Animal, "The cow goes \"moo\"");
        assert!(log_text(&cow).contains(" DEBUG The cow"));
        let chat = entry(ISO_REF, LogKind::Chat { from: "bob".into() }, "salut");
        assert!(log_text(&chat).ends_with("INFO  [chat] <bob> salut"));
    }

    #[test]
    fn gutter_aligns_right_and_handles_negatives() {
        assert_eq!(gutter_width(5, 30), 4);
        assert_eq!(gutter_width(-12, 8), 4);
        assert_eq!(gutter_width(-1200, -1180), 6);
        assert_eq!(gutter_line(7, 4), "  7 ");
        assert_eq!(gutter_line(-12, 5), " -12 ");
        assert_eq!(gutter_line(1000, 5), "1000 ");
    }

    #[test]
    fn command_line_keeps_the_tail() {
        assert_eq!(command_line("hi", 20), ":hi");
        assert_eq!(command_line("abcdefgh", 6), ":efgh");
        assert_eq!(command_line("x", 0), ":");
    }

    #[test]
    fn command_line_counts_display_width() {
        // caractères de 2 cellules : largeur 5 = \":\" + curseur + 3 cellules, donc un seul tient
        assert_eq!(command_line("日本語", 5), ":語");
        assert_eq!(command_line("日本語", 8), ":日本語");
        assert_eq!(Line::from(command_line("a日本語", 6)).width(), 5);
    }
}
