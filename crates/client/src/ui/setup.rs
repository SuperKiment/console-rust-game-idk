//! Screens of the first-launch wizard (logic in `crate::setup`).

use idk_shared::content::{PLAYER_COLORS, PLAYER_GLYPH, Rgb, Species, Tile};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use super::theme::NORMAL;
use crate::config::{ColorDepth, GlyphSet};
use crate::setup::{Step, Wizard};

const WIDTH: u16 = 74;
const HEIGHT: u16 = 20;
const GRADIENT_CELLS: u16 = 40;

/// Sample map shown with each glyph set (ground colors come from the tiles' own `bg` or `GROUND_BG`).
const PREVIEW: [[Tile; 12]; 3] = {
    use Tile::*;
    [
        [DeepWater, Water, Water, Sand, Sand, Grass, Flowers, Grass, Tree, Tree, Pine, Pine],
        [Water, Water, Sand, Gravel, Grass, TallGrass, Grass, Tree, Grass, Rock, Pine, Grass],
        [Water, Sand, Sand, Grass, Grass, Flowers, Grass, Grass, Rock, Rock, Grass, Cactus],
    ]
};
/// Preview cell showing the player avatar, and the one showing a cow.
const PLAYER_AT: (usize, usize) = (1, 5);
const COW_AT: (usize, usize) = (2, 6);
const GROUND_BG: Rgb = Rgb(20, 36, 18);

pub fn draw(f: &mut Frame, w: &Wizard) {
    let t = &NORMAL;
    let paint = |c: Rgb| w.colors.paint(c);
    let area = f.area();
    let area = Rect::new(
        area.x + area.width.saturating_sub(WIDTH) / 2,
        area.y + area.height.saturating_sub(HEIGHT) / 2,
        WIDTH.min(area.width),
        HEIGHT.min(area.height),
    );
    let block = super::bordered(w.glyphs)
        .border_style(Style::new().fg(paint(t.border)))
        .title(Span::styled(" idk · setup ", Style::new().fg(paint(t.title)).bold()));
    let inner = block.inner(area).inner(ratatui::layout::Margin::new(2, 1));
    f.render_widget(block, area);

    let [body, hint] = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let text = Style::new().fg(paint(t.text));
    let dim = Style::new().fg(paint(t.dim));
    let accent = Style::new().fg(paint(t.accent)).bold();
    let ask = |q: &str, keys: &str| {
        vec![Line::from(""), Line::styled(q.to_string(), text.bold()), Line::styled(keys.to_string(), accent)]
    };

    let mut lines: Vec<Line> = Vec::new();
    // Lignes d'aperçu de carte : retirées si le terminal est trop bas, la ligne de touches reste visible.
    let mut previews: Vec<std::ops::Range<usize>> = Vec::new();
    match w.step {
        Step::Unicode => {
            lines.push(Line::styled("Unicode symbols", accent));
            lines.push(Line::from(""));
            lines.push(symbols_line(
                w,
                &[Tile::Tree, Tile::Pine, Tile::Water, Tile::Flowers, Tile::Rock, Tile::Grass],
            ));
            lines.push(Line::from(""));
            lines.extend(frame_lines(w));
            lines.extend(ask("Do these symbols display correctly?", "(y) yes   (n) no"));
        }
        Step::EmojiWidth => {
            let (tree, cow) = (Tile::Tree.def().glyph.wide, Species::Cow.def().glyph.wide);
            lines.push(Line::styled("Emoji width", accent));
            lines.push(Line::from(""));
            lines.push(Line::styled(format!("|{cow}|{tree}|"), text));
            lines.push(Line::styled("|xx|xx|", text));
            lines.extend(ask("Are the vertical bars aligned?", "(y) yes   (n) no"));
        }
        Step::GlyphSet => {
            lines.push(Line::styled("Which map style do you prefer?", accent));
            for (label, set) in [
                ("1) Unicode (1 cell per tile)", GlyphSet::Unicode),
                ("2) Wide (emoji, 2 cells per tile)", GlyphSet::Wide),
            ] {
                lines.push(Line::from(""));
                lines.push(Line::styled(label, text.bold()));
                let start = lines.len();
                lines.extend(preview(set, w.colors));
                previews.push(start..lines.len());
            }
            lines.extend(ask("", "(1) Unicode   (2) Wide"));
        }
        Step::Colors => {
            lines.push(Line::styled("Colors", accent));
            lines.push(Line::from(""));
            lines.push(gradient());
            lines.extend(ask("Is the gradient smooth (no visible bands)?", "(y) yes   (n) no"));
        }
        Step::Name => {
            lines.push(Line::styled("Player", accent));
            lines.extend(ask("Your in-game name", "Enter to confirm"));
            lines.push(Line::from(""));
            lines.push(input_line(&w.name_input, &w.base.name, text, dim));
        }
        Step::Server => {
            lines.push(Line::styled("Server", accent));
            lines.extend(ask("Server address (host:port)", "Enter to confirm"));
            lines.push(Line::from(""));
            lines.push(input_line(&w.server_input, &w.base.server, text, dim));
        }
        Step::Incognito => {
            lines.push(Line::styled("Incognito", accent));
            let default = if w.incognito { "yes" } else { "no" };
            lines.extend(ask(
                "Start in incognito mode by default?",
                &format!("(y) yes   (n) no   Enter = {default}"),
            ));
        }
        Step::Summary => {
            let c = w.config();
            lines.push(Line::styled("Summary", accent));
            lines.push(Line::from(""));
            for (k, v) in [
                ("name", c.name),
                ("server", c.server),
                ("glyphs", format!("{:?}", c.glyphs).to_lowercase()),
                ("colors", format!("{:?}", c.colors).to_lowercase()),
                ("incognito", if c.incognito { "yes" } else { "no" }.to_string()),
            ] {
                lines.push(Line::from(vec![Span::styled(format!("{k:<11}"), dim), Span::styled(v, text)]));
            }
            let start = lines.len();
            lines.push(Line::from(""));
            lines.extend(preview(c.glyphs, c.colors));
            previews.push(start..lines.len());
            lines.push(Line::from(""));
            lines.push(Line::styled("Enter: save", accent));
        }
    }
    if lines.len() > body.height as usize {
        for r in previews.into_iter().rev() {
            lines.drain(r);
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), body);

    let back = if w.step == Step::Unicode { "" } else { "   Backspace: back" };
    f.render_widget(
        Paragraph::new(format!("Esc: quit without saving{back}")).style(dim).alignment(Alignment::Left),
        hint,
    );
}

