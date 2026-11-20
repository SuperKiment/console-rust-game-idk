# INDEX.md — Sommaire des fichiers de documentation de console-rust-game-idk

Règles de nommage et de rangement : [conventions-docs.md](conventions-docs.md) (importé dans CLAUDE.md).

Les docs de 80 lignes ou plus portent leur propre index en tête (ligne 1 = dernière ligne de l'index). Pour lire :
- `docs/mdx fichier.md` : affiche l'index du fichier (sections et plages de lignes).
- `docs/mdx fichier.md A B` : affiche les lignes A à B.

Ces commandes se lancent depuis la racine du projet.

Les plages ne sont volontairement pas recopiées ici : elles se périmaient. L'index de chaque fichier est régénéré automatiquement (hook `.claude/hooks/reindex-md.sh`, ou `bash docs/reindex.sh` à la main).

<!-- Une ligne par doc : lien + une phrase. Les fiches `vue-ensemble.md` de projet ne sont pas listées ici, mais par la carte du dépôt. Mettre à jour à chaque nouvelle doc, renommage ou déplacement. -->

## Racine (règles et outils)
- [conventions-docs.md](conventions-docs.md) — convention de nommage, d'arborescence et de liens des docs, gabarit de fiche projet.
- [templates/](templates/) — gabarits de prompt de début de tâche (`task-prompt.md`) et de fiche projet (`vue-ensemble-projet.md`). Les gabarits de tâche sont ceux d'issue et de PR dans `.github/`.
- `mdx` — lecture d'un `.md` indexé (commande : `docs/mdx`).
- `reindex.sh` — (re)génération de l'index d'un `.md` (commande : `bash docs/reindex.sh`).

## Suivi des tâches
- Issues et PR GitHub — voir CLAUDE.md, section « Suivi des tâches ». Gabarits : `.github/ISSUE_TEMPLATE/task.md`, `.github/pull_request_template.md`.

## Docs communes (`project-specific-docs/`)
<!-- guide-*.md communs à tous les dépôts. -->

## Dépôt / projet `<nom>`
<!-- Une section par dépôt ou projet de 1er niveau : vue-ensemble.md du dépôt, conventions-*.md, domaine-*.md, puis les projets. -->
