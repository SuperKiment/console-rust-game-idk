# console-rust-game-idk

Jeu multijoueur temps réel dans le terminal, en Rust. Monde ouvert infini généré procéduralement, animaux, mode incognito, rendu ASCII façon roguelike.
Serveur faisant autorité, client TUI (ratatui). Pensé pour être joué entre amis en LAN.

## Jouer en solo (docker)

```sh
docker compose run --rm client
```

Construit les images, démarre aussi le serveur, puis lance le client. Nom de joueur : `IDK_NAME=Bob docker compose run --rm client`.

## Héberger en LAN

Sur la machine hôte :

```sh
docker compose up -d server          # IDK_SEED=42 pour fixer la graine d'un nouveau monde
```

Les joueurs se connectent (port 7777) avec, au choix :

```sh
cargo run --release -p idk-client -- --server <ip>:7777
docker compose run --rm -e IDK_SERVER=<ip>:7777 client
```

## Sans docker

```sh
cargo run --release -p idk-server -- --port 7777 --world ./world
cargo run --release -p idk-client -- --server 127.0.0.1:7777
```

Options serveur : `--bind`, `--port`, `--world`, `--seed` (env `IDK_BIND`, `IDK_PORT`, `IDK_WORLD`, `IDK_SEED`).
Options client : `--server`, `--name`, `--incognito`, `--configure` (env `IDK_SERVER`, `IDK_NAME`). Les options de la ligne de commande ne valent que pour la session : elles ne sont pas écrites dans la config.

## Contrôles

| Touches | Action |
|---|---|
| flèches, `wasd`, `hjkl`, `zqsd` | se déplacer |
| `e`, espace | interagir, caresser |
| `b` | construire un mur |
| `x` | casser |
| `t`, entrée | chat (entrée envoie, `esc` annule) |
| `F2` | basculer le mode incognito |
| `esc`, `ctrl-c` | quitter |

## Config client

Au premier lancement, un assistant pose quelques questions (symboles Unicode et emoji, couleurs, nom, serveur, incognito) et écrit le fichier de config. `--configure` le relance à tout moment (`docker compose run --rm client --configure` en docker) ; Échap quitte sans rien sauvegarder.

Fichier TOML (nom, serveur, glyphes, couleurs, incognito) :
`$IDK_CONFIG`, sinon `$XDG_CONFIG_HOME/idk-game/client.toml`, sinon `~/.config/idk-game/client.toml`.
En docker : volume `client-config` monté dans `/config`.

## Mode incognito

Remplace l'interface par une fausse fenêtre d'éditeur de code (onglets, numéros de ligne, journal façon logs serveur, barre d'état), en niveaux de gris et en ASCII. La connexion et le jeu continuent normalement.
- `F2` : bascule à chaud (hors saisie du chat).
- `--incognito` : démarre dans ce mode (session seulement).
- Option `incognito` de la config (question de l'assistant) : mode de démarrage par défaut.

## Sauvegarde du monde

Le serveur sauvegarde toutes les 60 s et à l'arrêt (`ctrl-c` ou SIGTERM). En docker, le monde vit dans le volume nommé `world` (`/data`) ; `docker compose down` le garde, `docker compose down -v` le supprime.
