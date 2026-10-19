# Phase 7 — Robustesse et performances

- Mesures sur la build installée : 2026-10-19, `dev` après #59 + `test/isolated-migration`
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER, Intel Core i5-13600KF
  (20 cœurs logiques), écran principal 2560×1440 240 Hz

**Statut : en cours.** PERF-01, 02, 04 et la part CPU de PERF-03 sont mesurés ; restent la
perte de fps en jeu (PERF-03, GPU-bound), les scénarios T7 et l'essai G-Sync (CAP-11).

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| PERF-01 : zone de notification, replay désarmé, < 30 Mo, 0 % CPU | 🤖 | ✅ | Build installée (`setup.exe /S`), réglages isolés, `--autostart` : 4,9 Mo (working set privé, médiane sur 60 s), CPU 0,00 % de la machine, 1 seul processus (aucun WebView). `phase-7/perf-installed-2026-10-19.txt` |
| PERF-02 : replay armé, moteur hors tampon < 150 Mo | 🤖 | ✅ | Écran principal 2560×1440 à 60 i/s, 3 pistes, tampon 30 s : 243,4 Mo privés − tampon 164,7 Mo × 1,07 = **67,3 Mo** (même calcul que le panneau de performances) |
| PERF-03 : < 2 % CPU (1440p60 NVENC) | 🤖 | ✅ (CPU) | Replay armé, fenêtre fermée : 0,39 % de la machine. Perte de fps en jeu ≤ 2 % (GPU-bound) : à faire avec Palawi |
| PERF-04 : démarrage de l'UI < 1 s à chaud | 🤖 | ✅ | Relance (instance unique) → premier rendu avec l'état : première 652 ms, puis 534, 538, 563 ms à chaud |
| Scénarios T7 | 👤 | ⏳ | Séance avec Palawi (`docs/TESTING.md` § T7) |
| Alerte G-Sync (CAP-11) | 👤 | ⏳ | Code et tests faits (#51, #54) ; essai réel en séance T7 |

## Installation et désinstallation

- Installation silencieuse par utilisateur : 7 DLL FFmpeg, 4 licences, entrée de
  désinstallation ; aucun processus lancé, aucune entrée « Démarrer avec Windows ».
- Désinstallation sans la case : programme, entrée et raccourci retirés ;
  `%LOCALAPPDATA%\PalaClips` ne garde que `cache` et `replay` de Palawi ; ses 583 fichiers de
  données (`%APPDATA%\PalaClips`, `cache`, `replay`, dossiers `com.palawi.palaclips`) identiques
  en SHA-256 avant et après (`scripts/real-data-fingerprint.ps1`) ; dossier des clips inchangé.
- Migration en mode isolé : jamais (`an_isolated_app_never_migrates_the_real_data_folders`), et
  les empreintes ci-dessus le confirment sur la vraie machine.
- Désinstallation avec la case : à faire avec Palawi (`scripts/uninstall-check.ps1`).
