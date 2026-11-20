use super::{Glyph, Rgb, content_table, glyph};

#[derive(Debug, Clone, Copy)]
pub struct TileDef {
    pub name: &'static str,
    pub glyph: Glyph,
    pub fg: Rgb,
    pub bg: Rgb,
    /// Players and animals can stand on it.
    pub walkable: bool,
    /// What it becomes when a player breaks it (`None` = unbreakable).
    pub breaks_into: Option<Tile>,
    /// Rendered with box-drawing lines joined to neighbouring tiles of the same kind.
    pub connects: bool,
}

/// Defaults for a plain walkable ground tile; rows override what differs.
const GROUND: TileDef = TileDef {
    name: "",
    glyph: glyph('.', '·', "· "),
    fg: Rgb(120, 120, 120),
    bg: Rgb(0, 0, 0),
    walkable: true,
    breaks_into: None,
    connects: false,
};

/// Defaults for an obstacle that can be broken back into dirt.
const OBSTACLE: TileDef = TileDef { walkable: false, breaks_into: Some(Tile::Dirt), ..GROUND };

content_table! {
    /// Kind of a world tile. Discriminants are the wire/save format: append new rows at the end.
    Tile: TileDef => TILES {
        Grass => TileDef { name: "grass", glyph: glyph('.', '·', "· "), fg: Rgb(98, 160, 72), bg: Rgb(18, 34, 16), ..GROUND },
        TallGrass => TileDef { name: "tall grass", glyph: glyph('"', '"', "\" "), fg: Rgb(120, 190, 80), bg: Rgb(18, 34, 16), ..GROUND },
        Flowers => TileDef { name: "flowers", glyph: glyph('*', '✿', "🌼"), fg: Rgb(235, 200, 90), bg: Rgb(18, 34, 16), ..GROUND },
        Dirt => TileDef { name: "dirt", glyph: glyph('.', '·', "· "), fg: Rgb(150, 110, 70), bg: Rgb(36, 26, 16), ..GROUND },
        Sand => TileDef { name: "sand", glyph: glyph('.', '∙', "∙ "), fg: Rgb(220, 200, 130), bg: Rgb(58, 50, 28), ..GROUND },
        Snow => TileDef { name: "snow", glyph: glyph('.', '·', "· "), fg: Rgb(230, 235, 245), bg: Rgb(60, 64, 72), ..GROUND },
        Gravel => TileDef { name: "gravel", glyph: glyph(',', '∴', "∴ "), fg: Rgb(140, 140, 140), bg: Rgb(30, 30, 32), ..GROUND },
        Water => TileDef { name: "water", glyph: glyph('~', '≈', "≈≈"), fg: Rgb(110, 170, 240), bg: Rgb(16, 40, 90), walkable: false, ..GROUND },
        DeepWater => TileDef { name: "deep water", glyph: glyph('~', '≈', "≈≈"), fg: Rgb(70, 110, 200), bg: Rgb(8, 20, 60), walkable: false, ..GROUND },
        Tree => TileDef { name: "tree", glyph: glyph('T', '♣', "🌳"), fg: Rgb(60, 170, 70), bg: Rgb(18, 34, 16), breaks_into: Some(Tile::Grass), ..OBSTACLE },
        Pine => TileDef { name: "pine", glyph: glyph('A', '♠', "🌲"), fg: Rgb(40, 130, 90), bg: Rgb(18, 30, 22), breaks_into: Some(Tile::Grass), ..OBSTACLE },
        Cactus => TileDef { name: "cactus", glyph: glyph('!', '¥', "🌵"), fg: Rgb(110, 180, 80), bg: Rgb(58, 50, 28), breaks_into: Some(Tile::Sand), ..OBSTACLE },
        Rock => TileDef { name: "rock", glyph: glyph('^', '▲', "▲▲"), fg: Rgb(160, 150, 140), bg: Rgb(40, 38, 36), breaks_into: Some(Tile::Gravel), ..OBSTACLE },
        Wall => TileDef { name: "wall", glyph: glyph('#', '#', "##"), fg: Rgb(200, 190, 170), bg: Rgb(48, 44, 40), connects: true, ..OBSTACLE },
    }
}

impl Tile {
    /// Tile placed by the "build" action.
    pub const BUILT: Tile = Tile::Wall;
}
