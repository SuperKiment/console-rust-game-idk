//! Messages exchanged between client and server.
//!
//! Each message is serialized with postcard and framed with a 4-byte length prefix
//! (see [`crate::net`]). Bump [`PROTOCOL_VERSION`] on any incompatible change.

use serde::{Deserialize, Serialize};

use crate::content::{Biome, Effect, Item, Species, Tile};
use crate::geo::{CHUNK_AREA, ChunkPos, Dir, Pos};

pub const PROTOCOL_VERSION: u32 = 4;
pub const DEFAULT_PORT: u16 = 7777;
/// Radius (in chunks) of the area the server streams around each player.
pub const VIEW_CHUNKS: i32 = 3;
/// Chunks farther than this are forgotten by both sides: the server resends them on return.
/// The client must not drop a chunk the server still considers sent, hence a single constant.
pub const KEEP_CHUNKS: i32 = VIEW_CHUNKS + 1;
/// Longueur max d'un nom de joueur : le serveur tronque au-delà, l'assistant client bride la saisie.
pub const MAX_NAME: usize = 16;
/// Longueur max d'un message de chat : le serveur tronque au-delà, le client bride la saisie.
pub const MAX_CHAT: usize = 240;

/// Caractère à ne jamais afficher dans un nom ou un chat, sans contexte : contrôle, séparateurs de
/// ligne/paragraphe (2028-2029), U+2800, FFF9-FFFB, ou toute la propriété Unicode
/// `Default_Ignorable_Code_Point` (00AD, 034F, 061C, 115F-1160, 17B4-17B5, 180B-180F, 200B-200F,
/// 202A-202E, 2060-206F, 3164, FE00-FE0F, FEFF, FFA0, FFF0-FFF8, 1BCA0-1BCA3, 1D173-1D17A,
/// E0000-E0FFF). U+200D (ZWJ) reste permis : il compose les emoji. Les sélecteurs de variante sont
/// cachés ici, mais [`is_hidden_after`] les garde derrière un caractère visible (`❤️`).
pub fn is_hidden_char(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{00AD}' | '\u{034F}' | '\u{061C}' | '\u{115F}'..='\u{1160}'
            | '\u{17B4}'..='\u{17B5}' | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200C}'
            | '\u{200E}'..='\u{200F}' | '\u{2028}'..='\u{202E}' | '\u{2060}'..='\u{206F}'
            | '\u{2800}' | '\u{3164}' | '\u{FE00}'..='\u{FE0F}' | '\u{FEFF}' | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFFB}' | '\u{1BCA0}'..='\u{1BCA3}' | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}')
}

/// Sélecteur de variante (FE00-FE0F, E0100-E01EF).
fn is_variation_selector(c: char) -> bool {
    matches!(c, '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}')
}

/// Comme [`is_hidden_char`], avec le contexte : `prev` est le dernier caractère conservé. Un sélecteur
/// de variante est gardé seulement derrière un caractère conservé non blanc qui n'est pas lui-même
/// un sélecteur (`❤` + FE0F) ; seul, en tête ou après un blanc il est caché.
pub fn is_hidden_after(c: char, prev: Option<char>) -> bool {
    if is_variation_selector(c) {
        return !prev.is_some_and(|p| !p.is_whitespace() && !is_variation_selector(p));
    }
    is_hidden_char(c)
}

/// Retire les caractères cachés de `s` (voir [`is_hidden_after`]).
pub fn strip_hidden(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !is_hidden_after(c, out.chars().next_back()) {
            out.push(c);
        }
    }
    out
}

