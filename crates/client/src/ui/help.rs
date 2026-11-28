//! Écran d'aide (F1) : popup plein écran avec toutes les touches et la légende des glyphes.

use crossterm::event::KeyCode;
use idk_shared::content::{Item, PLAYER_COLORS, PLAYER_GLYPH, Rgb, Species, Tile};
use ratatui::Frame;
use ratatui::layout::Margin;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::theme::{NORMAL, Theme};
use super::{bordered, pick};
use crate::app::{App, Mode};
use crate::config::GlyphSet;
use crate::input::{Action, BINDINGS};

/// Marge entre le bord du terminal et le popup.
const MARGIN: u16 = 2;
/// Largeur de la colonne des touches.
const KEYS_WIDTH: usize = 14;

pub fn draw(f: &mut Frame, app: &App) {
    let Mode::Help { scroll } = app.mode else { return };
    let t = &NORMAL;
    let g = app.config.glyphs;
    let paint = |c: Rgb| app.config.colors.paint(c);
    // `inner` renvoie un rectangle vide si le terminal est trop petit : pas de dépassement.
    let popup = f.area().inner(Margin::new(MARGIN, MARGIN));
    let title = format!(" help {} esc to close ", pick(g, "—", "-"));
    let block = bordered(g)
        .border_style(Style::new().fg(paint(t.accent)))
        .title(Span::styled(title, Style::new().fg(paint(t.title)).bold()));
    let body = block.inner(popup);
    let lines = lines(app, t);
    // Borne simple : lignes de contenu moins hauteur visible. Mémorisée pour que ↑/↓ de `App` s'y arrêtent.
    let max = lines.len().saturating_sub(body.height as usize).min(u16::MAX as usize) as u16;
    app.help_max_scroll.set(max);
    f.render_widget(Clear, popup);
    f.render_widget(Paragraph::new(lines).block(block).scroll((scroll.min(max), 0)), popup);
}

/// Contenu complet de l'aide : touches, puis tuiles, puis animaux, puis objets.
fn lines(app: &App, t: &Theme) -> Vec<Line<'static>> {
    let g = app.config.glyphs;
    let paint = |c: Rgb| app.config.colors.paint(c);
    let dim = Style::new().fg(paint(t.dim));
    let heading = |name: &str| Line::styled(name.to_string(), Style::new().fg(paint(t.title)).bold());
    let arrow = pick(g, "→", "->");
    let mut out = vec![heading("keys")];
    for b in BINDINGS {
        let keys: Vec<String> = b.keys.iter().map(|k| key_name(*k, g)).collect();
        out.push(Line::from(vec![
            Span::styled(format!("{:<KEYS_WIDTH$}", keys.join(" ")), Style::new().fg(paint(t.accent))),
            Span::styled(format!("{arrow} "), dim),
            Span::styled(action_label(b.action), Style::new().fg(paint(t.text))),
        ]));
    }
    // Ctrl+C n'est pas dans `BINDINGS` (géré à part par `action_for`).
    out.push(Line::from(vec![
        Span::styled(format!("{:<KEYS_WIDTH$}", "ctrl-c"), Style::new().fg(paint(t.accent))),
        Span::styled(format!("{arrow} "), dim),
        Span::styled(action_label(Action::Quit), Style::new().fg(paint(t.text))),
    ]));

    out.push(Line::raw(""));
    out.push(heading("tiles"));
    for tile in Tile::ALL {
        let def = tile.def();
        let mut buf = [0; 8];
        // Les tuiles `connects` sont dessinées par `wall` sur la carte : on montre le mur isolé.
        let symbol = if def.connects { g.wall(0, &mut buf) } else { g.symbol(&def.glyph, &mut buf) };
        out.push(legend(g, symbol, def.fg, def.name, t, &paint));
    }

    out.push(Line::raw(""));
    out.push(heading("animals"));
    for species in Species::ALL {
        let def = species.def();
        let mut buf = [0; 8];
        out.push(legend(g, g.symbol(&def.glyph, &mut buf), def.fg, def.name, t, &paint));
    }
    // Couleur du joueur : première de la palette (chaque joueur a la sienne, hachée du nom).
    let mut buf = [0; 8];
    out.push(legend(g, g.symbol(&PLAYER_GLYPH, &mut buf), PLAYER_COLORS[0], "player", t, &paint));

    out.push(Line::raw(""));
    out.push(heading("items"));
    for item in Item::ALL {
        let def = item.def();
        let mut buf = [0; 8];
        out.push(legend(g, g.symbol(&def.glyph, &mut buf), def.fg, def.name, t, &paint));
    }
    out
}

