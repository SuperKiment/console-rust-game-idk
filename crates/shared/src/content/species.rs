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
    }
}
