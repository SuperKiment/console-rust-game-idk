//! Picks the symbol and color of things according to the terminal capabilities chosen at setup.

use idk_shared::content::{Glyph, Rgb};
use ratatui::style::Color;

use crate::config::{ColorDepth, GlyphSet};

impl GlyphSet {
    /// Terminal cells per map tile.
    pub fn tile_width(self) -> u16 {
        if self == GlyphSet::Wide { 2 } else { 1 }
    }

    /// Symbol of `g` in this set, written into `buf`.
    pub fn symbol<'a>(self, g: &Glyph, buf: &'a mut [u8; 8]) -> &'a str {
        match self {
            GlyphSet::Ascii => g.ascii.encode_utf8(buf),
            GlyphSet::Unicode => g.unicode.encode_utf8(buf),
            GlyphSet::Wide => {
                let n = g.wide.len().min(8);
                buf[..n].copy_from_slice(&g.wide.as_bytes()[..n]);
                std::str::from_utf8(&buf[..n]).unwrap_or("??")
            }
        }
    }

    /// Wall symbol joined to neighbours. `mask` bits: 1 = north, 2 = east, 4 = south, 8 = west.
    pub fn wall(self, mask: u8, buf: &mut [u8; 8]) -> &str {
        const LINES: [char; 16] =
            ['■', '║', '═', '╚', '║', '║', '╔', '╠', '═', '╝', '═', '╩', '╗', '╣', '╦', '╬'];
        let c = LINES[(mask & 15) as usize];
        match self {
            GlyphSet::Ascii => '#'.encode_utf8(buf),
            GlyphSet::Unicode => c.encode_utf8(buf),
            GlyphSet::Wide => {
                let n = c.encode_utf8(buf).len();
                let tail = if mask & 2 != 0 { '═' } else { ' ' };
                let m = tail.encode_utf8(&mut buf[n..]).len();
                std::str::from_utf8(&buf[..n + m]).unwrap_or("##")
            }
        }
    }
}

impl ColorDepth {
    pub fn paint(self, Rgb(r, g, b): Rgb) -> Color {
        match self {
            ColorDepth::Truecolor => Color::Rgb(r, g, b),
            ColorDepth::Ansi256 => Color::Indexed(ansi256(r, g, b)),
        }
    }
}

/// Nearest xterm-256 color (6x6x6 cube or grayscale ramp).
fn ansi256(r: u8, g: u8, b: u8) -> u8 {
    let level = |v: u8| ((v as u16 * 5 + 127) / 255) as u8;
    let cube = 16 + 36 * level(r) + 6 * level(g) + level(b);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    if max - min < 12 {
        // Near-gray: the 24-step ramp is finer than the cube's diagonal.
        let avg = (r as u16 + g as u16 + b as u16) / 3;
        if avg < 8 {
            16
        } else if avg > 238 {
            231
        } else {
            232 + ((avg - 8) / 10) as u8
        }
    } else {
        cube
    }
}

/// `c` moved toward white by `t` (0..1).
pub fn lighten(Rgb(r, g, b): Rgb, t: f32) -> Rgb {
    let f = |v: u8| (v as f32 + (255.0 - v as f32) * t) as u8;
    Rgb(f(r), f(g), f(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idk_shared::content::glyph;

    #[test]
    fn symbols_per_set() {
        let g = glyph('T', '♣', "🌳");
        let mut b = [0; 8];
        assert_eq!(GlyphSet::Ascii.symbol(&g, &mut b), "T");
        assert_eq!(GlyphSet::Unicode.symbol(&g, &mut b), "♣");
        assert_eq!(GlyphSet::Wide.symbol(&g, &mut b), "🌳");
    }

    #[test]
    fn walls_join_neighbours() {
        let mut b = [0; 8];
        assert_eq!(GlyphSet::Unicode.wall(2 | 8, &mut b), "═");
        assert_eq!(GlyphSet::Unicode.wall(2 | 4, &mut b), "╔");
        assert_eq!(GlyphSet::Wide.wall(2 | 4, &mut b), "╔═");
        assert_eq!(GlyphSet::Wide.wall(1, &mut b), "║ ");
    }

    #[test]
    fn ansi256_mapping() {
        assert_eq!(ansi256(255, 0, 0), 196);
        assert_eq!(ansi256(0, 0, 0), 16);
        assert_eq!(ansi256(128, 128, 128), 244);
    }
}