/// Ligne de légende : symbole coloré (colonne de 3 cellules) puis nom.
fn legend(
    g: GlyphSet,
    symbol: &str,
    fg: Rgb,
    name: &str,
    t: &Theme,
    paint: &impl Fn(Rgb) -> ratatui::style::Color,
) -> Line<'static> {
    let pad = " ".repeat(3 - g.tile_width() as usize);
    Line::from(vec![
        Span::styled(symbol.to_string(), Style::new().fg(paint(fg))),
        Span::raw(pad),
        Span::styled(name.to_string(), Style::new().fg(paint(t.text))),
    ])
}

/// Nom lisible d'une touche (flèches en lettres en ASCII).
fn key_name(code: KeyCode, g: GlyphSet) -> String {
    match code {
        KeyCode::Up => pick(g, "↑", "up").into(),
        KeyCode::Down => pick(g, "↓", "down").into(),
        KeyCode::Left => pick(g, "←", "left").into(),
        KeyCode::Right => pick(g, "→", "right").into(),
        KeyCode::Char(' ') => "space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::F(n) => format!("F{n}"),
        KeyCode::PageUp => "pgup".into(),
        KeyCode::PageDown => "pgdn".into(),
        KeyCode::End => "end".into(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        other => format!("{other:?}"),
    }
}

fn action_label(action: Action) -> String {
    match action {
        Action::Move(dir) => format!("move {}", dir.name()),
        Action::Interact => "interact, pet".into(),
        Action::Build => "build".into(),
        Action::PrevBuild => "previous block".into(),
        Action::NextBuild => "next block".into(),
        Action::Break => "break".into(),
        Action::Mark => "mark position".into(),
        Action::Chat => "chat".into(),
        Action::ToggleIncognito => "incognito".into(),
        Action::Help => "help".into(),
        Action::LogUp => "scroll log up".into(),
        Action::LogDown => "scroll log down".into(),
        Action::LogEnd => "log back to bottom".into(),
        Action::Quit => "quit now".into(),
        Action::ConfirmQuit => "quit (press twice)".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Status;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn help_app(glyphs: GlyphSet) -> App {
        let mut app = App::for_test(Status::Online);
        app.config.glyphs = glyphs;
        app.mode = Mode::Help { scroll: 0 };
        app
    }

    fn screen(app: &App, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| crate::ui::draw(f, app)).unwrap();
        term.backend().to_string()
    }

    #[test]
    fn lists_hidden_alternatives_and_legend() {
        let app = help_app(GlyphSet::Unicode);
        // Fenêtre assez haute pour tout voir ; `z` et `q` sont des alternatives masquées du panneau keys.
        let s = screen(&app, 100, 80);
        assert!(s.contains("w k z") && s.contains("a h q"), "{s}");
        assert!(s.contains("space") && s.contains("F1") && s.contains("pgup"), "{s}");
        assert!(s.contains("help — esc to close"), "{s}");
        assert!(s.contains("cow") && s.contains("tall grass") && s.contains("player"), "{s}");
        assert!(s.contains("items") && s.contains("wood") && s.contains("stone"), "{s}");
    }

    #[test]
    fn wall_legend_shows_isolated_wall() {
        let wall_line = |glyphs| {
            let s = screen(&help_app(glyphs), 100, 80);
            s.lines().find(|l| l.contains("wall") && !l.contains("build")).unwrap_or_default().to_string()
        };
        let unicode = wall_line(GlyphSet::Unicode);
        assert!(unicode.contains('■') && !unicode.contains('#'), "{unicode}");
        assert!(wall_line(GlyphSet::Ascii).contains('#'));
    }

    #[test]
    fn ascii_set_leaves_no_unicode_in_popup() {
        let app = help_app(GlyphSet::Ascii);
        let s = screen(&app, 100, 80);
        assert!(s.contains("help - esc to close") && s.contains("->"), "{s}");
        assert!(s.is_ascii(), "{s}");
    }

    #[test]
    fn wide_set_renders() {
        let app = help_app(GlyphSet::Wide);
        assert!(screen(&app, 100, 80).contains("cow"));
    }

    #[test]
    fn tiny_terminals_do_not_panic() {
        for (w, h) in [(20, 8), (4, 4), (3, 3), (1, 1), (80, 24)] {
            let mut app = help_app(GlyphSet::Unicode);
            app.mode = Mode::Help { scroll: 999 };
            screen(&app, w, h);
        }
    }

    #[test]
    fn scroll_is_bounded_to_content() {
        let mut app = help_app(GlyphSet::Ascii);
        app.mode = Mode::Help { scroll: 999 };
        let s = screen(&app, 80, 24);
        // Tout en bas : dernière ligne de la légende visible, haut (section keys) masqué.
        assert!(s.contains("flower") && !s.contains("move north"), "{s}");
        assert!(app.help_max_scroll.get() > 0);
    }
}
