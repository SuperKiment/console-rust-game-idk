//! Map rendering, shared by every layout.

use std::time::Instant;

use idk_shared::content::{PLAYER_COLORS, PLAYER_GLYPH, Rgb, Tile};
use idk_shared::geo::{Dir, Pos};
use idk_shared::protocol::{EntityView, Look};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::glyphs::lighten;
use super::theme::Theme;
use crate::config::{ColorDepth, GlyphSet};
use crate::state::GameState;

pub struct MapStyle<'a> {
    pub glyphs: GlyphSet,
    pub colors: ColorDepth,
    pub theme: &'a Theme,
}

/// World position shown at the top-left cell, so that `center` is in the middle of `area`.
pub fn origin(area: Rect, glyphs: GlyphSet, center: Pos) -> Pos {
    let cols = (area.width / glyphs.tile_width()) as i32;
    Pos::new(center.x - cols / 2, center.y - area.height as i32 / 2)
}

pub fn render(buf: &mut Buffer, area: Rect, state: &GameState, s: &MapStyle, now: Instant) {
    let Some(me) = state.me() else { return };
    let tw = s.glyphs.tile_width();
    let o = origin(area, s.glyphs, me.pos);
    let facing = me.pos.step(me.facing);
    let paint = |c: Rgb| s.colors.paint(c);
    let mut sym = [0u8; 8];

    for row in 0..area.height {
        for col in 0..area.width / tw {
            let p = Pos::new(o.x + col as i32, o.y + row as i32);
            let (x, y) = (area.x + col * tw, area.y + row);
            let Some(tile) = state.tile(p) else {
                let blank =
                    if s.theme.map_colors { Style::new().bg(paint(s.theme.unknown)) } else { Style::new() };
                buf.set_stringn(x, y, "  ", tw as usize, blank);
                continue;
            };
            let def = tile.def();
            let mut style = Style::new();
            let mut ground = false;
            let mut text = if let Some(effect) = state.effect_at(p, now) {
                let d = effect.def();
                style = style.fg(paint(d.fg)).add_modifier(Modifier::BOLD);
                s.glyphs.symbol(&d.glyph, &mut sym)
            } else if let Some(e) = state.entity_at(p) {
                let (glyph, color) = entity_look(e);
                style = style.fg(paint(color));
                if Some(e.id) == state.me
                    || matches!(&e.look, Look::Animal { tamed_by: Some(n), .. } if *n == state.name)
                {
                    style = style.add_modifier(Modifier::BOLD);
                }
                s.glyphs.symbol(glyph, &mut sym)
            } else if def.connects {
                style = style.fg(paint(def.fg));
                s.glyphs.wall(wall_mask(state, p, tile), &mut sym)
            } else {
                ground = def.walkable && def.bg.is_none();
                style = style.fg(paint(def.fg));
                s.glyphs.symbol(&def.glyph, &mut sym)
            };
            if s.theme.map_colors {
                let ground = def.bg.or_else(|| state.biome(p).map(|b| b.def().bg)).unwrap_or(s.theme.unknown);
                let bg = if p == facing { lighten(ground, 0.22) } else { ground };
                style = style.bg(paint(bg));
            } else {
                // Monochrome: open ground is blank (sparse like a text file), the rest is plain text.
                style = style.fg(paint(s.theme.text));
                if ground {
                    text = " ";
                }
                if p == facing {
                    style = style.add_modifier(Modifier::UNDERLINED);
                }
            }
            buf.set_stringn(x, y, text, tw as usize, style);
        }
    }
}

fn entity_look(e: &EntityView) -> (&'static idk_shared::content::Glyph, Rgb) {
    match &e.look {
        Look::Player { color, .. } => (&PLAYER_GLYPH, PLAYER_COLORS[*color as usize % PLAYER_COLORS.len()]),
        Look::Animal { species, .. } => (&species.def().glyph, species.def().fg),
    }
}

fn wall_mask(state: &GameState, p: Pos, tile: Tile) -> u8 {
    [(Dir::North, 1), (Dir::East, 2), (Dir::South, 4), (Dir::West, 8)]
        .into_iter()
        .filter(|(d, _)| state.tile(p.step(*d)) == Some(tile))
        .map(|(_, bit)| bit)
        .sum()
}