/// Tile symbols in Unicode, spaced out so a missing glyph is obvious.
fn symbols_line(w: &Wizard, tiles: &[Tile]) -> Line<'static> {
    let spans = tiles.iter().flat_map(|t| {
        let d = t.def();
        [
            Span::styled(d.glyph.unicode.to_string(), Style::new().fg(w.colors.paint(d.fg)).bold()),
            Span::raw("   "),
        ]
    });
    Line::from(spans.collect::<Vec<_>>())
}

/// `╔═╗║╚╝` as a 3-line box, built with the same wall table as the game.
fn frame_lines(w: &Wizard) -> Vec<Line<'static>> {
    let mut b = [0; 8];
    // Wall masks: 1 north, 2 east, 4 south, 8 west.
    let mut piece = |mask| GlyphSet::Unicode.wall(mask, &mut b).to_string();
    let style = Style::new().fg(w.colors.paint(NORMAL.text));
    let row = |l: String, m: String, r: String| Line::styled(format!("{l}{m}{m}{m}{r}"), style);
    let (h, v) = (piece(2 | 8), piece(1 | 4));
    vec![
        row(piece(2 | 4), h.clone(), piece(4 | 8)),
        row(v.clone(), " ".into(), v),
        row(piece(1 | 2), h, piece(1 | 8)),
    ]
}

/// Truecolor ramp red to blue. Always `Color::Rgb`: the point is to test the terminal.
fn gradient() -> Line<'static> {
    let last = (GRADIENT_CELLS - 1) as f32;
    let cells = (0..GRADIENT_CELLS).map(|i| {
        let t = i as f32 / last;
        Span::styled(" ", Style::new().bg(Color::Rgb((255.0 * (1.0 - t)) as u8, 0, (255.0 * t) as u8)))
    });
    Line::from(cells.collect::<Vec<_>>())
}

/// Typed text with a cursor, or the dim default when nothing is typed yet.
fn input_line(typed: &str, default: &str, text: Style, dim: Style) -> Line<'static> {
    let (shown, style) = if typed.is_empty() { (default, dim) } else { (typed, text) };
    Line::from(vec![
        Span::styled("> ", text),
        Span::styled(shown.to_string(), style),
        Span::styled("▏", text),
    ])
}

/// Three rows of sample map drawn with `set` and `colors`, same symbols and colors as the game.
fn preview(set: GlyphSet, colors: ColorDepth) -> Vec<Line<'static>> {
    let mut buf = [0u8; 8];
    PREVIEW
        .iter()
        .enumerate()
        .map(|(r, row)| {
            let spans = row.iter().enumerate().map(|(c, tile)| {
                let d = tile.def();
                let (glyph, fg) = match (r, c) {
                    PLAYER_AT => (&PLAYER_GLYPH, PLAYER_COLORS[0]),
                    COW_AT => (&Species::Cow.def().glyph, Species::Cow.def().fg),
                    _ => (&d.glyph, d.fg),
                };
                let style = Style::new().fg(colors.paint(fg)).bg(colors.paint(d.bg.unwrap_or(GROUND_BG)));
                Span::styled(set.symbol(glyph, &mut buf).to_string(), style)
            });
            Line::from(spans.collect::<Vec<_>>())
        })
        .collect()
}
