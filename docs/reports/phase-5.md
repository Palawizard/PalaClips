# Phase 5 — Application et sélecteurs

- Date de validation : 2026-10-17
- Commit : `5c0c34c` (`dev`, après #35)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF, trois écrans (principal 2560×1440 240 Hz)

**Statut : un critère 👤 reste ouvert** (ressenti des sélecteurs, à voir avec Palawi) ; tous
les livrables et les autres critères sont faits.

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Fenêtre fermée : un seul processus PalaClips, aucun `msedgewebview2.exe` enfant | 🤖 | ✅ | Build release isolée (`scripts/app-isolated.ps1`) : fenêtre ouverte `msedgewebview2.exe ×6, palaclips-app.exe ×1` → après `CloseMainWindow` : `palaclips-app.exe ×1`, processus toujours vivant (zone de notification) |
| `/impeccable audit` sans problème bloquant ; navigation clavier complète | 🤖 | ✅ | Audit initial 15/20 (P0 0, P1 3, P2 6, P3 4), détecteur `[]` ; corrections #35 → 18/20 « Excellent », puis derniers P2/P3 corrigés (focus dans la grille Capture, Tab libre pendant la saisie d'un raccourci, focus rendu après confirmations et listes, région live des alertes). Écart assumé : pas de zoom d'interface (P2-6), la mise à l'échelle Windows 100–200 % couvre APP-13 |
| Sauvegarde en plein écran dans Fortnite : le jeu garde le focus, le toast apparaît | 👤 | ✅ | Tests du 2026-10-17 ci-dessous : focus gardé, carillon entendu en jeu, toast retenu par Windows en plein écran (Ne pas déranger) puis présent dans le centre de notifications, toast visible sur le bureau. Palawi : critère rempli |
| Palawi valide le ressenti des sélecteurs (« propre et sympa ») | 👤 | ❌ | En attente : Palawi regardera Capture et Audio dans l'app isolée |

## Livrables

- Shell Tauri (zone de notification, instance unique, fenêtre détruite, raccourcis globaux
  personnalisables avec conflits), toasts sans vol de focus et carillon, `settings.json`
  versionné avec migration et réinitialisation : PR #19 à #27.
- Écrans : État, Réglages > Capture (miniatures en direct, ~0,35 % CPU machine pour
  3 écrans + 6 fenêtres, 0 % hors section), Audio (vumètres, recherche, appli non lancée,
  renommage), Replay & qualité (estimation en direct, alerte RAM, limites du codec), Raccourcis,
  Stockage, Général : PR #28 à #33.
- Assistant de calibrage (AUD-11) : PR #33, corrigé par #34 (ADR-027 : référence = capture par
  processus de l'émetteur). Trois mesures sur la machine de Palawi : excédent Voicemeeter
  103–106 ms, Jeu et applis 0 ms.
- Contrat IPC à jour (`ipc.ts`, `commands.rs`, `events.rs`, `ARCHITECTURE.md` § IPC).
- Aucun `todo!()` ni usage de `EngineError::NotImplemented` (seule la définition de la variante
  reste dans `error.rs`).

## Exigences SPEC couvertes

APP-01, APP-02, APP-03, APP-04, APP-05, APP-07, APP-13 ; CAP-02, CAP-03, CAP-05, CAP-06,
CAP-07 ; AUD-02, AUD-04, AUD-05, AUD-06, AUD-11 ; RPL-01, RPL-02, RPL-03, RPL-04, RPL-06,
RPL-07 ; ENC-02, ENC-03 ; STO-02, STO-03.

## Écarts et corrections

- Calibrage : la première version alignait chaque piste sur zéro (Mix −138, Jeu −33 ms) ;
  corrigé par ADR-027 avant toute application (rien n'a été appliqué aux réglages de Palawi).
- Quota (STO-04) et disque plein (STO-05) : phase 6, comme prévu au PLAN.

## Mesures en cours de phase

### PERF-03 : charge de l'app pendant une capture (2026-10-17)

`pwsh scripts\app-load.ps1` : build release (`npx tauri build --no-bundle`), replay armé avec
la config par défaut (écran principal 2560×1440 à 60 i/s, `h264_nvenc` 40 Mb/s, pistes mix,
micro, Jeu), 20 s de mise en régime puis 60 s par mesure ; CPU = temps processeur de l'app et
de tous ses processus enfants (WebView2) ; GPU = compteurs « GPU Engine » de ces processus,
moyenne par moteur.

| Configuration | CPU (machine, 20 cœurs logiques) | CPU (d'un cœur) | GPU 3D | GPU encodage |
|---|---|---|---|---|
| Fenêtre ouverte sur un écran secondaire, sans focus (7 processus) | 0,57 % | 11,5 % | 0,45 % | 19,3 % |
| Fenêtre fermée, zone de notification (1 processus) | 0,27 % | 5,3 % | 0,43 % | 19,3 % |

- Vumètres sans focus : une valeur appliquée à chaque événement (10 par seconde), aucune
  boucle d'animation entre deux événements ; avec focus, animation plafonnée à 60 i/s
  (`fix(ui): cap meter animation and skip it in the background`).
- La WebView ouverte sans focus coûte ~0,3 % du CPU de la machine et ~0,02 % de GPU 3D.
- Fenêtre ouverte, le focus est resté sur la fenêtre de Palawi (vérifié par
  `GetForegroundWindow`).
- Limite : trois pistes (la config par défaut), pas quatre ; à refaire avec quatre pistes et
  en jeu GPU-bound pour la validation de PERF-03 en phase 7.
- Journal : `docs/reports/phase-5/app-load-2026-10-17_11-51.txt`.

### Tests 👤 de l'écran État avec Palawi (2026-10-17)

Build release (`target\release\palaclips-app.exe`), replay armé avec la config par défaut,
Fortnite en plein écran sur l'écran principal.

| Test | Résultat |
|---|---|
| Ressenti de l'écran État | ✅ validé ; une phrase jugée « style IA » retirée (« 3 pistes dans chaque clip, dans cet ordre ») avec l'accroche de l'état vide des clips |
| `Alt+F10` dans Fortnite plein écran : le jeu garde le focus | ✅ rien n'est passé devant le jeu ; 2 clips de 303 Mo écrits (supprimés après le test). Le toast et le son (RPL-06) ne sont pas encore faits : critère de phase « le toast apparaît » encore ouvert |
| Sauvegarde fenêtre ouverte : surlignage des pistes et carte du clip qui arrive | ✅ |

- Remarque : les clips de Fortnite sont rangés sous `FortniteClient-Win64-Shipping\` (nom
  d'exe sans `.exe`, STO-03). Un nom lisible (« Fortnite », lu dans les métadonnées de l'exe)
  serait plus propre : à décider avec Palawi.

### Confirmation de sauvegarde et nom du jeu, avec Palawi (2026-10-17)

Build release, Fortnite plein écran.

| Test | Résultat |
|---|---|
| `Alt+F10` en partie : carillon de PalaClips, jeu au premier plan | ✅ entendu, focus gardé |
| Toast pendant la partie | ✅ bloqué par Windows (Ne pas déranger automatique), retrouvé dans le centre de notifications |
| `Alt+F10` sur le bureau : toast + carillon | ✅ |
| Dossier et nom du clip | ✅ `Vidéos\PalaClips\Fortnite\Fortnite_2026-10-17_12-28-33_replay.mp4` ; nom figé dans `settings.json` (`fortniteclient-win64-shipping.exe` → `Fortnite`) |

- Le carillon passe par la sortie par défaut : il est enregistré dans la piste « Mix complet »
  des clips suivants (piste Jeu non touchée).
- Hors installation (`target\release`), le toast porte l'identité de PowerShell : l'identifiant
  de l'app ne s'applique qu'une fois installée (phase 8).
