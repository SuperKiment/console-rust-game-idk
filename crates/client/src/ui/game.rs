//! Normal layout: map, side panels, event log.

use std::collections::BTreeMap;
use std::time::Instant;

use idk_shared::content::{PLAYER_COLORS, Rgb};
use idk_shared::protocol::{LogKind, Look};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use super::map::{self, MapStyle};
use super::theme::{NORMAL, Theme};
use super::{bordered, pick};
use crate::app::{App, Mode, Status};
use crate::config::GlyphSet;
use crate::input::BINDINGS;

const SIDEBAR: u16 = 32;
const LOG_HEIGHT: u16 = 9;
const SEED_MIN_WIDTH: u16 = 40;
const LOG_MIN_HEIGHT: u16 = 4;
const ME_HEIGHT: u16 = 6;
const LIST_MIN_HEIGHT: u16 = 3;
const NEARBY_RADIUS: i32 = 12;

pub fn draw(f: &mut Frame, app: &App) {
    let t = &NORMAL;
    let paint = |c: Rgb| app.config.colors.paint(c);
    let g = app.config.glyphs;
    // Journal : au plus un quart de la hauteur, jamais sous LOG_MIN_HEIGHT
    let log_h = (f.area().height / 4).clamp(LOG_MIN_HEIGHT, LOG_HEIGHT);
    let [main, log] = Layout::vertical([Constraint::Min(8), Constraint::Length(log_h)]).areas(f.area());
    let [map_area, side] = Layout::horizontal([Constraint::Min(20), Constraint::Length(SIDEBAR)]).areas(main);

    let block = |title: &str| {
        bordered(g)
            .border_style(Style::new().fg(paint(t.border)))
            .title(Span::styled(format!(" {title} "), Style::new().fg(paint(t.title)).bold()))
    };

    // Map
    let s = &app.state;
    // Seed en hex court (8 caractères), seulement si le titre « idk » a la place
    let online = if map_area.width >= SEED_MIN_WIDTH {
        format!(" seed {:08x} {} {} online ", s.seed as u32, pick(g, "·", "-"), s.online.len())
    } else {
        format!(" {} online ", s.online.len())
    };
    let map_block =
        block("idk").title_top(Line::styled(online, Style::new().fg(paint(t.dim))).right_aligned());
    let inner = map_block.inner(map_area);
    f.render_widget(map_block, map_area);
    let style = MapStyle { glyphs: app.config.glyphs, colors: app.config.colors, theme: t };
    map::render(f.buffer_mut(), inner, s, &style, Instant::now());

    // Side panels
    // Priorité de place : you > keys (entier ou masqué) > online > nearby
    let keys = BINDINGS.iter().filter(|b| b.help.is_some()).count() as u16;
    let keys_h = keys + 2;
    let [me_area, rest] = Layout::vertical([Constraint::Length(ME_HEIGHT), Constraint::Min(0)]).areas(side);
    let show_keys = rest.height >= keys_h;
    let (keys_area, lists) = if show_keys {
        let [k, l] = Layout::vertical([Constraint::Length(keys_h), Constraint::Min(0)]).areas(rest);
        (Some(k), l)
    } else {
        (None, rest)
    };
    let show_online = lists.height >= LIST_MIN_HEIGHT;
    let show_nearby = lists.height >= 2 * LIST_MIN_HEIGHT;
    let [online_area, nearby_area] = Layout::vertical([
        Constraint::Length(if show_nearby { lists.height / 2 } else { lists.height }),
        Constraint::Min(0),
    ])
    .areas(lists);
    let dim = Style::new().fg(paint(t.dim));
    let text = Style::new().fg(paint(t.text));
    let kv =
        |k: &str, v: String| Line::from(vec![Span::styled(format!("{k:<8}"), dim), Span::styled(v, text)]);

    let me_lines = match s.me() {
        Some(me) => {
            let facing = me.pos.step(s.facing());
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
                kv("facing", format!("{faced} ({})", s.facing().name())),
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
    if show_online {
        f.render_widget(Paragraph::new(online_lines).block(block("online")), online_area);
    }

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
                    l.push_span(Span::styled(
                        format!(" {}", pick(g, "♥", "<3")),
                        Style::new().fg(paint(Rgb(255, 95, 135))),
                    ));
                }
                l
            })
            .collect()
    };
    if show_nearby {
        f.render_widget(Paragraph::new(nearby_lines).block(block("nearby")), nearby_area);
    }

    let key_lines: Vec<Line> = BINDINGS
        .iter()
        .filter_map(|b| b.help)
        .map(|(k, what)| {
            // Les flèches ne sont pas ASCII
            let k = if g == GlyphSet::Ascii { k.replace("↑←↓→", "arrows") } else { k.to_string() };
            Line::from(vec![
                Span::styled(format!("{k:<17}"), Style::new().fg(paint(t.accent))),
                Span::styled(what, dim),
            ])
        })
        .collect();
    if let Some(keys_area) = keys_area {
        f.render_widget(Paragraph::new(key_lines).block(block("keys")), keys_area);
    }

    draw_log(
        f,
        app,
        t,
        log,
        block(if matches!(app.mode, Mode::Chat(_)) {
            pick(g, "chat · enter send · esc cancel", "chat - enter send - esc cancel")
        } else {
            "log"
        }),
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
    // Retour à la ligne, puis défilement pour garder les lignes les plus récentes visibles
    let log_text = Paragraph::new(lines).wrap(Wrap { trim: false });
    let hidden = log_text.line_count(log_area.width).saturating_sub(log_area.height as usize);
    f.render_widget(log_text.scroll((hidden.min(u16::MAX as usize) as u16, 0)), log_area);
    if let Mode::Chat(input) = &app.mode {
        let line = Line::from(vec![
            Span::styled("> ", Style::new().fg(paint(t.accent)).bold()),
            Span::styled(
                super::tail_fit(input, (input_area.width as usize).saturating_sub(3)),
                Style::new().fg(paint(t.text)),
            ),
            Span::styled(pick(app.config.glyphs, "█", "_"), Style::new().fg(paint(t.accent))),
        ]);
        f.render_widget(Paragraph::new(line), input_area);
    }
}

