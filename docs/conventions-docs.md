# Conventions de documentation

Règles obligatoires pour créer, nommer, ranger et lier les docs de ce dossier. Elles s'appliquent à Claude et aux agents. Elles ne s'appliquent pas aux `.md` du code (dans `..`, chemin relatif à ce dossier de docs).

## Arborescence

```
docs/
├─ CLAUDE.md, INDEX.md, conventions-docs.md   règles Claude/agents, sommaire, cette convention
├─ mdx, reindex.sh                            outils de lecture et d'indexation
├─ templates/                                 gabarits de prompt et de fiche projet
└─ project-specific-docs/                     toute la doc propre au projet
   ├─ guide-*.md                              docs communes à tous les dépôts (build, lancement)
   └─ <Dépôt>/                                un dossier par dépôt (ou par projet de 1er niveau)
      ├─ vue-ensemble.md                      carte des projets du dépôt
      ├─ conventions-*.md, domaine-*.md       docs transverses au dépôt
      └─ <Projet>/                            un dossier par projet de 1er niveau du dépôt
         ├─ vue-ensemble.md                   fiche du projet (obligatoire)
         └─ carto-*.md, domaine-*.md, guide-*.md
```

<!-- `docs` est le dossier des docs, relatif à la racine du projet (`.` = la racine elle-même). Le schéma interne est identique que les docs soient à la racine ou dans un sous-dossier (ex. `docs`), et qu'elles soient versionnées avec le code ou non. -->

- Une doc propre à un seul projet va dans le dossier de ce projet. Une doc qui couvre plusieurs projets d'un même dépôt va à la racine du dépôt. Une doc commune à tous les dépôts va à la racine de `project-specific-docs/`.
- S'il n'y a qu'un seul dépôt, `project-specific-docs/<Dépôt>/` reste utile : il garde la structure prête si un deuxième dépôt arrive.
- Sous-projets et projets de tests : documentés dans le dossier du projet parent de 1er niveau.
- Dossiers sans code (pipelines, collections de requêtes, binaires legacy…) : une ligne dans le `vue-ensemble.md` du dépôt, pas de dossier.

## Nommage

- Fichiers : minuscules, ASCII sans accent, mots séparés par des tirets (kebab-case), extension `.md`. Langue des mots : français.
- Dossiers projet : nom exact du dossier dans le dépôt, casse comprise.
- Préfixe obligatoire selon le type de doc :

| Préfixe / nom | Contenu | Exemple |
|---|---|---|
| `vue-ensemble.md` | Point d'entrée d'un dossier : fiche projet ou carte du dépôt | `<Dépôt>/<Projet>/vue-ensemble.md` |
| `carto-<sujet>.md` | Structure d'un projet : dossiers, couches, qui appelle quoi | `<Dépôt>/<Projet>/carto-structure.md` |
| `conventions-<sujet>.md` | Règles de code à appliquer | `<Dépôt>/conventions-nommage.md` |
| `domaine-<sujet>.md` | Flux métier, souvent transverse à plusieurs projets | `<Dépôt>/domaine-paiements.md` |
| `guide-<sujet>.md` | Procédure, commandes, mode d'emploi | `project-specific-docs/guide-build-run.md` |

- Exceptions au nommage : `CLAUDE.md`, `INDEX.md`.
- Une analyse produite pendant une tâche va dans l'issue ou la PR. Si elle reste utile après la tâche, elle devient une doc de `project-specific-docs/` avec un nom conforme.

## Liens et références

- Liens relatifs Markdown vers les autres docs : `[carto du projet](../Projet/carto-structure.md)`. Pas de chemin absolu.
- Pour citer une section, nomme-la après le lien : `[conventions](conventions-nommage.md), section « Nommage des classes »`. Pas de numéro de ligne (les index les recalculent).
- Code du dépôt : chemin depuis la racine du dépôt, en code (par exemple `src/Main.ext`).
- `INDEX.md` liste toutes les docs, sauf les fiches `vue-ensemble.md` de projet : elles sont listées par la carte du dépôt (`<Dépôt>/vue-ensemble.md`).
- Toute nouvelle doc, tout renommage ou tout déplacement met à jour `INDEX.md` (ou la carte du dépôt pour une fiche) et les liens qui pointent vers elle (`grep -rn "<ancien-nom>" --include=*.md`).

## Gabarit d'une fiche `vue-ensemble.md` de projet

Gabarit complet : `templates/vue-ensemble-projet.md` (dossier `templates/` des docs). Contenu attendu :

```md
# <Projet>

> Dépôt `<Dépôt>`, chemin `<Dépôt>/<Projet>/`. Vérifié le <AAAA-MM-JJ> depuis <sources lues>.

## À quoi ça sert
## Techno
Type (web, API, lib, service, console, tests…), framework cible, fichier de build.
## Dépendances
Références entre projets, qui l'appelle, ce qu'il appelle (base de données, API, services).
## Place dans l'ensemble
## Tests
## Docs liées
```

- Écris seulement ce qui est vérifié dans le code. Marque une déduction par « (non vérifié) ». Une section sans info : « Rien trouvé ».
- Rédaction des docs d'architecture : skill `visual-docs` (diagrammes mermaid, alertes).
