# client

> Dépôt `console-rust-game-idk`, chemin `crates/client/` (paquet `idk-client`). Vérifié le 2026-11-20 depuis `crates/client/Cargo.toml` et `src/`.

## À quoi ça sert

Client terminal : se connecte au serveur, affiche la carte, le journal et les panneaux, envoie les actions du joueur. Assistant de configuration au premier lancement (`--configure` pour le relancer), mode incognito (fausse interface d'éditeur).

## Techno

Binaire (`src/main.rs`, `clap`). `ratatui` 0.30, `crossterm` (feature `event-stream`), `tokio`, `toml` + `serde` pour la config. Options : `--server`, `--name`, `--incognito`, `--configure` (env `IDK_SERVER`, `IDK_NAME`).

## Dépendances

- `idk-shared` (protocole, contenu).
- Serveur de jeu via TCP.
- Fichier de config TOML : `$IDK_CONFIG`, sinon `$XDG_CONFIG_HOME/idk-game/client.toml`, sinon `~/.config/idk-game/client.toml`.

## Place dans l'ensemble

Un client par joueur. Image docker cible `client` (volume `/config`, `TERM=xterm-256color`, `COLORTERM=truecolor`).

## Tests

Tests unitaires : `src/input.rs`, `src/config.rs`, `src/setup.rs` (machine d'états de l'assistant), `src/ui/glyphs.rs`, `src/ui/incognito.rs`. Pas de test d'intégration (`crates/client/tests` absent). L'interface se teste à la main dans `tmux` (voir `docs/CLAUDE.md`).

## Docs liées

- [carto-architecture.md](carto-architecture.md)
- [carto-protocole.md](../carto-protocole.md)
- [guide-ajout-contenu.md](../guide-ajout-contenu.md)