/// Centered popup while not connected.
fn draw_status(f: &mut Frame, app: &App, t: &Theme, area: Rect) {
    let paint = |c: Rgb| app.config.colors.paint(c);
    let g = app.config.glyphs;
    let spinner: &[char] = if g == GlyphSet::Ascii {
        &['|', '/', '-', '\\']
    } else {
        &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
    };
    let frame = spinner[(app.started.elapsed().as_millis() / 80) as usize % spinner.len()];
    let dots = pick(g, "…", "...");
    let (title, msg) = match &app.status {
        Status::Online if app.state.me().is_some() => return,
        Status::Online => ("joining", format!("{frame} entering the world{dots}")),
        Status::Connecting => ("connecting", format!("{frame} connecting to {}{dots}", app.config.server)),
        Status::Offline(reason) => ("offline", format!("{frame} {reason}\nretrying{dots}")),
        Status::Rejected(reason) => ("rejected", format!("{reason}\npress esc to quit")),
    };
    let w = (msg.lines().map(|l| Line::from(l).width()).max().unwrap_or(0) as u16 + 6).min(area.width);
    let text = Paragraph::new(msg).alignment(Alignment::Center).wrap(Wrap { trim: true });
    // Hauteur après retour à la ligne (bordures : 2 colonnes et 2 lignes)
    let h = (text.line_count(w.saturating_sub(2)) as u16 + 2).min(area.height);
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    let block = bordered(g)
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

    fn screen(app: &App) -> String {
        screen_at(app, 80, 24)
    }

    fn screen_at(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| draw(f, app)).unwrap();
        term.backend().to_string()
    }

    #[test]
    fn long_chat_input_keeps_end_and_cursor() {
        let mut app = App::for_test(Status::Online);
        app.mode = Mode::Chat(format!("{}END", "x".repeat(150)));
        let s = screen(&app);
        assert!(s.contains("END█"), "{s}");
    }

    #[test]
    fn long_log_message_wraps_and_stays_at_bottom() {
        use idk_shared::protocol::LogLine;
        let mut app = App::for_test(Status::Online);
        let line = |text: String| LogLine { kind: LogKind::Info, text };
        app.state
            .log
            .push_back(crate::state::LogEntry { time: 0, line: line(format!("{}FINMSG", "y".repeat(200))) });
        app.state.log.push_back(crate::state::LogEntry { time: 0, line: line("derniere".into()) });
        let s = screen(&app);
        assert!(s.contains("FINMSG"), "{s}");
        assert!(s.contains("derniere"), "{s}");
    }

    /// App connectée avec un joueur au centre d'un chunk de plaine.
    fn app_with_me() -> App {
        use idk_shared::geo::{Dir, Pos};
        use idk_shared::protocol::{EntityView, ServerMsg};
        let mut app = App::for_test(Status::Online);
        let now = Instant::now();
        app.state
            .apply(ServerMsg::Welcome { id: 1, name: "bob".into(), seed: 12_345_678_901_234_567_890 }, now);
        let look = Look::Player { name: "bob".into(), color: 0 };
        app.state.apply(
            ServerMsg::Entities(vec![EntityView {
                id: 1,
                pos: Pos { x: 3, y: 4 },
                facing: Dir::South,
                look,
            }]),
            now,
        );
        app
    }

    #[test]
    fn small_terminals_keep_me_panel() {
        let app = app_with_me();
        for (w, h) in [(80, 24), (60, 18)] {
            let s = screen_at(&app, w, h);
            assert!(s.contains("pos"), "{w}x{h}\n{s}");
            assert!(s.contains("biome"), "{w}x{h}\n{s}");
            assert!(s.contains("idk"), "{w}x{h}\n{s}");
        }
    }

    #[test]
    fn keys_panel_is_whole_or_hidden() {
        let app = app_with_me();
        assert!(screen_at(&app, 120, 40).contains("quit"));
        assert!(!screen_at(&app, 60, 18).contains("keys"));
    }

    #[test]
    fn ascii_glyphs_leave_no_unicode_in_chrome() {
        use crate::config::GlyphSet;
        // Connecté, chat ouvert, puis popup offline sur une autre app
        let mut app = app_with_me();
        app.config.glyphs = GlyphSet::Ascii;
        app.mode = Mode::Chat("salut".into());
        let s = screen_at(&app, 120, 40);
        assert!(s.is_ascii(), "{s}");
        let mut off = App::for_test(Status::Offline("connection lost".into()));
        off.config.glyphs = GlyphSet::Ascii;
        let s = screen_at(&off, 120, 40);
        assert!(s.contains("retrying...") && s.is_ascii(), "{s}");
        // Incognito
        app.incognito = true;
        app.mode = Mode::Play;
        assert!(screen_at(&app, 120, 40).is_ascii());
        off.incognito = true;
        assert!(screen_at(&off, 120, 40).is_ascii());
    }
}
