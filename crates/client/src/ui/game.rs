//! Normal layout: map, side panels, event log.

use std::collections::BTreeMap;
use std::time::Instant;

use idk_shared::content::{PLAYER_COLORS, Rgb};
use idk_shared::geo::Pos;
use idk_shared::protocol::{LogKind, Look};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use super::map::{self, MapStyle};
use super::theme::{NORMAL, Theme};
use super::{bordered, pick};
use crate::app::{App, Mode, Status};
use crate::config::GlyphSet;
use crate::input::BINDINGS;
use crate::state::{shifted, utc_offset};

const SIDEBAR: u16 = 32;
const MAP_MIN_WIDTH: u16 = 20;
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
    // Terminal étroit : pas de sidebar, la carte prend toute la largeur.
    let wide = main.width >= MAP_MIN_WIDTH + SIDEBAR;
    let [map_area, side] = if wide {
        Layout::horizontal([Constraint::Min(MAP_MIN_WIDTH), Constraint::Length(SIDEBAR)]).areas(main)
    } else {
        [main, Rect::default()]
    };

    let block = |title: &str| {
        bordered(g)
            .border_style(Style::new().fg(paint(t.border)))
            .title(Span::styled(format!(" {title} "), Style::new().fg(paint(t.title)).bold()))
    };

    // Map
    let s = &app.state;
    // Seed en hex court (8 caractères), seulement si le titre « idk » a la place
    // Pas de seed avant le premier Welcome (elle reste connue hors ligne)
    let online = if map_area.width >= SEED_MIN_WIDTH && (s.me.is_some() || s.is_offline()) {
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
    // Priorité de place : you > bag > keys (entier ou masqué) > online > nearby
    let keys = BINDINGS.iter().filter(|b| b.help.is_some()).count() as u16;
    let keys_h = keys + 2;
    let [me_area, below_me] =
        Layout::vertical([Constraint::Length(ME_HEIGHT), Constraint::Min(0)]).areas(side);
    // Sac entier ou masqué : une ligne par objet (ou `empty`), plus les 2 bordures.
    let bag_h = s.inventory.len().max(1) as u16 + 2;
    let show_bag = below_me.height >= bag_h;
    let (bag_area, rest) = if show_bag {
        let [b, r] = Layout::vertical([Constraint::Length(bag_h), Constraint::Min(0)]).areas(below_me);
        (Some(b), r)
    } else {
        (None, below_me)
    };
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
        None => me_offline_lines(s.is_offline(), s.map_center(), dim, text),
    };
    f.render_widget(Paragraph::new(me_lines).block(block("you")), me_area);

    if let Some(bag_area) = bag_area {
        let inner_width = bag_area.width.saturating_sub(2) as usize;
        let lines = bag_lines(app, t, inner_width);
        f.render_widget(Paragraph::new(lines).block(block("bag")), bag_area);
    }

    // Lignes dispo = hauteur moins les 2 bordures ; si ça déborde, la dernière devient `+K more`.
    let avail = online_area.height.saturating_sub(2) as usize;
    let shown = if s.online.len() > avail { avail.saturating_sub(1) } else { s.online.len() };
    let mut online_lines: Vec<Line> = s
        .online
        .iter()
        .take(shown)
        .map(|(name, c)| {
            let color = PLAYER_COLORS[*c as usize % PLAYER_COLORS.len()];
            let mut style = Style::new().fg(paint(color));
            if *name == s.name {
                style = style.add_modifier(Modifier::BOLD);
            }
            Line::from(vec![Span::styled("@ ", style), Span::styled(name.clone(), style)])
        })
        .collect();
    if s.online.len() > shown && avail > 0 {
        online_lines.push(Line::styled(format!("+{} more", s.online.len() - shown), dim));
    }
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
        block(&if matches!(app.mode, Mode::Chat(_)) {
            pick(g, "chat · enter send · esc cancel", "chat - enter send - esc cancel").to_string()
        } else if app.state.log_scroll > 0 {
            format!("log {}{}", pick(g, "↓", "v"), app.state.log_scroll)
        } else {
            "log".to_string()
        }),
    );
    draw_status(f, app, t, map_area);
}

/// Panneau « you » sans joueur : hors ligne (dernière position connue gardée) ou pas encore entré.
fn me_offline_lines(offline: bool, last: Option<Pos>, dim: Style, text: Style) -> Vec<Line<'static>> {
    if !offline {
        return vec![Line::styled("not in the world yet", dim)];
    }
    let mut lines = vec![Line::styled("offline", dim)];
    if let Some(p) = last {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<8}", "pos"), dim),
            Span::styled(format!("{}, {}", p.x, p.y), text),
        ]));
    }
    lines
}

