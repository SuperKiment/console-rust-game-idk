//! Normal layout: map, side panels, event log.

use std::collections::BTreeMap;
use std::time::Instant;

use idk_shared::content::{PLAYER_COLORS, Rgb};
use idk_shared::protocol::{LogKind, Look};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};

use super::map::{self, MapStyle};
use super::theme::{NORMAL, Theme};
use crate::app::{App, Mode, Status};
use crate::input::BINDINGS;

const SIDEBAR: u16 = 32;
const LOG_HEIGHT: u16 = 9;
const NEARBY_RADIUS: i32 = 12;

pub fn draw(f: &mut Frame, app: &App) {
    let t = &NORMAL;
    let paint = |c: Rgb| app.config.colors.paint(c);
    let [main, log] = Layout::vertical([Constraint::Min(8), Constraint::Length(LOG_HEIGHT)]).areas(f.area());
    let [map_area, side] = Layout::horizontal([Constraint::Min(20), Constraint::Length(SIDEBAR)]).areas(main);

    let block = |title: &str| {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(paint(t.border)))
            .title(Span::styled(format!(" {title} "), Style::new().fg(paint(t.title)).bold()))
    };

    // Map
    let s = &app.state;
    let online = format!(" seed {} · {} online ", s.seed, s.online.len());
    let map_block =
        block("idk").title_top(Line::styled(online, Style::new().fg(paint(t.dim))).right_aligned());
    let inner = map_block.inner(map_area);
    f.render_widget(map_block, map_area);
    let style = MapStyle { glyphs: app.config.glyphs, colors: app.config.colors, theme: t };
    map::render(f.buffer_mut(), inner, s, &style, Instant::now());

    // Side panels
    let keys = BINDINGS.iter().filter(|b| b.help.is_some()).count() as u16;
    let [me_area, online_area, nearby_area, keys_area] = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(3),
        Constraint::Min(3),
        Constraint::Length(keys + 2),
    ])
    .areas(side);
    let dim = Style::new().fg(paint(t.dim));
    let text = Style::new().fg(paint(t.text));
    let kv =
        |k: &str, v: String| Line::from(vec![Span::styled(format!("{k:<8}"), dim), Span::styled(v, text)]);

    let me_lines = match s.me() {
        Some(me) => {
            let facing = me.pos.step(me.facing);
            let faced = s
                .entity_at(facing)
                .map(|e| match &e.look {
                    Look::Player { name, .. } => name.clone(),
                    Look::Animal { species, .. } => species.def().name.to_string(),
                })
                .or_else(|| s.tile(facing).map(|t| t.def().name.to_string()))
                .unwrap_or_default();
            vec![
                kv("name", s.name.clone()),
                kv("pos", format!("{}, {}", me.pos.x, me.pos.y)),
                kv("biome", s.biome(me.pos).map(|b| b.def().name).unwrap_or("?").to_string()),
                kv("facing", format!("{faced} ({})", me.facing.name())),
            ]
        }
        None => vec![Line::styled("not in the world yet", dim)],
    };
    f.render_widget(Paragraph::new(me_lines).block(block("you")), me_area);

    let online_lines: Vec<Line> = s
        .online
        .iter()
        .map(|(name, c)| {
            let color = PLAYER_COLORS[*c as usize % PLAYER_COLORS.len()];
            let mut style = Style::new().fg(paint(color));
            if *name == s.name {
                style = style.add_modifier(Modifier::BOLD);
            }
            Line::from(vec![Span::styled("@ ", style), Span::styled(name.clone(), style)])
        })
        .collect();
    f.render_widget(Paragraph::new(online_lines).block(block("online")), online_area);

    let mut nearby: BTreeMap<&str, (usize, bool)> = BTreeMap::new();
    if let Some(pos) = s.my_pos() {
        for e in s.entities.iter().filter(|e| e.pos.dist(pos) <= NEARBY_RADIUS) {
            if let Look::Animal { species, tamed_by } = &e.look {
                let entry = nearby.entry(species.def().name).or_default();
                entry.0 += 1;
                entry.1 |= tamed_by.as_deref() == Some(s.name.as_str());
            }
        }
    }
    let nearby_lines: Vec<Line> = if nearby.is_empty() {
        vec![Line::styled("nothing around", dim)]
    } else {
        nearby
            .iter()
            .map(|(name, (n, yours))| {
                let mut l = Line::from(vec![
                    Span::styled(format!("{n:>3} "), dim),
                    Span::styled(plural(name, *n), text),
                ]);
                if *yours {
                    l.push_span(Span::styled(" ♥", Style::new().fg(paint(Rgb(255, 95, 135)))));
                }
                l
            })
            .collect()
    };
    f.render_widget(Paragraph::new(nearby_lines).block(block("nearby")), nearby_area);

    let key_lines: Vec<Line> = BINDINGS
        .iter()
        .filter_map(|b| b.help)
        .map(|(k, what)| {
            Line::from(vec![
                Span::styled(format!("{k:<15}"), Style::new().fg(paint(t.accent))),
                Span::styled(what, dim),
            ])
        })
        .collect();
    f.render_widget(Paragraph::new(key_lines).block(block("keys")), keys_area);

    draw_log(
        f,
        app,
        t,
        log,
        block(if matches!(app.mode, Mode::Chat(_)) { "chat · enter send · esc cancel" } else { "log" }),
    );
    draw_status(f, app, t, map_area);
}

