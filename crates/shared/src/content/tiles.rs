use super::{Glyph, Item, Rgb, content_table, glyph};

#[derive(Debug, Clone, Copy)]
pub struct TileDef {
    pub name: &'static str,
    pub glyph: Glyph,
    pub fg: Rgb,
    /// Background override; `None` uses the biome's ground color.
    pub bg: Option<Rgb>,
    /// Players and animals can stand on it.
    pub walkable: bool,
    /// What it becomes when a player breaks it (`None` = unbreakable).
    pub breaks_into: Option<Tile>,
    /// Item given to the player who breaks it (`None` = nothing).
    pub drops: Option<Item>,
    /// Objects consumed to build it (empty = cannot be built).
    pub cost: &'static [(Item, u16)],
    /// Rendered with box-drawing lines joined to neighbouring tiles of the same kind.
    pub connects: bool,
    /// Interact switches to this tile (`None` = nothing happens).
    pub toggles_to: Option<Tile>,
}

/// Defaults for a plain walkable ground tile; rows override what differs.
const GROUND: TileDef = TileDef {
    name: "",
    glyph: glyph('.', '·', "· "),
    fg: Rgb(120, 120, 120),
    bg: None,
    walkable: true,
    breaks_into: None,
    drops: None,
    cost: &[],
    connects: false,
    toggles_to: None,
};

/// Defaults for an obstacle that can be broken back into dirt.
const OBSTACLE: TileDef = TileDef { walkable: false, breaks_into: Some(Tile::Dirt), ..GROUND };

content_table! {
    /// Kind of a world tile. Discriminants are the wire/save format: append new rows at the end.
    Tile: TileDef => TILES {
        Grass => TileDef { name: "grass", glyph: glyph('.', '·', "· "), fg: Rgb(98, 160, 72), ..GROUND },
        TallGrass => TileDef { name: "tall grass", glyph: glyph('"', '"', "\" "), fg: Rgb(120, 190, 80), ..GROUND },
        Flowers => TileDef { name: "flowers", glyph: glyph('*', '✿', "🌼"), fg: Rgb(235, 200, 90), breaks_into: Some(Tile::Grass), drops: Some(Item::Flower), ..GROUND },
        Dirt => TileDef { name: "dirt", glyph: glyph('.', '·', "· "), fg: Rgb(150, 110, 70), ..GROUND },
        Sand => TileDef { name: "sand", glyph: glyph('.', '∙', "∙ "), fg: Rgb(220, 200, 130), ..GROUND },
        Snow => TileDef { name: "snow", glyph: glyph('.', '·', "· "), fg: Rgb(230, 235, 245), ..GROUND },
        Gravel => TileDef { name: "gravel", glyph: glyph(',', '∴', "∴ "), fg: Rgb(140, 140, 140), ..GROUND },
        Water => TileDef { name: "water", glyph: glyph('~', '≈', "≈≈"), fg: Rgb(110, 170, 240), bg: Some(Rgb(16, 40, 90)), walkable: false, ..GROUND },
        DeepWater => TileDef { name: "deep water", glyph: glyph('~', '≈', "≈≈"), fg: Rgb(70, 110, 200), bg: Some(Rgb(8, 20, 60)), walkable: false, ..GROUND },
        Tree => TileDef { name: "tree", glyph: glyph('T', '♣', "🌳"), fg: Rgb(60, 170, 70), breaks_into: Some(Tile::Grass), drops: Some(Item::Wood), ..OBSTACLE },
        Pine => TileDef { name: "pine", glyph: glyph('A', '♠', "🌲"), fg: Rgb(40, 130, 90), breaks_into: Some(Tile::Grass), drops: Some(Item::Wood), ..OBSTACLE },
        Cactus => TileDef { name: "cactus", glyph: glyph('!', '¥', "🌵"), fg: Rgb(110, 180, 80), breaks_into: Some(Tile::Sand), drops: Some(Item::Fiber), ..OBSTACLE },
        Rock => TileDef { name: "rock", glyph: glyph('^', '▲', "▲▲"), fg: Rgb(160, 150, 140), bg: Some(Rgb(40, 38, 36)), breaks_into: Some(Tile::Gravel), drops: Some(Item::Stone), ..OBSTACLE },
        Wall => TileDef { name: "wall", glyph: glyph('#', '#', "##"), fg: Rgb(200, 190, 170), bg: Some(Rgb(48, 44, 40)), drops: Some(Item::Stone), cost: &[(Item::Stone, 1)], connects: true, ..OBSTACLE },
        Door => TileDef { name: "door", glyph: glyph('+', '▣', "🚪"), fg: Rgb(170, 110, 60), bg: Some(Rgb(70, 45, 25)), drops: Some(Item::Wood), cost: &[(Item::Wood, 2)], toggles_to: Some(Tile::OpenDoor), ..OBSTACLE },
        OpenDoor => TileDef { name: "open door", glyph: glyph('\'', '▢', "▢ "), fg: Rgb(170, 110, 60), bg: Some(Rgb(70, 45, 25)), breaks_into: Some(Tile::Dirt), drops: Some(Item::Wood), toggles_to: Some(Tile::Door), ..GROUND },
        Planks => TileDef { name: "planks", glyph: glyph('=', '▤', "▤▤"), fg: Rgb(190, 140, 90), bg: Some(Rgb(90, 60, 30)), breaks_into: Some(Tile::Dirt), drops: Some(Item::Wood), cost: &[(Item::Wood, 1)], ..GROUND },
        Fence => TileDef { name: "fence", glyph: glyph('|', '┼', "┼─"), fg: Rgb(170, 120, 70), drops: Some(Item::Wood), cost: &[(Item::Wood, 1)], ..OBSTACLE },
    }
}
