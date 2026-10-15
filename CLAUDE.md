# CLAUDE.md — PalaClips

> Manuel de travail de Claude Code sur ce repo. **Lis-le en entier au début de chaque session**,
> puis `docs/HANDOFF.md` (où on en est) et la phase en cours dans `docs/PLAN.md`.

PalaClips est une app Windows d'enregistrement et de **replay instantané** (alternative légère à
NVIDIA ShadowPlay), écrite en **Rust**, avec une **vraie séparation de l'audio par logiciel**, une
galerie de clips, et une UI **Tauri 2 + React**. Spécification complète : `docs/SPEC.md`.

---

## 0. Règle n°1 : la V1 est complète, on ne reporte rien

- La V1 (tag `v1.0.0`) contient **tout** ce qui est listé dans `docs/SPEC.md`. Aucune fonctionnalité
  de la SPEC ne peut être marquée « plus tard », « V2 », « TODO » ou désactivée pour livrer.
- Si une fonctionnalité de la SPEC s'avère impossible telle quelle, **tu t'arrêtes** et tu
  l'écris dans `docs/HANDOFF.md` § « Bloquants » avec les options. Palawi décide. Tu ne réduis
  jamais le périmètre de toi-même.
- Ce qui n'est pas dans la V1 est listé explicitement dans `docs/SPEC.md` § « Hors périmètre ».
  Ce sont des choix, pas des reports.
- `EngineError::NotImplemented` et les `todo!()` sont tolérés pendant le dev d'une phase, et
  **interdits** dans le code à la fin de la phase qui les concerne (vérifié par `/valider-phase`).

## 1. Protocole de session

1. Début : lance la commande `/reprendre` (`.claude/commands/reprendre.md`).
2. Travaille **une phase à la fois**, dans l'ordre de `docs/PLAN.md`. Une phase n'est terminée
   que quand **tous** ses critères d'acceptation sont vérifiés, avec un rapport dans
   `docs/reports/phase-N.md` (`/valider-phase`).
3. Fin de session (ou contexte qui se remplit) : lance `/handoff`. `docs/HANDOFF.md` doit
   permettre à une nouvelle session de reprendre **sans rien te demander**.
4. Une phase = une ou plusieurs branches conventionnelles parties de `dev` (§ 4 Git) = une PR
   par branche vers `dev` (modèle dans `.github/`), mergée par toi une fois la CI verte, puis
   branche supprimée. `main` ne reçoit que les versions publiées.
