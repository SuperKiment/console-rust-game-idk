use super::{Glyph, Rgb, content_table, glyph};

/// Short-lived visual drawn over a tile by clients.
#[derive(Debug, Clone, Copy)]
pub struct EffectDef {
    pub glyph: Glyph,
    pub fg: Rgb,
    pub duration_ms: u32,
}

content_table! {
    Effect: EffectDef => EFFECTS {
        Heart => EffectDef { glyph: glyph('<', '♥', "💖"), fg: Rgb(255, 95, 135), duration_ms: 900 },
        Sound => EffectDef { glyph: glyph('!', '♪', "🎵"), fg: Rgb(255, 255, 175), duration_ms: 700 },
        Dust => EffectDef { glyph: glyph('*', '✶', "💥"), fg: Rgb(200, 180, 140), duration_ms: 350 },
    }
}