fn plural(name: &str, n: usize) -> String {
    match (n, name) {
        (1, _) => name.to_string(),
        (_, "sheep") => name.to_string(),
        _ => format!("{name}s"),
    }
}

fn draw_log(f: &mut Frame, app: &App, t: &Theme, area: Rect, block: Block) {
    let paint = |c: Rgb| app.config.colors.paint(c);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let input_rows = matches!(app.mode, Mode::Chat(_)) as u16;
    let rows = inner.height.saturating_sub(input_rows) as usize;
    let lines: Vec<Line> = app
        .state
        .log
        .iter()
        .rev()
        .take(rows)
        .rev()
        .map(|e| {
            let time =
                Span::styled(format!("{} ", super::clock(e.time % 86_400)), Style::new().fg(paint(t.dim)));
            let body = match &e.line.kind {
                LogKind::Chat { from } => {
                    let c = app.state.online.iter().find(|(n, _)| n == from).map(|(_, c)| *c).unwrap_or(0);
                    let color = PLAYER_COLORS[c as usize % PLAYER_COLORS.len()];
                    vec![
                        Span::styled(format!("<{from}> "), Style::new().fg(paint(color)).bold()),
                        Span::styled(e.line.text.clone(), Style::new().fg(paint(t.text))),
                    ]
                }
                LogKind::System => {
                    vec![Span::styled(e.line.text.clone(), Style::new().fg(paint(t.system)).italic())]
                }
                LogKind::Animal => vec![Span::styled(e.line.text.clone(), Style::new().fg(paint(t.animal)))],
                LogKind::Info => vec![Span::styled(e.line.text.clone(), Style::new().fg(paint(t.text)))],
            };
            Line::from([vec![time], body].concat())
        })
        .collect();
    let [log_area, input_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(input_rows)]).areas(inner);
    f.render_widget(Paragraph::new(lines), log_area);
    if let Mode::Chat(input) = &app.mode {
        let line = Line::from(vec![
            Span::styled("> ", Style::new().fg(paint(t.accent)).bold()),
            Span::styled(input.clone(), Style::new().fg(paint(t.text))),
            Span::styled("█", Style::new().fg(paint(t.accent))),
        ]);
        f.render_widget(Paragraph::new(line), input_area);
    }
}

/// Centered popup while not connected.
fn draw_status(f: &mut Frame, app: &App, t: &Theme, area: Rect) {
    let paint = |c: Rgb| app.config.colors.paint(c);
    let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let frame = spinner[(app.started.elapsed().as_millis() / 80) as usize % spinner.len()];
    let (title, msg) = match &app.status {
        Status::Online if app.state.me().is_some() => return,
        Status::Online => ("joining", format!("{frame} entering the world…")),
        Status::Connecting => ("connecting", format!("{frame} connecting to {}…", app.config.server)),
        Status::Offline(reason) => ("offline", format!("{frame} {reason}\nretrying…")),
        Status::Rejected(reason) => ("rejected", format!("{reason}\npress esc to quit")),
    };
    let w = (msg.lines().map(|l| Line::from(l).width()).max().unwrap_or(0) as u16 + 6).min(area.width);
    let text = Paragraph::new(msg).alignment(Alignment::Center).wrap(Wrap { trim: true });
    // Hauteur après retour à la ligne (bordures : 2 colonnes et 2 lignes)
    let h = (text.line_count(w.saturating_sub(2)) as u16 + 2).min(area.height);
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(paint(t.accent)))
        .title(Span::styled(format!(" {title} "), Style::new().fg(paint(t.title)).bold()));
    f.render_widget(text.block(block).fg(paint(t.text)), popup);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn long_status_reason_keeps_last_line() {
        let app = App::for_test(Status::Offline("x".repeat(200)));
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        term.draw(|f| draw_status(f, &app, &NORMAL, f.area())).unwrap();
        let screen = term.backend().to_string();
        assert!(screen.contains("retrying"), "{screen}");
    }
}