/// Lignes du panneau « bag » : glyphe, nom, puis compte aligné à droite dans `width` colonnes.
fn bag_lines(app: &App, t: &Theme, width: usize) -> Vec<Line<'static>> {
    let g = app.config.glyphs;
    let paint = |c: Rgb| app.config.colors.paint(c);
    let dim = Style::new().fg(paint(t.dim));
    if app.state.inventory.is_empty() {
        return vec![Line::styled("empty", dim)];
    }
    app.state
        .inventory
        .iter()
        .map(|(item, n)| {
            let def = item.def();
            let mut buf = [0; 8];
            let symbol = g.symbol(&def.glyph, &mut buf);
            let count = format!("{}{n}", pick(g, "×", "x"));
            // Largeur réelle (glyphes Wide sur 2 colonnes) pour aligner le compte à droite.
            let used = Line::from(symbol).width()
                + 1
                + Line::from(def.name).width()
                + Line::from(count.as_str()).width();
            Line::from(vec![
                Span::styled(symbol.to_string(), Style::new().fg(paint(def.fg))),
                Span::raw(" "),
                Span::styled(def.name, Style::new().fg(paint(t.text))),
                Span::raw(" ".repeat(width.saturating_sub(used))),
                Span::styled(count, dim),
            ])
        })
        .collect()
}

