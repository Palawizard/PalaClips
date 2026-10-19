# Phase 6 — Enregistrement, galerie, lecteur

- Mesures automatiques : 2026-10-19, `dev` + `chore/phase-6-bench` (build release, app isolée)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER, Intel Core i5-13600KF,
  écran principal 2560×1440 240 Hz

**Statut : critères vérifiés** (2026-10-19) : 🤖 mesurés, 👤 faits avec Palawi le même jour
(`docs/TESTING.md` § T6) ; reste `/valider-phase`.

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Tuer le processus pendant un enregistrement : fichier lisible, finalisé au redémarrage | 👤 → 🤖 | ✅ | `scripts/recording-crash.ps1` : `taskkill /F /T` après 12 s → fichier lisible sur 13,01 s avant redémarrage ; relance → `recordings.txt` vidé, clip `recording` de 13,01 s dans la galerie. Découpage 1 min vérifié au passage : 60,01 s + 15,54 s pour 75 s. `phase-6/recording-crash-2026-10-19.txt` |
| Clip de 4 Go : déplacement dans le lecteur < 300 ms | 🤖 | ✅ | Enregistrement factice 1 h, 6 pistes, 4,26 Go (`scripts/gallery-bench.ps1`). 12 déplacements à l'arrêt : image affichée p50 192 ms, max 288 ms ; 6 pistes recalées max 141 ms. 12 en lecture : max 220 ms (image), 166 ms (pistes). `phase-6/gallery-bench-2026-10-19.txt` |
| 5 000 clips factices : défilement à 60 fps, ouverture de la galerie < 500 ms | 🤖 | ✅ | 5 001 clips dans le dossier isolé, supprimés à la fin. Ouverture : 74 ms (médiane de 6), max 102 ms ; `list_clips` 31 ms. Défilement 24 px/image (5 700 px/s) : 226 i/s, p95 8,3 ms, aucune image > 16,7 ms ; 120 px/image (28 600 px/s) : 117 i/s, p95 16,6 ms, aucune > 33 ms |
| RAM du lecteur sur un enregistrement d'1 h (6 pistes), sans croissance avec la durée | 🤖 | ✅ | Mémoire privée app + WebView2 après 20 s de lecture à 0, 15, 30, 45, 59 min : 857, 802, 805, 810, 812 Mo (+11 Mo de 15 à 59 min) ; cache des pistes 498 Mo sur disque, aucune piste en mémoire. Avant correction : +1 600 Mo (`phase-6/gallery-bench-2026-10-19-before-fix.txt`) |
| Glisser un clip copié dans Discord : envoi OK, son = mix complet avec le micro | 👤 | ✅ | Premier essai : envoi OK mais sans la voix (la piste 1 ne contenait pas le micro). Après ADR-029 (micro aussi dans la piste 1) : clip enregistré en jeu en parlant, sélectionné d'un clic, `Ctrl+C`, collé dans Discord → voix présente. Palawi : « bon » |
| Enregistrement réel en jeu | 👤 | ✅ | App isolée, `Ctrl+Alt+F9` en jeu, 2–3 min : point rouge dans la zone de notification, clip dans la galerie, lecture, pistes séparées, déplacements. Palawi : « ok » |
| Disque plein sur VHDX (STO-05) | 👤 | ✅ | `scripts/disk-full.ps1` en administrateur : VHDX 1 Go en Y:, enregistrement à 80 Mb/s → `disk_almost_full`, puis arrêt `recording_failed` sous 300 Mo libres, fichier lisible de 76,0 s (725 Mo), sauvegarde du replay refusée (`clip_save_failed`), journal des enregistrements vide, VHDX démonté et supprimé. Premier essai : échec de diskpart au formatage, script corrigé (cmdlets Storage, détachement garanti). `phase-6/disk-full-2026-10-19.txt` |
| Sélection d'un clip | 👤 | ✅ | Retour de Palawi (raccourcis seulement au survol puis touche) : un clic sélectionne (bordure), raccourcis sur la sélection, double-clic ou `Entrée` ouvre. Vérifié par CDP puis par Palawi : « bon » |

## Corrections issues des mesures

- **RAM du lecteur** : la variable CSS `--progress` écrite à chaque image sur la racine du
  lecteur recalculait les styles de tout le sous-arbre ; WebView2 accumulait ~330 Mo par
  minute de lecture (3,3 Go sur l'heure), rendus seulement au repos. Les têtes de lecture et la
  barre lue sont maintenant mises à jour directement. Isolé par élimination : pistes audio
  seules, `<video>` seul par `palaclips://` et par HTTP local ne grossissaient pas.
- **Défilement** : la grille ne se re-rend plus qu'à chaque déplacement de 200 px de la fenêtre
  visible (au lieu de chaque image) : 24 px/image de 121 à 226 i/s, 120 px/image de 79 à
  117 i/s.
- **Isolation des tests** : avec `PALACLIPS_CONFIG_DIR`, miniatures, pistes extraites et
  buffer de replay vont dans `<dossier>\local\` ; le banc vérifie que
  `%LOCALAPPDATA%\PalaClips\cache` n'a pas bougé.

## Méthode

App isolée pilotée par le protocole de débogage de WebView2 (`scripts/cdp.mjs`, port local,
clics et commandes IPC dans la page, aucune touche simulée), fenêtre sans focus sur l'écran
principal. Pistes du lecteur en sourdine pendant les mesures. RAM : `WorkingSetPrivate` de
l'app et de ses processus WebView2.