5. Les critères qui exigent la machine de Palawi (jeu, Resolve, test d'une heure...) : prépare
   tout (commande CLI, script, protocole de `docs/TESTING.md`), puis demande-lui de lancer le
   test et de te donner le résultat. Ne coche jamais un critère que personne n'a vérifié.

## 2. Front : skills de design obligatoires

Tout travail sur `app/src/` (écrans, composants, styles, animations) passe par :

- **`impeccable`** (plugin) : `/impeccable init` une seule fois (écrit `PRODUCT.md` et
  `DESIGN.md` à la racine), puis `/impeccable shape` avant chaque écran, `/impeccable critique`
  et `/impeccable audit` après, `/impeccable polish` avant de clore la phase.
- **`emil-design-eng`** (skill) : pour toute animation, transition, micro-interaction, état de
  survol/pression, et le ressenti général des composants.
- Le brief produit à donner à `/impeccable init` est dans `docs/DESIGN-BRIEF.md`.
- Si l'un des deux n'est pas installé, **arrête-toi** et donne à Palawi la commande
  d'installation (voir `README.md` § Prérequis). Pas de front « au feeling ».
- Commande raccourci : `/design <écran>` (`.claude/commands/design.md`).

## 3. Règles techniques non négociables

Le détail et les sources sont dans `docs/ARCHITECTURE.md` et `docs/RESEARCH.md`.

1. **Horloge unique QPC (unités de 100 ns).** Vidéo : `Frame::timestamp`
   (`SystemRelativeTime`). Audio : `pu64QPCPosition` de `GetBuffer`. Les PTS ne sont **jamais**
   calculés en additionnant des durées théoriques. Type : `palaclips_engine::clock::Hns`.
2. **Vidéo en CFR** au fps choisi : dupliquer/sauter des images (WGC n'émet que sur changement).
3. **Silences comblés** d'après les écarts QPC ; `AUDCLNT_BUFFERFLAGS_SILENT` = silence ; une
   piste ne s'arrête **jamais** sur timeout.
4. **Audio par appli** : `AudioClient::new_application_loopback_client(pid_parent, true)`.
   Persistance par **nom d'exe**, jamais par PID ; rattachement automatique au redémarrage.
5. **Piste muette anormale** (zéro numérique alors que la session de l'appli a du signal) →
   alerte `app_track_silent`.
6. **Ordre des pistes** : 1 = mix complet, 2 = micro, puis une par appli. Titres des pistes
   écrits dans les métadonnées.
7. **Fichiers** : enregistrement en MP4 `movflags +hybrid_fragmented` ; replay en MP4 classique.
8. **Aucune injection** dans un autre processus (anti-cheat). Uniquement WGC + WASAPI.
9. **Jamais de vol de focus** quand un jeu est au premier plan (sauvegarde → toast).
10. **Légèreté** : fenêtre fermée = WebView **détruit** ; aucune boucle de polling à vide.
11. **Le moteur (`crates/engine`) ne dépend jamais de Tauri.**
12. Pas de Tauri 3 alpha : on reste sur Tauri **2.x** stable.

## 4. Conventions de code

- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  et `npm run build` (dans `app/`) passent avant chaque commit. La CI fait pareil sur Windows.

### Commentaires

- **Pas de commentaires dans le code.** Pas de doc comments (`///`, `//!`, JSDoc), pas de
  commentaires qui paraphrasent le code, pas de renvois à `CLAUDE.md`, à une phase ou à une
  règle, pas de `// Phase N`, pas de code commenté, pas de commentaires dans les fichiers de
  config (TOML, YAML, JSON, CSS, `.gitignore`, scripts).
- Le code s'explique par ses noms (types, fonctions, variables, tests). Si un passage a besoin
  d'une explication, c'est d'abord un signal pour mieux nommer ou découper.
- Seules exceptions, en une ligne et sans phrase d'introduction :
  - `// SAFETY:` avant chaque bloc `unsafe` (exigé par le lint `undocumented_unsafe_blocks`) ;
  - un « pourquoi » réellement non évident (contournement d'un bug Windows, valeur imposée par
    une API), avec le lien vers la source.
- Le contexte, les règles et les raisons vivent dans `docs/` (ARCHITECTURE, DECISIONS,
  RESEARCH), jamais dans le code.
- Noms de tests en anglais, au format comportement (`replay_starts_on_keyframe`).
- Les textes visibles par l'utilisateur (erreurs, `about` de la CLI) restent en français.
- Messages d'erreur destinés à l'utilisateur : en **français**, actionnables. Logs : `tracing`,
  structurés, avec par piste : paquets, trous comblés, crête.
- Nouvelle dépendance : justifiée dans la PR, version **stable**, ajoutée à
  `[workspace.dependencies]` si partagée.
- Contrat IPC (commandes Tauri + `EngineEvent`) : toujours synchronisé entre
  `app/src-tauri/src/commands.rs`, `crates/engine/src/events.rs`, `app/src/lib/ipc.ts` et
  `docs/ARCHITECTURE.md` § IPC.
- Doute sur le comportement d'une API Windows : écris d'abord une sous-commande de test dans
  `crates/cli`, ne suppose pas.

### Git

**Commits** — [Conventional Commits 1.0](https://www.conventionalcommits.org/en/v1.0.0/),
strictement :

```
<type>(<scope>): <description>
```

- `type` : `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `build`, `ci`, `chore`, `style`,
  `revert`.
- `scope` (obligatoire) : `engine`, `capture`, `audio`, `encode`, `replay`, `record`, `mux`,
  `library`, `cli`, `app`, `ui`, `ipc`, `deps`, `ci`, `scripts`, `docs`, `handoff`, `reports`,
  `repo`.
- `description` : en anglais, à l'impératif, en minuscules, sans point final, ≤ 72 caractères
  pour toute la première ligne. Ex. : `feat(capture): add cfr frame pacer`,
  `fix(audio): fill silence gaps from qpc timestamps`, `docs(handoff): update session notes`.
- Corps facultatif (ligne vide puis texte) : le **pourquoi**, pas la liste des fichiers.
- Changement cassant : `!` après le scope et pied `BREAKING CHANGE: ...`.
- Un commit = un changement cohérent. Pas de commit fourre-tout.

**Branches** — [Conventional Branch](https://conventionalbranch.org/) :

```
<type>/<description>
```

- `type` : `feature/`, `fix/`, `hotfix/`, `release/`, `chore/`. Jamais de préfixe d'agent
  (`claude/`, `ai/`...).
- `description` : minuscules, chiffres et tirets uniquement, courte. Ex. :
  `feature/video-capture`, `feature/per-app-audio`, `fix/replay-keyframe-alignment`,
  `chore/ffmpeg-setup`, `release/v1.0.0`.
**Branches longues** — deux, et **jamais de commit direct** sur l'une ou l'autre : tout passe
par une PR.

- `main` : versions publiées uniquement. Alimentée seulement par des PR `dev` → `main` ou
  `release/vX.Y.Z` → `main` (et les `hotfix/`).
- `dev` : branche d'intégration. Toute branche `feature/`, `fix/`, `chore/` part de `dev` et
  revient dans `dev` par PR.
- `release/vX.Y.Z` part de `dev`, est mergée dans `main` (puis le tag), et `main` est remergée
  dans `dev` si la release a reçu des correctifs.
- `hotfix/` part de `main` et est mergée dans `main` **et** dans `dev`.

**Merge et ménage** — dès que la CI d'une PR vers `dev` est verte et que les critères de la
tâche sont vérifiés, tu la merges toi-même puis tu supprimes la branche :

- `gh pr merge <N> --merge --subject "<titre conventionnel de la PR> (#<N>)" --delete-branch`
  (commit de merge au format conventionnel, branche supprimée en local et sur GitHub) ;
- puis `git switch dev && git pull --ff-only && git fetch --prune`, et supprime toute autre
  branche locale déjà contenue dans `dev` (`git branch -d`).
- Il ne reste que `main` et `dev` (plus les branches de travail en cours).
- Jamais de merge vers `main` sans demande explicite de Palawi (publication d'une version ou
  `hotfix/`).

**Tags** : `vMAJEUR.MINEUR.CORRECTIF` (`v1.0.0`), posés uniquement sur `main`.

- **Identité des commits** : tout agent (Claude Code ou autre) commite **uniquement** avec le
  profil git de l'utilisateur (`git config user.name` / `user.email`, aujourd'hui
  `Palawi <bapcoloj@gmail.com>`). Jamais de trailer `Co-Authored-By`, `Claude-Session`, ni de
  mention « Generated with Claude » dans les commits, PR, tags ou releases. L'attribution est
  coupée dans `.claude/settings.json` ; ne la réactive pas.
- Si le profil git est vide ou que tu ne sais vraiment pas qui est l'utilisateur, **demande-lui**
  avant de commiter. N'invente jamais d'auteur et ne commite jamais sous une identité d'agent.

## 5. Carte du repo

```
crates/engine/      moteur (capture, audio, encodage, replay, mux) — sans UI ; les modules
                    sont créés au fil des phases (responsabilités : docs/ARCHITECTURE.md § 2)
crates/cli/         banc de test du moteur (une sous-commande par phase)
app/                front React + TS + Tailwind 4 (Vite)
app/src-tauri/      shell Tauri 2 : commandes, zone de notification, raccourcis, plugins
scripts/            fetch-ffmpeg.ps1, check-env.ps1
docs/               SPEC, ARCHITECTURE, PLAN, HANDOFF, DECISIONS, RESEARCH, TESTING, DESIGN-BRIEF
docs/reports/       rapports de validation de chaque phase
.claude/commands/   /reprendre, /handoff, /valider-phase, /design
```

## 6. Environnement

- Windows 11 (build ≥ 20348), toolchain MSVC, VS Build Tools (C++), LLVM (bindgen), Node ≥ 20.19.
- `pwsh scripts/check-env.ps1` vérifie tout. `pwsh scripts/fetch-ffmpeg.ps1` installe FFmpeg LGPL.
- Machine de Palawi : RTX 4070 Super, i5 13e gen, écran 1440p 240 Hz. Jeu de test principal :
  Fortnite (Easy Anti-Cheat).