fn plural(name: &str, n: usize) -> String {
    if n == 1 || name == "sheep" {
        name.to_string()
    } else if ["x", "s", "sh", "ch"].iter().any(|e| name.ends_with(e)) {
        format!("{name}es")
    } else {
        format!("{name}s")
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
        .visible_log(rows)
        .map(|e| {
            let time = Span::styled(
                format!("{} ", super::clock(shifted(e.time, utc_offset()) % 86_400)),
                Style::new().fg(paint(t.dim)),
            );
            let body = match &e.line.kind {
                LogKind::Chat { from } => {
                    let color = PLAYER_COLORS[e.color as usize % PLAYER_COLORS.len()];
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
    render_tail(f, lines, log_area);
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

/// Affiche `lines` avec retour à la ligne, puis défilement pour garder les lignes les plus récentes
/// visibles. Commun au log du layout normal et à celui du layout incognito.
pub(super) fn render_tail(f: &mut Frame, lines: Vec<Line>, area: Rect) {
    // Pas de bloc sur ce `Paragraph` : `area.width` est la largeur réellement utilisée.
    let lines = prewrap_long_words(lines, area.width as usize);
    let text = Paragraph::new(lines).wrap(Wrap { trim: false });
    let hidden = text.line_count(area.width).saturating_sub(area.height as usize);
    f.render_widget(text.scroll((hidden.min(u16::MAX as usize) as u16, 0)), area);
}

/// Le `Wrap` de ratatui renvoie à la ligne un mot plus large que la zone avant de le couper, ce qui
/// laisse le début de la ligne seul. Les lignes contenant un tel mot sont donc découpées ici par
/// colonnes (par graphème, styles conservés) ; les autres passent inchangées.
fn prewrap_long_words(lines: Vec<Line<'_>>, width: usize) -> Vec<Line<'_>> {
    if width == 0 {
        return lines;
    }
    let has_long_word = |line: &Line| {
        let mut run = 0;
        for span in &line.spans {
            for g in span.content.graphemes(true) {
                if g.chars().all(char::is_whitespace) {
                    run = 0;
                } else {
                    run += Line::from(g).width();
                    if run > width {
                        return true;
                    }
                }
            }
        }
        false
    };
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        if !has_long_word(&line) {
            out.push(line);
            continue;
        }
        let mut row: Vec<Span> = Vec::new();
        let mut used = 0;
        for span in &line.spans {
            for g in span.content.graphemes(true) {
                let w = Line::from(g).width();
                // Un graphème large qui ne tient pas dans la colonne restante passe à la ligne suivante.
                if used + w > width && used > 0 {
                    out.push(Line::from(std::mem::take(&mut row)).style(line.style));
                    used = 0;
                }
                match row.last_mut() {
                    Some(last) if last.style == span.style => last.content.to_mut().push_str(g),
                    _ => row.push(Span::styled(g.to_string(), span.style)),
                }
                used += w;
            }
        }
        if !row.is_empty() {
            out.push(Line::from(row).style(line.style));
        }
    }
    out
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
    fn plural_rules() {
        assert_eq!(plural("fox", 2), "foxes");
        assert_eq!(plural("sheep", 2), "sheep");
        assert_eq!(plural("cat", 2), "cats");
        assert_eq!(plural("fox", 1), "fox");
    }

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
        app.state.log.push_back(crate::state::LogEntry {
            time: 0,
            line: line(format!("{}FINMSG", "y".repeat(200))),
            color: 0,
        });
        app.state.log.push_back(crate::state::LogEntry { time: 0, line: line("derniere".into()), color: 0 });
        let s = screen(&app);
        assert!(s.contains("FINMSG"), "{s}");
        assert!(s.contains("derniere"), "{s}");
    }

    #[test]
    fn prewrap_cuts_long_words_by_grapheme_and_keeps_styles() {
        let red = Style::new().fg(ratatui::style::Color::Red);
        let line = Line::from(vec![Span::raw("ab "), Span::styled("日".repeat(5), red)]);
        let out = prewrap_long_words(vec![Line::from("court mot"), line], 5);
        assert_eq!(out.len(), 1 + 3);
        assert_eq!(out[0].to_string(), "court mot");
        // `日` fait 2 colonnes : 2 par ligne dans 5 colonnes, jamais coupé en deux.
        assert_eq!(out[1].to_string(), "ab 日");
        assert_eq!(out[2].to_string(), "日日");
        assert_eq!(out[3].to_string(), "日日");
        assert!(out[1..].iter().all(|l| l.width() <= 5));
        assert_eq!(out[2].spans[0].style, red);
    }

    #[test]
    fn prewrap_survives_graphemes_wider_than_area() {
        let out = prewrap_long_words(vec![Line::from("日日")], 1);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn long_word_chat_starts_on_the_name_line() {
        use idk_shared::protocol::LogLine;
        let mut app = App::for_test(Status::Online);
        let kind = LogKind::Chat { from: "alice".into() };
        let text = format!("{}FINMSG", "A".repeat(300));
        app.state.log.push_back(crate::state::LogEntry { time: 0, line: LogLine { kind, text }, color: 0 });
        let s = screen_at(&app, 120, 40);
        let row = s.lines().find(|l| l.contains("<alice>")).expect(&s);
        assert!(row.contains("<alice> AAAA"), "{s}");
        assert!(s.contains("FINMSG"), "{s}");
    }

    #[test]
    fn online_panel_says_how_many_are_hidden() {
        let mut app = App::for_test(Status::Online);
        for i in 0..8 {
            app.state.online.push((format!("p{i}"), i));
        }
        let s = screen_at(&app, 120, 40);
        assert!(s.contains("more"), "{s}");
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
    fn me_panel_says_offline_with_last_position() {
        let mut app = app_with_me();
        app.state.disconnect();
        app.status = Status::Offline("lost".into());
        let s = screen_at(&app, 120, 40);
        assert!(s.contains("offline") && s.contains("pos") && s.contains("3, 4"), "{s}");
        assert!(!s.contains("not in the world yet"), "{s}");
        // Avant le premier Welcome : pas encore dans le monde
        let fresh = App::for_test(Status::Connecting);
        assert!(screen_at(&fresh, 120, 40).contains("not in the world yet"));
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
    fn bag_panel_lists_items_with_counts() {
        use idk_shared::content::Item;
        let mut app = app_with_me();
        app.state.inventory = vec![(Item::Wood, 12), (Item::Stone, 3)];
        let s = screen_at(&app, 120, 40);
        assert!(s.contains("bag") && s.contains("wood") && s.contains("12"), "{s}");
        assert!(s.contains("stone") && s.contains("×3"), "{s}");
        // Compte aligné à droite, dans la même ligne que le nom
        let row = s.lines().find(|l| l.contains("wood")).expect(&s);
        assert!(row.contains("×12"), "{row}");
    }

    #[test]
    fn empty_bag_says_empty() {
        let s = screen_at(&app_with_me(), 120, 40);
        assert!(s.contains("bag") && s.contains("empty"), "{s}");
    }

    #[test]
    fn bag_count_stays_aligned_in_wide_glyphs() {
        use crate::config::GlyphSet;
        use idk_shared::content::Item;
        let mut app = app_with_me();
        app.config.glyphs = GlyphSet::Wide;
        app.state.inventory = vec![(Item::Wood, 12), (Item::Flower, 7)];
        let width = |name: &str| {
            let mut app = app_with_me();
            app.config.glyphs = GlyphSet::Wide;
            app.state.inventory = vec![(Item::Wood, 12), (Item::Flower, 7)];
            let lines = bag_lines(&app, &NORMAL, 30);
            lines.iter().find(|l| l.to_string().contains(name)).map(|l| l.width()).unwrap()
        };
        assert_eq!(width("wood"), 30);
        assert_eq!(width("flower"), 30);
        let s = screen_at(&app, 120, 40);
        assert!(s.contains("12"), "{s}");
    }

    #[test]
    fn bag_panel_survives_small_terminals() {
        use idk_shared::content::Item;
        let mut app = app_with_me();
        app.state.inventory = Item::ALL.iter().map(|i| (*i, 5)).collect();
        for (w, h) in [(80, 24), (60, 18), (52, 12)] {
            let s = screen_at(&app, w, h);
            assert!(s.contains("pos"), "{w}x{h}\n{s}");
        }
    }

    #[test]
    fn ascii_glyphs_leave_no_unicode_in_chrome() {
        use crate::config::GlyphSet;
        // Connecté, chat ouvert, puis popup offline sur une autre app
        let mut app = app_with_me();
        app.state.inventory =
            vec![(idk_shared::content::Item::Wood, 12), (idk_shared::content::Item::Flower, 1)];
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
