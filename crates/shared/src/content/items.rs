use super::{Glyph, Rgb, content_table, glyph};

#[derive(Debug, Clone, Copy)]
pub struct ItemDef {
    pub name: &'static str,
    pub glyph: Glyph,
    pub fg: Rgb,
}

content_table! {
    /// Kind of an item. Discriminants are the wire/save format: append new rows at the end.
    Item: ItemDef => ITEMS {
        Wood => ItemDef { name: "wood", glyph: glyph('=', '≡', "🪵"), fg: Rgb(150, 110, 70) },
        Stone => ItemDef { name: "stone", glyph: glyph('o', '●', "🪨"), fg: Rgb(160, 150, 140) },
        Fiber => ItemDef { name: "fiber", glyph: glyph('%', '§', "🌾"), fg: Rgb(110, 180, 80) },
        Flower => ItemDef { name: "flower", glyph: glyph('*', '✿', "🌼"), fg: Rgb(235, 200, 90) },
        Berry => ItemDef { name: "berry", glyph: glyph(':', '•', "🫐"), fg: Rgb(200, 60, 90) },
        Mushroom => ItemDef { name: "mushroom", glyph: glyph('?', 'τ', "🍄"), fg: Rgb(200, 160, 120) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::TILES;

    #[test]
    fn dropping_tiles_are_breakable() {
        for t in TILES {
            if t.drops.is_some() {
                assert!(t.breaks_into.is_some(), "{} a un drop mais ne se casse pas", t.name);
            }
        }
    }

    #[test]
    fn buildable_tiles_are_breakable() {
        for t in TILES {
            if !t.cost.is_empty() {
                assert!(t.breaks_into.is_some(), "{} se construit mais ne se casse pas", t.name);
            }
        }
    }

    #[test]
    fn planks_are_walkable_and_buildable() {
        let def = crate::content::Tile::Planks.def();
        assert!(def.walkable);
        assert_eq!(def.cost, &[(Item::Wood, 1)]);
        assert_eq!(def.drops, Some(Item::Wood));
    }

    #[test]
    fn fence_blocks_and_is_buildable() {
        let def = crate::content::Tile::Fence.def();
        assert!(!def.walkable);
        assert!(!def.cost.is_empty());
    }

    #[test]
    fn flowers_drop_flower() {
        let def = crate::content::Tile::Flowers.def();
        assert_eq!(def.drops, Some(Item::Flower));
        assert!(def.walkable, "les fleurs restent marchables");
    }

    #[test]
    fn berry_bush_drops_berry_and_empties() {
        use crate::content::Tile;
        let def = Tile::BerryBush.def();
        assert_eq!(def.drops, Some(Item::Berry));
        assert_eq!(def.breaks_into, Some(Tile::BushEmpty));
        assert_eq!(Tile::BushEmpty.def().drops, None);
    }

    #[test]
    fn mushroom_drops_mushroom_and_is_walkable() {
        let def = crate::content::Tile::Mushroom.def();
        assert_eq!(def.drops, Some(Item::Mushroom));
        assert!(def.walkable, "les champignons restent marchables");
    }

    #[test]
    fn item_names_are_unique() {
        for (i, a) in ITEMS.iter().enumerate() {
            assert!(!a.name.is_empty(), "objet sans nom : {:?}", a.glyph);
            assert!(ITEMS[i + 1..].iter().all(|b| b.name != a.name), "nom d'objet en double : {}", a.name);
        }
    }
}