/// Marque combinante courante (approximation de Mn/Me sans dépendance) : n'affiche rien seule.
fn is_combining_mark(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

/// Vrai s'il reste un caractère visible : ni blanc, ni caché, ni ZWJ seul, ni marque combinante.
pub fn has_visible_char(s: &str) -> bool {
    s.chars().any(|c| !c.is_whitespace() && c != '\u{200D}' && !is_hidden_char(c) && !is_combining_mark(c))
}

/// Nom d'affichage : sans caractères cachés (`strip_hidden`), sans espaces aux bords, au plus `MAX_NAME`
/// caractères, `player` si rien ne reste. Le serveur et le client l'appliquent : même nom partout.
pub fn clean_name(raw: &str) -> String {
    let filtered = strip_hidden(raw);
    let name: String = filtered.trim().chars().take(MAX_NAME).collect();
    // Le `take` peut laisser un espace en fin de nom (ex. "abc def" coupé après l'espace).
    let name = name.trim_end();
    if has_visible_char(name) { name.to_string() } else { "player".to_string() }
}

/// Identifier of anything that moves (players and animals share the id space).
pub type EntityId = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// First message on a connection.
    Hello {
        version: u32,
        name: String,
    },
    /// Step one tile (or just turn, if blocked).
    Move(Dir),
    /// Pet / talk to whatever stands on the adjacent tile.
    Interact(Dir),
    /// Build the given tile on the adjacent tile (refused if it has no build cost).
    Build(Dir, Tile),
    /// Break the adjacent tile.
    Break(Dir),
    Chat(String),
    /// Keepalive sent every few seconds: lets the server drop dead connections (read timeout).
    Ping,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome {
        id: EntityId,
        name: String,
        seed: u64,
    },
    /// Full content of a chunk the player is near.
    Chunk(ChunkData),
    TileChanged {
        pos: Pos,
        tile: Tile,
    },
    /// Every entity close to the player, sent each tick.
    Entities(Vec<EntityView>),
    Log(LogLine),
    Effect {
        pos: Pos,
        effect: Effect,
    },
    /// Everyone connected, as `(name, color)`. Sent on every join and leave.
    Online(Vec<(String, u8)>),
    /// Connection refused (e.g. protocol mismatch). The server closes the socket after it.
    Rejected(String),
    /// Inventaire complet du joueur, envoyé au Welcome et à chaque changement.
    Inventory(Vec<(Item, u16)>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkData {
    pub pos: ChunkPos,
    /// `CHUNK_AREA` tiles, row-major.
    pub tiles: Vec<Tile>,
    /// `CHUNK_AREA` biomes, row-major.
    pub biomes: Vec<Biome>,
}

impl ChunkData {
    pub fn is_valid(&self) -> bool {
        self.tiles.len() == CHUNK_AREA && self.biomes.len() == CHUNK_AREA
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityView {
    pub id: EntityId,
    pub pos: Pos,
    pub facing: Dir,
    pub look: Look,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Look {
    Player { name: String, color: u8 },
    Animal { species: Species, tamed_by: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    pub kind: LogKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LogKind {
    /// Server notices (joins, leaves); the client also uses it for its own connection notices.
    System,
    /// Replies to actions and chat commands, usually to the player alone (`/me` is broadcast).
    Info,
    Chat {
        from: String,
    },
    /// Animal sounds and reactions.
    Animal,
}

impl LogLine {
    pub fn new(kind: LogKind, text: impl Into<String>) -> Self {
        Self { kind, text: text.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  bob "), "bob");
        assert_eq!(clean_name("a "), "a");
        assert_eq!(clean_name(" "), "player");
        assert_eq!(clean_name("abcdefghijklmno pqrst"), "abcdefghijklmno");
        assert_eq!(clean_name(&"é".repeat(40)).chars().count(), MAX_NAME);
    }

    #[test]
    fn hidden_chars_are_dropped_but_zwj_kept() {
        assert_eq!(clean_name("\u{200B}"), "player");
        assert_eq!(clean_name("\u{202E}bob\u{FEFF}"), "bob");
        assert_eq!(clean_name("a\u{200D}b"), "a\u{200D}b");
    }

    #[test]
    fn zwj_only_name_falls_back_to_default() {
        assert_eq!(clean_name("\u{200D}"), "player");
        assert_eq!(clean_name(" \u{200D} \u{200D}"), "player");
        assert!(has_visible_char("a\u{200D}"));
        assert!(!has_visible_char("\u{200D} "));
    }

    #[test]
    fn invisible_ranges_are_hidden() {
        let ranges = [
            ('\u{2028}', '\u{2029}'),
            ('\u{061C}', '\u{061C}'),
            ('\u{2065}', '\u{2065}'),
            ('\u{206A}', '\u{206F}'),
            ('\u{00AD}', '\u{00AD}'),
            ('\u{180B}', '\u{180F}'),
            ('\u{FFF0}', '\u{FFFB}'),
            ('\u{E0000}', '\u{E0FFF}'),
            ('\u{3164}', '\u{3164}'),
            ('\u{115F}', '\u{1160}'),
            ('\u{2800}', '\u{2800}'),
            ('\u{FFA0}', '\u{FFA0}'),
            ('\u{034F}', '\u{034F}'),
            ('\u{17B4}', '\u{17B5}'),
            ('\u{FE00}', '\u{FE0F}'),
            ('\u{1BCA0}', '\u{1BCA3}'),
            ('\u{1D173}', '\u{1D17A}'),
        ];
        for (lo, hi) in ranges {
            for c in lo..=hi {
                assert!(is_hidden_char(c), "U+{:04X} doit être caché", c as u32);
                assert_eq!(clean_name(&c.to_string()), "player");
                assert!(!has_visible_char(&c.to_string()));
            }
        }
        assert!(!is_hidden_char('a') && !is_hidden_char('\u{200D}'));
    }

    #[test]
    fn combining_marks_alone_are_not_visible() {
        assert_eq!(clean_name("\u{0301}\u{0301}"), "player");
        assert_eq!(clean_name("e\u{0301}"), "e\u{0301}");
    }

    #[test]
    fn variation_selectors_kept_only_after_visible_char() {
        assert_eq!(clean_name("\u{2764}\u{FE0F}"), "\u{2764}\u{FE0F}");
        assert_eq!(strip_hidden("a\u{E0100}"), "a\u{E0100}");
        assert_eq!(clean_name("\u{FE0F}bob"), "bob");
        assert_eq!(clean_name("a \u{FE0F}b"), "a b");
        assert_eq!(strip_hidden("\u{2764}\u{FE0F}\u{FE0F}"), "\u{2764}\u{FE0F}");
        assert_eq!(strip_hidden("\u{200B}\u{FE0F}"), "");
    }

    #[test]
    fn zwj_emoji_sequence_is_kept() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(clean_name(family), family);
        assert_eq!(strip_hidden(family), family);
    }
}
