use super::{Glyph, Rgb, content_table, glyph};

/// Behaviour parameters shared by every animal. Combining them gives distinct personalities
/// without writing new AI code.
#[derive(Debug, Clone, Copy)]
pub struct Ai {
    /// Milliseconds between two steps.
    pub step_ms: u32,
    /// Probability of standing still on a given step.
    pub idle: f32,
    /// Runs away from players closer than this (0 = never flees).
    pub flee_radius: i32,
    /// Walks toward players closer than this (0 = indifferent).
    pub curious_radius: i32,
    /// Petting it makes it follow the player (petting again releases it).
    pub tameable: bool,
    /// Probability per step of making an ambient sound.
    pub sound_chance: f32,
}

const AI: Ai =
    Ai { step_ms: 900, idle: 0.6, flee_radius: 0, curious_radius: 0, tameable: false, sound_chance: 0.01 };

#[derive(Debug, Clone, Copy)]
pub struct SpeciesDef {
    pub name: &'static str,
    pub glyph: Glyph,
    pub fg: Rgb,
    pub ai: Ai,
    /// Ambient sounds, picked at random.
    pub sounds: &'static [&'static str],
    /// Reactions to being petted, picked at random.
    pub pet: &'static [&'static str],
}

content_table! {
    /// Animal species. Discriminants are the wire/save format: append new rows at the end.
    Species: SpeciesDef => SPECIES {
        Cow => SpeciesDef {
            name: "cow", glyph: glyph('C', 'C', "🐄"), fg: Rgb(230, 220, 210),
            ai: Ai { step_ms: 1400, idle: 0.7, ..AI },
            sounds: &["Moo.", "Mooo!"],
            pet: &["The cow looks at you, chewing slowly.", "The cow lets out a happy moo."],
        },
        Dog => SpeciesDef {
            name: "dog", glyph: glyph('d', 'd', "🐕"), fg: Rgb(205, 150, 90),
            ai: Ai { step_ms: 300, idle: 0.3, curious_radius: 6, tameable: true, sound_chance: 0.02, ..AI },
            sounds: &["Woof!", "Arf arf!"],
            pet: &["The dog wags its tail.", "The dog licks your hand."],
        },
        Sheep => SpeciesDef {
            name: "sheep", glyph: glyph('s', 's', "🐑"), fg: Rgb(240, 240, 240),
            ai: Ai { step_ms: 1100, idle: 0.6, ..AI },
            sounds: &["Baa.", "Baaa!"],
            pet: &["The sheep's wool is soft and warm.", "The sheep bleats contentedly."],
        },
        Chicken => SpeciesDef {
            name: "chicken", glyph: glyph('c', 'c', "🐔"), fg: Rgb(250, 230, 160),
            ai: Ai { step_ms: 450, idle: 0.5, flee_radius: 2, sound_chance: 0.02, ..AI },
            sounds: &["Cluck.", "Bok bok!"],
            pet: &["The chicken pecks at your fingers.", "The chicken flaps away indignantly."],
        },
        Rabbit => SpeciesDef {
            name: "rabbit", glyph: glyph('r', 'r', "🐇"), fg: Rgb(200, 180, 160),
            ai: Ai { step_ms: 250, idle: 0.6, flee_radius: 5, sound_chance: 0.0, ..AI },
            sounds: &[],
            pet: &["The rabbit twitches its nose.", "The rabbit freezes, then relaxes."],
        },
        Cat => SpeciesDef {
            name: "cat", glyph: glyph('f', 'f', "🐈"), fg: Rgb(220, 170, 110),
            ai: Ai { step_ms: 500, idle: 0.7, curious_radius: 3, tameable: true, sound_chance: 0.01, ..AI },
            sounds: &["Meow.", "Mrrp?"],
            pet: &["The cat purrs.", "The cat rubs against your leg."],
        },
        Pig => SpeciesDef {
            name: "pig", glyph: glyph('p', 'p', "🐖"), fg: Rgb(240, 160, 170),
            ai: Ai { step_ms: 1200, idle: 0.6, sound_chance: 0.015, ..AI },
            sounds: &["Oink.", "Snort!"],
            pet: &["The pig snuffles happily.", "The pig rolls over for a belly rub."],
        },
        Duck => SpeciesDef {
            name: "duck", glyph: glyph('u', 'u', "🦆"), fg: Rgb(120, 180, 90),
            ai: Ai { step_ms: 600, idle: 0.5, flee_radius: 2, sound_chance: 0.02, ..AI },
            sounds: &["Quack.", "Quack quack!"],
            pet: &["The duck waddles around you.", "The duck quacks and shakes its feathers."],
        },
        Goat => SpeciesDef {
            name: "goat", glyph: glyph('g', 'g', "🐐"), fg: Rgb(190, 180, 165),
            ai: Ai { step_ms: 400, idle: 0.5, flee_radius: 3, sound_chance: 0.01, ..AI },
            sounds: &["Maaa.", "Meh-eh-eh!"],
            pet: &["The goat nibbles at your sleeve.", "The goat butts your hand gently."],
        },
        Fox => SpeciesDef {
            name: "fox", glyph: glyph('v', 'v', "🦊"), fg: Rgb(225, 120, 50),
            ai: Ai { step_ms: 300, idle: 0.5, flee_radius: 6, sound_chance: 0.005, ..AI },
            sounds: &["Yip!", "A fox barks in the distance."],
            pet: &["The fox sniffs your hand warily.", "The fox lets you touch it, then darts back."],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::PLAYER_GLYPH;
    use std::collections::HashSet;

    #[test]
    fn species_ascii_glyphs_are_unique() {
        let mut seen = HashSet::new();
        for g in SPECIES.iter().map(|s| s.glyph).chain([PLAYER_GLYPH]) {
            assert!(seen.insert(g.ascii), "glyphe ASCII en double : {:?}", g.ascii);
        }
    }
}
