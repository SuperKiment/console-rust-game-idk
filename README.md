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

L'image serveur a un healthcheck (connexion TCP locale) : `docker compose ps` indique `healthy` une fois le serveur prêt, et le client compose attend cet état pour démarrer.

Les joueurs se connectent (port 7777) avec, au choix :

```sh
cargo run --release -p idk-client -- --server <ip>:7777
docker compose run --rm --no-deps -e IDK_SERVER=<ip>:7777 client
```

`--no-deps` évite de démarrer un serveur local inutile sur la machine du joueur.

## Sans docker

```sh
cargo run --release -p idk-server -- --port 7777 --world ./world
cargo run --release -p idk-client -- --server 127.0.0.1:7777
```

Options serveur : `--bind`, `--port`, `--world`, `--seed`, `--backups` (env `IDK_BIND`, `IDK_PORT`, `IDK_WORLD`, `IDK_SEED`, `IDK_BACKUPS`). `--version` (serveur et client) affiche la version du paquet et celle du protocole.
Options client : `--server`, `--name`, `--incognito`, `--configure` (env `IDK_SERVER`, `IDK_NAME`). Les options de la ligne de commande ne valent que pour la session : elles ne sont pas écrites dans la config.

## Contrôles

| Touches | Action |
|---|---|
| flèches, `wasd`, `hjkl`, `zqsd` | se déplacer |
| `e`, espace | interagir, caresser |
| `b` | construire la tuile choisie (mur, porte… : voir `[`, `]`) |
| `[`, `]` | choisir la tuile à construire (précédente, suivante, avec bouclage) ; la tuile et son coût s'affichent dans le panneau `you` (`build wall (1 stone)`) |
| `x` | casser (arbre, pin, roche, cactus, mur, porte, fleurs : rapporte bois, pierre, fibre ou fleur) |
| `t`, entrée | chat (entrée envoie, `esc` annule, `↑`/`↓` historique des messages envoyés) |
| `PgUp`, `PgDn`, `fin` | remonter, redescendre le journal, revenir en bas |
| `F1` | aide : toutes les touches et la légende des glyphes (`F1` ou `esc` referme, `↑`/`↓`/`PgUp`/`PgDn` défilent ; sans effet en incognito) |
| `F2` | basculer le mode incognito |
| `esc` deux fois de suite (entre 0,15 et 2 s d'écart, toute autre touche annule ; esc maintenu ne quitte pas), `ctrl-c` | quitter (`ctrl-c` directement) |

## Commandes de chat

Un message qui commence par `/` est une commande (réponse visible par toi seul, sauf `/me`, `/msg` et `/tp`) :

| Commande | Effet |
|---|---|
| `/help` | liste les commandes |
| `/who` | joueurs en ligne, distance et direction (N, NE, E…) |
| `/me <action>` | action visible de tous : `* alice <action>` |
| `/msg <nom> <texte>` | message privé (le nom peut contenir des espaces) |
| `/tp <nom>` | te téléporte à côté du joueur (nom entier, casse ignorée ; refusé si aucune tuile libre autour) |
| `/release` | libère l'animal apprivoisé que tu regardes (le tien uniquement) ; caresser son animal bascule suit / reste |

## Config client

Au premier lancement, un assistant pose quelques questions (symboles Unicode et emoji, couleurs, nom, serveur, incognito) et écrit le fichier de config. `--configure` le relance à tout moment (`docker compose run --rm client --configure` en docker) ; Échap quitte sans rien sauvegarder.

Fichier TOML (nom, serveur, glyphes, couleurs, incognito) ; un fichier invalide est renommé en `client.toml.bak` et l'assistant se relance :
`$IDK_CONFIG`, sinon `$XDG_CONFIG_HOME/idk-game/client.toml`, sinon `~/.config/idk-game/client.toml`.
En docker : volume `client-config` monté dans `/config` ; `/etc/localtime` est monté pour que le journal et l'horloge incognito affichent l'heure locale.

## Mode incognito

Remplace l'interface par une fausse fenêtre d'éditeur de code (onglets, numéros de ligne, journal façon logs serveur, barre d'état), en niveaux de gris et en ASCII. La connexion et le jeu continuent normalement.
- `F2` : bascule à chaud (y compris en saisie de chat, le texte est conservé).
- `--incognito` : démarre dans ce mode (session seulement).
- Option `incognito` de la config (question de l'assistant) : mode de démarrage par défaut.

## Sauvegarde du monde

Le serveur sauvegarde toutes les 60 s et à l'arrêt (`ctrl-c` ou SIGTERM). En docker, le monde vit dans le volume nommé `world` (`/data`) ; `docker compose down` le garde, `docker compose down -v` le supprime.

Copies tournantes : `world.bin.1` (la plus récente) à `world.bin.N`, `N` = `--backups` (défaut 3, 0 = aucune). Rotation au démarrage (après lecture réussie) puis toutes les 10 autosaves (10 min), pas à l'arrêt. Restauration manuelle : serveur arrêté, copier `world.bin.N` sur `world.bin`. Si `world.bin` est illisible, le serveur refuse de démarrer et indique les copies.
