//! Data-driven game content.
//!
//! Every kind of tile, biome, species and effect is one row of a table declared with
//! [`content_table!`]. Adding content means adding a row: the enum variant, its numeric id
//! on the wire and its definition are generated together, so they cannot drift apart.

mod biomes;
mod effects;
mod species;
mod tiles;

pub use biomes::{BIOMES, Biome, BiomeDef};
pub use effects::{EFFECTS, Effect, EffectDef};
pub use species::{Ai, SPECIES, Species, SpeciesDef};
pub use tiles::{TILES, Tile, TileDef};

/// 24-bit color. Clients downgrade it when the terminal has fewer colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// How a thing looks, for each glyph set the client can pick during setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    /// Plain 7-bit ASCII, works everywhere.
    pub ascii: char,
    /// Single-cell Unicode symbol.
    pub unicode: char,
    /// Exactly two terminal cells (emoji or two symbols), used by the wide glyph set.
    pub wide: &'static str,
}

/// Shorthand constructor used by the content tables.
pub const fn glyph(ascii: char, unicode: char, wide: &'static str) -> Glyph {
    Glyph { ascii, unicode, wide }
}

/// Look of every player avatar (color comes from [`PLAYER_COLORS`]).
pub const PLAYER_GLYPH: Glyph = glyph('@', '@', "🧑");

/// Palette players are colored from (picked by hashing the player's name).
pub const PLAYER_COLORS: &[Rgb] = &[
    Rgb(255, 215, 95),
    Rgb(95, 215, 255),
    Rgb(255, 135, 175),
    Rgb(175, 255, 135),
    Rgb(215, 175, 255),
    Rgb(255, 175, 95),
];

/// Stable color index for a player name.
pub fn player_color_index(name: &str) -> u8 {
    let h = name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    (h % PLAYER_COLORS.len() as u32) as u8
}

/// Declares a content enum, its definition table and the `ALL`/`def()` accessors.
macro_rules! content_table {
    (
        $(#[$meta:meta])*
        $enum:ident : $def:ty => $table:ident {
            $($variant:ident => $value:expr),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[repr(u16)]
        pub enum $enum {
            $($variant),+
        }

        /// Definitions, indexed by the enum discriminant.
        pub static $table: &[$def] = &[$($value),+];

        impl $enum {
            pub const ALL: &'static [$enum] = &[$($enum::$variant),+];

            pub fn def(self) -> &'static $def {
                &$table[self as usize]
            }
        }
    };
}
pub(crate) use content_table;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_glyphs_are_two_cells_or_one_emoji() {
        let glyphs = TILES
            .iter()
            .map(|t| t.glyph)
            .chain(SPECIES.iter().map(|s| s.glyph))
            .chain(EFFECTS.iter().map(|e| e.glyph))
            .chain([PLAYER_GLYPH]);
        for g in glyphs {
            assert!(g.ascii.is_ascii(), "{g:?}");
            let n = g.wide.chars().count();
            assert!(n == 1 || n == 2, "wide glyph must be 2 cells: {g:?}");
        }
    }

    #[test]
    fn biome_rows_reference_valid_content() {
        for b in BIOMES {
            assert!(!b.tiles.is_empty(), "{} has no tiles", b.name);
            assert!(b.elevation.0 < b.elevation.1, "{}", b.name);
        }
    }

    #[test]
    fn species_rows_are_playable() {
        for s in SPECIES {
            assert!(s.ai.step_ms > 0, "{}: step_ms doit être > 0", s.name);
            assert!((0.0..=1.0).contains(&s.ai.idle), "{}: idle", s.name);
            assert!((0.0..=1.0).contains(&s.ai.sound_chance), "{}: sound_chance", s.name);
            assert!(!s.pet.is_empty(), "{}: au moins une phrase de caresse", s.name);
            assert!(s.pet.iter().all(|l| !l.is_empty()), "{}: phrase de caresse vide", s.name);
            assert!(s.ai.sound_chance == 0.0 || !s.sounds.is_empty(), "{}: sound_chance sans sons", s.name);
        }
    }

    #[test]
    fn effects_last_and_tiles_break_into_walkable() {
        for e in EFFECTS {
            assert!(e.duration_ms > 0, "{:?}", e.glyph);
        }
        for t in TILES {
            assert!(!t.name.is_empty(), "tuile sans nom : {:?}", t.glyph);
            if let Some(into) = t.breaks_into {
                assert!(into.def().walkable, "{} se casse en une tuile non praticable", t.name);
            }
        }
    }
}
