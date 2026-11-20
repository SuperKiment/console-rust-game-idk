# Ajouter du contenu (tuiles, biomes, animaux, effets)

Tout le contenu est déclaré dans des tables de `crates/shared/src/content/`, via la macro `content_table!` (`content/mod.rs`). Une ligne de table génère à la fois la variante d'enum (identifiant réseau et de sauvegarde) et sa définition : impossible de les désynchroniser.

> [!WARNING]
> L'ordre des lignes (index de variante, encodé par postcard) est le format réseau et de sauvegarde. **Ajoute toujours les lignes à la fin** d'une table. Réordonner ou supprimer une ligne casse les sauvegardes existantes (voir CLAUDE.md, convention de migration).

| Table | Fichier | Ce qu'une ligne définit |
|---|---|---|
| `Tile` / `TILES` | `content/tiles.rs` | nom, glyphe (ASCII / Unicode / large), couleurs, praticable, ce qu'elle devient une fois cassée, raccord en traits de boîte |
| `Biome` / `BIOMES` | `content/biomes.rs` | bande d'altitude, climat préféré (température, humidité), palette pondérée de tuiles, apparitions d'animaux par chunk |
| `Species` / `SPECIES` | `content/species.rs` | nom, glyphe, couleur, paramètres d'IA (`Ai`), sons, réactions aux caresses |
| `Effect` / `EFFECTS` | `content/effects.rs` | glyphe et durée d'un effet visuel éphémère |

## Glyphes

`glyph(ascii, unicode, wide)` : le client choisit le jeu selon l'assistant de configuration.
- `ascii` : 7 bits, toujours lisible.
- `unicode` : un seul caractère d'une cellule.
- `wide` : exactement deux cellules (un emoji, ou deux caractères étroits). Test `wide_glyphs_are_two_cells_or_one_emoji`.

## Exemples

Nouvel animal : une ligne dans `SPECIES` (comportement composé avec les champs de `Ai` : `flee_radius`, `curious_radius`, `tameable`…), puis l'ajouter aux `spawns` d'un ou plusieurs biomes.

Nouveau biome : une ligne dans `BIOMES`. Les biomes terrestres partagent la bande `LAND` ; dans une même bande, le biome au climat le plus proche gagne (`Biome::select`). Les bandes d'altitude doivent couvrir tout l'intervalle (test `elevation_bands_cover_range`).
