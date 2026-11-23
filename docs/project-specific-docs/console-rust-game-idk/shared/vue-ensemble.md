# shared

> Dépôt `console-rust-game-idk`, chemin `crates/shared/` (paquet `idk-shared`). Vérifié le 2026-11-22 depuis `crates/shared/Cargo.toml` et `crates/shared/src/`.

## À quoi ça sert

Code commun au serveur et au client, pour qu'ils ne puissent pas diverger :
- `src/geo.rs` : `Pos`, `ChunkPos`, `Dir`, `CHUNK_SIZE = 32`, conversions position/chunk/index local.
- `src/content/` : tables de contenu (`Tile`, `Biome`, `Species`, `Effect`) générées par la macro `content_table!`, plus `Rgb`, `Glyph`, `PLAYER_GLYPH`, `PLAYER_COLORS`.
- `src/protocol.rs` : `ClientMsg`, `ServerMsg`, `PROTOCOL_VERSION`, `DEFAULT_PORT`, `VIEW_CHUNKS`, `KEEP_CHUNKS`, `MAX_NAME` (16), `MAX_CHAT` (240), `clean_name` (nettoyage du nom, partagé serveur et client).
- `src/net.rs` : `split()`, `Reader::recv`, `Writer::send` (longueur + postcard).

## Techno

Bibliothèque Rust (`Cargo.toml` de `crates/shared`). Dépendances : `serde`, `postcard`, `tokio` (feature `net`), `tokio-util` (codec), `bytes`, `futures`.

## Dépendances

Ne dépend d'aucun autre crate du workspace. Utilisé par `idk-server` et `idk-client`.

## Place dans l'ensemble

Source de vérité du contenu (une ligne de table = variante + définition) et du format réseau. Le serveur lit les champs de gameplay (`walkable`, `breaks_into`, `Ai`, `spawns`), le client les champs d'affichage (`glyph`, `fg`, `bg`).

## Tests

Tests unitaires dans `src/net.rs` (aller-retour TCP), `src/content/mod.rs` (glyphes larges, biomes valides), `src/content/biomes.rs` (`elevation_bands_cover_range`, `climate_picks_nearest`), `src/geo.rs` (géométrie), `src/protocol.rs` (`clean_name`). `cargo test -p idk-shared`.

## Docs liées

- [carto-protocole.md](../carto-protocole.md)
- [guide-ajout-contenu.md](../guide-ajout-contenu.md)
- [Carte du dépôt](../vue-ensemble.md)
