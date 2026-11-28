# Priorité
En cas de conflit entre ce fichier et un skill (superpowers, etc.), ce fichier prime.

# Projet
**console-rust-game-idk** : jeu multijoueur temps réel en Rust, architecture client-serveur (serveur faisant autorité, confiance totale, joué entre amis en LAN). Client terminal (ratatui/crossterm), graphismes ASCII style roguelike, monde ouvert infini procédural, animaux, mode incognito. Serveur et client tournent en docker.

# Ressources à disposition
## Dossier des docs (`docs`, relatif à la racine du projet)
INDEX.md : à lire systématiquement en début de session. Contient les informations de où trouver certaines informations et conventions.

Conventions de nommage, d'arborescence et de liens des docs (obligatoires, toute la doc projet vit dans `project-specific-docs/`) :
@conventions-docs.md

## Suivi des tâches : issues et PR GitHub (pas de `current-task/`)
Le dépôt n'utilise ni `current-task/` ni `old-tasks/`. Correspondance avec la méthode du kit :

| Fichier du kit | Remplacé par |
|---|---|
| GOAL.md (tâche, scope, but) | corps de l'issue (gabarit `.github/ISSUE_TEMPLATE/task.md`) |
| PLAN.md (étapes d'une grande tâche) | checklist `- [ ]` dans le corps de l'issue, cochée au fil de l'eau |
| HANDOFF.md (où reprendre) | dernier commentaire de l'issue ou de la PR ouverte (section « Reprendre ici ») |
| JOURNAL.md (fait, décisions) | descriptions des PR mergées (sections « Fait », « Décisions », « À vérifier ») |
| DOC_CURRENT_TASK.md (pièges, décisions bancales) | section « Pièges » de la PR ; si durable : `project-specific-docs/` (`domaine-*`, `conventions-*`) |

Début de session : lire INDEX.md, puis les issues ouvertes et la PR ouverte éventuelle (outils GitHub MCP). Un travail interrompu laisse un commentaire « Reprendre ici » sur l'issue.

## Conventions du projet
Les conventions de code propres à console-rust-game-idk vivent dans `project-specific-docs/` (préfixe `conventions-`). Elles sont listées dans INDEX.md. Lis celles qui concernent le code que tu touches avant de l'écrire. Aucune n'existe encore : à créer quand une règle de code durable émerge.

## Build, test, lint
Le code est dans `..` (chemin relatif au dossier de docs ; `.` = même dossier).

Workspace cargo à la racine du dépôt (`crates/shared`, `crates/server`, `crates/client`). Toutes les commandes se lancent depuis la racine du dépôt.

| Action | Commande |
|---|---|
| Build | `cargo build --workspace` |
| Tests | `cargo test --workspace` |
| Lint | `cargo clippy --workspace --all-targets -- -D warnings` |
| Format | `cargo fmt --all` (CI : `cargo fmt --all -- --check`) |
| Serveur | `cargo run -p idk-server -- --port 7777 --world ./world` |
| Client | `cargo run -p idk-client -- --server 127.0.0.1:7777` |
| Docker | `docker compose build` puis `docker compose run --rm client` (démarre aussi le serveur) |

Pièges d'environnement :
- Session cloud : le démon docker n'est pas lancé par défaut. `dockerd > /tmp/dockerd.log 2>&1 &` puis attendre `docker info`.
- Le client est une TUI : pour le tester sans terminal interactif, le lancer dans `tmux` (`tmux new -d -s t -x 120 -y 40 '<cmd>'`, `tmux send-keys -t t ...`, `tmux capture-pane -p -t t`).

## Git, branches et migrations
- Commits : Claude commite et pousse en autonomie sur la branche de l'issue en cours. Messages au format Conventional Commits (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`), référence de l'issue dans le corps (`Refs #N`). Une PR par issue vers `main` (corps `Closes #N`, gabarit `.github/pull_request_template.md`), mergée par Claude (squash) une fois build, tests, clippy et fmt verts et le jeu testé.
- Attribution : trailers `Co-Authored-By` et `Claude-Session` dans les commits, pied de page Claude Code dans les PR et commentaires GitHub. Aucune mention dans le code.
- Branches : `<type>/<n°issue>-<slug>` depuis `main` à jour (ex. `feat/3-server-world`, `chore/1-bootstrap`). Une branche par issue.
- Migrations de schéma ou de données : format de sauvegarde du monde : `<world>/world.bin` (postcard), champ `version` de `SaveData` (`crates/server/src/save.rs`, `SAVE_VERSION`) ; tout changement incompatible incrémente la version et ajoute une conversion au chargement. Mécanique : `save::decode` lit `version` seule en tête (premier champ de `SaveData`, doit le rester), puis `match` : un bras par version, une version future ou inconnue est refusée (« unsupported save version N »). Version courante `SAVE_VERSION` = 4, modules `v1`, `v2` et `v3` figés (versions 1, 2 et 3) ; procédure « ajouter une version » dans la carto serveur, section « Sauvegarde ».
- Code temporaire : marqueur `TODO TEMP <réf>` (réf = numéro de ticket ou nom de tâche) pour le retrouver et le retirer avant commit.

## Langue
- Docs `.md`, échanges et commentaires de code : français.
- Code (noms de classes, méthodes, variables, etc.) : anglais.

# Ce qu'il ne faut pas faire

- Réinventer la roue : réutilise un composant existant.
- Inventer des situations ou informations. Pose la question, je suis là. Mieux que brûler 150k tokens sans la bonne info. Je suis la source de vérité quand une information est complexe à trouver.
- **Trop de temps** = 2 recherches sans résultat sur la même information : pose la question directement.
- A part dit contraire, pas besoin de over-expliquer ce qui n'allait pas, tant que c'est réparé c'est OK.
- Pas d'over-engineering. On essaie de faire le plus clair possible, le plus straightforward possible. Critère : code direct tant qu'il y a moins de 3 cas ; à partir de 3 cas, array/enum/liste extensible sans toucher au code hardcodé.
- Pas de people-pleasing. Si je dis n'importe-quoi, dis-le moi et explique pourquoi je dis de la merde.

# Obligations

## Décisions
Mode autonome (demandé par l'utilisateur) : tu tranches toi-même les questions à enjeu et tu les consignes dans la section « Décisions » de la PR (circonstances, possibilités, choix et pourquoi). Tu ne poses une question que si l'information manque et ne peut pas être déduite du dépôt ou de l'environnement.
**Question à enjeu** = au moins un de : migration de schéma ou de données, nouvelle dépendance, suppression de code existant, choix entre approches d'architecture, plus de 100 lignes modifiées.

## Workflow
- **Grande tâche** = plus de 3 fichiers modifiés, ou plusieurs crates (`shared`, `server`, `client`).
- Grande tâche : plan en checklist dans l'issue, puis étape par étape. Pas de validation utilisateur entre les étapes : tu testes toi-même (build, tests, clippy, et le jeu lancé dans tmux pour le client). Doc mise à jour à chaque étape.
- Une issue = une branche = une PR. La PR n'est mergée qu'après vérifications vertes.

## Orchestrateur
- Tu es orchestrateur : récupérer l'info résumée de ce qui a été fait, créer des tâches faciles, assigner.
- Tu codes toi-même seulement si la modif tient dans ce seuil : une issue entière tant qu'elle reste cohérente (le contexte du code est déjà chargé) ; délégation à un agent pour les recherches et les tâches isolables volumineuses (ex. ajout massif de contenu). Au-delà, délègue à un agent.
- Avant de me présenter une étape, vérifie toi-même : build, tests et lint (voir « Build, test, lint »), et un diff cohérent avec le résumé de l'agent (`git diff --stat`). Un résumé d'agent peut être faux. Si le build n'est pas lançable, dis-le explicitement au lieu d'affirmer que ça compile.

## Description de PR
À chaque tâche finie : sections du gabarit `.github/pull_request_template.md` remplies (fait, décisions, pièges, vérifications lancées et résultat, quoi vérifier et où).

## Skills
Si je ne l'ai pas spécifié, utilise /caveman (`anthropic-skills:caveman`) et /using-superpowers.

## Rédaction de documentation
Doc de recherche de données et d'architecture (en général en préparation d'une nouvelle tâche) : utilise le skill `visual-docs` (diagrammes, alertes, imports Markdown Preview Enhanced). Mets des liens vers les fichiers qui ne bougeront pas, sinon des chemins relatifs.

# Index des documents .md

## Périmètre
- **Indexés** : les `.md` de docs de ce dossier (racine, `project-specific-docs/`) de 80 lignes ou plus, ou qui ont déjà un index.
- **Jamais indexés** : `CLAUDE.md`, `INDEX.md`, `conventions-docs.md` (importé dans CLAUDE.md), `README.md`, tout `.md` du dépôt de code, et tout `.md` de moins de 80 lignes sans index.
- Ne lance pas `bash docs/reindex.sh` sur autre chose : il modifie le fichier (entier + tableau en tête, casse le rendu GitHub/IDE).

## Écriture
Ne calcule jamais l'index à la main. Un hook PostToolUse (`.claude/hooks/reindex-md.sh`) réindexe automatiquement après chaque Edit/Write d'un `.md` du périmètre. Il recalcule toutes les plages : après une modification, relis le fichier avant de le rééditer.
Le hook ne couvre pas les modifs par `Bash` (sed, etc.) ni les éditions manuelles dans l'IDE. Dans ces cas : `bash docs/reindex.sh chemin/fichier.md` (commande lancée depuis la racine du projet, chemin du fichier relatif à cette racine).

## Lecture
Un seul outil, `docs/mdx` (commande lancée depuis la racine du projet, chemin du fichier relatif à cette racine) :
- `docs/mdx fichier.md` : affiche l'index.
- `docs/mdx fichier.md A B` : affiche les lignes A à B (plage de l'index).

Ne lis jamais un document entier s'il a un index. Si `docs/mdx fichier.md` répond "Pas d'index" et que le fichier est dans le périmètre : `bash docs/reindex.sh fichier.md`. Sinon, lis-le directement.

## Format
```md
5
| Lignes | Section |
|---|---|
| 6-20 | 1. Première section |
| 21-35 | — Sous-section |
```
Ligne 1 = dernière ligne de l'index (ligne 1 incluse). Chaque ligne du tableau = plage de la section.

# Utilisation des agents
- **Implémentation** : implémentation : agents `sonnet` en effort medium, high si la tâche est complexe ; recherche : `sonnet` effort medium. Tâches bien scopées et définies : l'orchestrateur crée des tâches simples et rapides, récupère le résumé, donne la tâche suivante.
- **Recherche** (usage d'une fonction, implémentation de référence) : pas besoin du plus gros modèle pour chercher, prends le modèle de recherche défini ci-dessus.
- Les agents n'héritent pas des skills ni de ce fichier : écris dans chaque prompt de délégation les skills à utiliser (/caveman (`anthropic-skills:caveman`) et /using-superpowers), « lis les `.md` avec `docs/mdx` » et les chemins utiles (conventions, commandes de build si besoin).
- Les agents n'écrivent ni sur les issues ni sur les PR, et ne commitent pas. Ils renvoient un résumé détaillé : fichiers modifiés, build/lint lancés et résultat, points d'attention.
- Les agents de recherche ne renvoient que les informations précises demandées (fichier:ligne, signature), pas de résumé de travail.
