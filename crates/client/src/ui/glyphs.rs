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

/// Niveaux xterm de chaque composante du cube 6x6x6 (non linéaires).
const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Distance euclidienne au carré entre deux couleurs RGB.
fn dist2(a: [u8; 3], b: [u8; 3]) -> u32 {
    a.iter().zip(b).map(|(&x, y)| (x as i32 - y as i32).pow(2) as u32).sum()
}

/// Nearest xterm-256 color (6x6x6 cube or grayscale ramp), by RGB distance.
fn ansi256(r: u8, g: u8, b: u8) -> u8 {
    let rgb = [r, g, b];
    // Niveau du cube le plus proche, par composante.
    let nearest =
        |v: u8| -> usize { (0..6).min_by_key(|&i| (v as i32 - CUBE_LEVELS[i] as i32).abs()).unwrap_or(0) };
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube_rgb = [CUBE_LEVELS[ri], CUBE_LEVELS[gi], CUBE_LEVELS[bi]];
    let cube_idx = 16 + 36 * ri + 6 * gi + bi;
    // Rampe de gris 232..=255 : valeur = 8 + 10 * i.
    let avg = (r as u16 + g as u16 + b as u16) / 3;
    let gi = ((avg.saturating_sub(3)) / 10).min(23) as usize;
    let gray = (8 + 10 * gi) as u8;
    if dist2(rgb, [gray; 3]) < dist2(rgb, cube_rgb) { (232 + gi) as u8 } else { cube_idx as u8 }
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
        assert_eq!(ansi256(255, 255, 255), 231);
        // Exact cube color (95,135,175) = 16 + 36*1 + 6*2 + 3.
        assert_eq!(ansi256(95, 135, 175), 67);
    }

    #[test]
    fn ansi256_dark_backgrounds_stay_dark() {
        // Plains: ancien résultat 22 = (0,95,0), bien trop clair.
        assert_eq!(ansi256(20, 36, 18), 234);
        // Plus de gris cube (95,95,95) pour un fond très sombre.
        assert!(ansi256(30, 30, 30) >= 232);
    }
}
