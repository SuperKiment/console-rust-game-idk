6
| Lignes | Section |
|---|---|
| 22-28 | Glyphes |
| 29-38 | Exemples |
| 39-41 | Tests de non-régression |
# Ajouter du contenu (tuiles, biomes, animaux, effets, objets)

Tout le contenu est déclaré dans des tables de `crates/shared/src/content/`, via la macro `content_table!` (`content/mod.rs`). Une ligne de table génère à la fois la variante d'enum (identifiant réseau et de sauvegarde) et sa définition : impossible de les désynchroniser.

> [!WARNING]
> L'ordre des lignes (index de variante, encodé par postcard) est le format réseau et de sauvegarde. **Ajoute toujours les lignes à la fin** d'une table. Réordonner ou supprimer une ligne casse les sauvegardes existantes (voir CLAUDE.md, convention de migration).

| Table | Fichier | Ce qu'une ligne définit |
|---|---|---|
| `Tile` / `TILES` | `content/tiles.rs` | nom, glyphe (ASCII / Unicode / large), couleur, fond optionnel (`bg`, sinon fond du biome), praticable, ce qu'elle devient une fois cassée, objet donné quand on la casse (`drops`), coût de construction (`cost`, liste vide = non constructible), raccord en traits de boîte |
| `Biome` / `BIOMES` | `content/biomes.rs` | couleur de sol, bande d'altitude, climat préféré (température, humidité), palette pondérée de tuiles, apparitions d'animaux par chunk |
| `Species` / `SPECIES` | `content/species.rs` | nom, glyphe, couleur, paramètres d'IA (`Ai`), sons, réactions aux caresses |
| `Effect` / `EFFECTS` | `content/effects.rs` | glyphe, couleur et durée d'un effet visuel éphémère |
| `Item` / `ITEMS` | `content/items.rs` | nom, glyphe, couleur d'un objet |

## Glyphes

`glyph(ascii, unicode, wide)` : le client choisit le jeu selon l'assistant de configuration.
- `ascii` : 7 bits, toujours lisible.
- `unicode` : un seul caractère d'une cellule.
- `wide` : exactement deux cellules (un emoji, ou deux caractères étroits). Test `wide_glyphs_are_two_cells_or_one_emoji`.

## Exemples

Nouvel animal : une ligne dans `SPECIES` (comportement composé avec les champs de `Ai` : `flee_radius`, `curious_radius`, `tameable`…), puis l'ajouter aux `spawns` d'un ou plusieurs biomes.

Nouveau biome : une ligne dans `BIOMES`. Les biomes terrestres partagent la bande `LAND` ; dans une même bande, le biome au climat le plus proche gagne (`Biome::select`). Les bandes d'altitude doivent couvrir tout l'intervalle (test `elevation_bands_cover_range`).

Nouvel objet : une ligne dans `ITEMS` (nom unique, glyphe, couleur), à la fin de la table. Pour qu'une tuile le donne quand on la casse, mettre `drops: Some(Item::X)` sur sa ligne de `TILES` : elle doit aussi avoir `breaks_into: Some(_)` (test `dropping_tiles_are_breakable`). Le serveur donne l'objet au joueur qui casse la tuile (`Game::break_tile`, `add_item`) et lui envoie son inventaire (`ServerMsg::Inventory`).

Tuile constructible : `cost: &[(Item::X, n)]` sur sa ligne de `TILES` (vide par défaut = non constructible). Elle doit aussi avoir `breaks_into: Some(_)` (test `buildable_tiles_are_breakable`). `Game::build` retire le coût du sac (`remove_items`, tout ou rien) avant de poser la tuile demandée par `ClientMsg::Build(dir, tile)` ; une tuile sans coût est refusée.

## Tests de non-régression

Si tu changes le générateur, le bruit ou le RNG, le test golden (`golden_generation`, `crates/server/src/world/generator.rs`) casse : le mettre à jour seulement si le changement des mondes est voulu. Les tables sont aussi vérifiées par `species_rows_are_playable` et `effects_last_and_tiles_break_into_walkable` (`content/mod.rs`).
