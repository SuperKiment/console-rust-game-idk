# server

> Dépôt `console-rust-game-idk`, chemin `crates/server/` (paquet `idk-server`). Vérifié le 2026-11-22 depuis `crates/server/Cargo.toml`, `src/`, `tests/network.rs`, `examples/worldmap.rs`.

## À quoi ça sert

Serveur de jeu faisant autorité : accepte les connexions TCP, simule le monde (joueurs, animaux, tuiles), génère le terrain par chunks à la demande, diffuse l'état aux clients, sauvegarde le monde.

## Techno

Lib (`src/lib.rs`, fonction `run`) et binaire (`src/main.rs`, `clap`). Runtime `tokio`, journalisation `tracing`. Options : `--bind`, `--port`, `--world`, `--seed` (env `IDK_BIND`, `IDK_PORT`, `IDK_WORLD`, `IDK_SEED`).

## Dépendances

- `idk-shared` (protocole, contenu, géométrie).
- Système de fichiers : `<world>/world.bin`.
- Aucune base de données ni service externe.

## Place dans l'ensemble

Un seul serveur, un monde. Image docker cible `server` (volume `/data`, port 7777).

## Tests

- Unitaires : `src/ai.rs`, `src/game.rs`, `src/save.rs`, `src/world/mod.rs`, `src/world/generator.rs` (dont `golden_generation` : empreinte FNV-1a des tuiles et biomes + apparitions de 3 chunks pour une seed fixe), `src/rng.rs`, `src/world/noise.rs`. Si tu changes le générateur, le bruit ou le RNG, le test golden casse : le mettre à jour seulement si le changement des mondes est voulu.
- Intégration : `tests/network.rs` (vrai serveur sur port aléatoire, client brut : connexion, construction, chat, persistance au redémarrage, reprise du même nom, handshake invalide).
- Exemple : `cargo run -p idk-server --example worldmap -- <seed> [step]` affiche une vue d'ensemble des biomes.

## Docs liées

- [carto-architecture.md](carto-architecture.md)
- [carto-protocole.md](../carto-protocole.md)
- [guide-ajout-contenu.md](../guide-ajout-contenu.md)
