//! Screen layouts.

mod game;
pub mod glyphs;
mod incognito;
pub mod map;
pub mod setup;
pub mod theme;

use ratatui::Frame;
use ratatui::symbols::border;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::App;
use crate::config::GlyphSet;

pub const ASCII_BORDER: border::Set = border::Set {
    top_left: "+",
    top_right: "+",
    bottom_left: "+",
    bottom_right: "+",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

/// Cadre avec bordures `+-|` en ASCII, arrondies sinon.
pub fn bordered(glyphs: GlyphSet) -> Block<'static> {
    let block = Block::bordered();
    if glyphs == GlyphSet::Ascii {
        block.border_set(ASCII_BORDER)
    } else {
        block.border_type(BorderType::Rounded)
    }
}

/// `ascii` si le jeu de glyphes est ASCII, `unicode` sinon (texte du chrome hors carte).
pub fn pick<'a>(glyphs: GlyphSet, unicode: &'a str, ascii: &'a str) -> &'a str {
    if glyphs == GlyphSet::Ascii { ascii } else { unicode }
}

pub fn draw(f: &mut Frame, app: &App) {
    if app.incognito {
        incognito::draw(f, app);
    } else {
        game::draw(f, app);
    }
}

/// `HH:MM:SS` from seconds since midnight.
pub fn clock(secs: u64) -> String {
    format!("{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// Fin de `text` tenant dans `budget` colonnes d'affichage (fenêtre glissante).
fn tail_fit(text: &str, budget: usize) -> String {
    let mut used = 0;
    let mut start = text.len();
    // Par graphème : `❤️` (2 colonnes) se mesure d'un bloc, pas en `❤` (1) + U+FE0F (0).
    for (i, g) in text.grapheme_indices(true).rev() {
        used += Line::from(g).width();
        if used > budget {
            break;
        }
        start = i;
    }
    text[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_fit_counts_graphemes_not_chars() {
        let text = "❤️".repeat(70);
        let tail = tail_fit(&text, 20);
        assert!(Line::from(tail.as_str()).width() <= 20);
        assert_eq!(tail, "❤️".repeat(10));
        assert_eq!(tail_fit("abc", 10), "abc");
    }
}
