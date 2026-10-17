# Phase 5 — Application et sélecteurs

- Branche : `feature/app-shell` (PR #19, en cours)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF, trois écrans (principal 2560×1440 240 Hz)

**Statut : en cours.** Rapport complété par `/valider-phase` à la fin de la phase.

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
