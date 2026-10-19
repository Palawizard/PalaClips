# Phase 6 — Enregistrement, galerie, lecteur

- Date de validation : 2026-10-19
- Commit : `0857bda` (`dev`, après #48)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF, trois écrans (principal 2560×1440 240 Hz)

**Statut : phase 6 validée** (2026-10-19) : tous les critères et livrables sont faits.

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Tuer le processus pendant un enregistrement : fichier lisible, finalisé au redémarrage | 👤 → 🤖 | ✅ | `scripts/recording-crash.ps1` sur `0857bda` : `taskkill /F /T` après 12 s → fichier lisible sur 12,01 s avant tout redémarrage ; relance → `recordings.txt` vidé, clip `recording` de 12,01 s dans la galerie. Découpage 1 min vérifié au passage : 60,03 s + 15,33 s. `phase-6/recording-crash-2026-10-19.txt` |
| Clip de 4 Go : déplacement dans le lecteur < 300 ms | 🤖 | ✅ | Enregistrement factice 1 h, 6 pistes, 4,26 Go (`scripts/gallery-bench.ps1`). 12 déplacements à l'arrêt : image affichée p50 192 ms, max 288 ms ; 6 pistes recalées max 141 ms. 12 en lecture : max 220 ms (image), 166 ms (pistes). `phase-6/gallery-bench-2026-10-19.txt` |
| 5 000 clips factices : défilement à 60 fps, ouverture de la galerie < 500 ms | 🤖 | ✅ | 5 001 clips dans le dossier isolé, supprimés à la fin. Ouverture : 74 ms (médiane de 6), max 102 ms ; `list_clips` 31 ms. Défilement 24 px/image (5 700 px/s) : 226 i/s, p95 8,3 ms, aucune image > 16,7 ms ; 120 px/image (28 600 px/s) : 117 i/s, p95 16,6 ms, aucune > 33 ms |
| RAM du lecteur sur un enregistrement d'1 h (6 pistes), sans croissance avec la durée | 🤖 | ✅ | Mémoire privée app + WebView2 après 20 s de lecture à 0, 15, 30, 45, 59 min : 857, 802, 805, 810, 812 Mo (+11 Mo de 15 à 59 min) ; cache des pistes 498 Mo sur disque, aucune piste en mémoire. Avant correction : +1 600 Mo (`phase-6/gallery-bench-2026-10-19-before-fix.txt`) |
| Glisser un clip copié dans Discord : envoi OK, son = mix complet avec le micro | 👤 + 🤖 | ✅ | Premier essai de Palawi : envoi OK mais sans la voix. ADR-029 (micro dans la piste 1) : clip enregistré en jeu en parlant, `Ctrl+C`, collé dans Discord → voix présente, Palawi : « bon ». ADR-030 (piste 1 = somme des pistes choisies) vérifié sans nouvel essai Discord, à la demande de Palawi : `palaclips-cli replay --tracks-mix --app ffplay.exe` (sinus 440 Hz) → piste 1 sans capture propre (0 paquet) identique à la piste de l'appli, crête −43,08 dB et RMS −46,30 dB sur les deux |

## Autres vérifications

| Vérification | Type | Résultat | Preuve |
|---|---|---|---|
| Enregistrement réel en jeu (REC-01) | 👤 | ✅ | App isolée, `Ctrl+Alt+F9` en jeu, 2–3 min : point rouge dans la zone de notification, clip dans la galerie, lecture, pistes séparées, déplacements. Palawi : « ok ». Menu de la zone de notification : entrée « Démarrer / Arrêter l'enregistrement » (`tray.recordStart` / `tray.recordStop`) |
| Disque plein (STO-05) | 👤 | ✅ | `scripts/disk-full.ps1` en administrateur : VHDX 1 Go en Y:, 80 Mb/s → `disk_almost_full`, arrêt `recording_failed` sous 300 Mo libres, fichier lisible de 76,0 s (725 Mo), sauvegarde du replay refusée (`clip_save_failed`), journal des enregistrements vide, VHDX démonté et supprimé. `phase-6/disk-full-2026-10-19.txt`. Test unitaire `a_full_disk_stops_the_recording_and_the_file_is_recovered_at_startup` |
| Replay inclus au début (REC-04) | 🤖 | ✅ | CLI, replay de 10 s armé depuis 12 s, 5 s d'enregistrement : 15,1 s avec `--record-include-replay`, 5,1 s sans |
| Découpage (REC-05) | 🤖 | ✅ | Banc ci-dessus (60 s + 15 s) ; tests `a_duration_split_waits_for_the_next_video_keyframe`, `a_size_split_counts_the_bytes_of_the_current_file`, `a_recording_without_split_never_splits` |
| Découpe et export (GAL-06, GAL-07) | 🤖 | ✅ | Tests `a_fast_cut_starts_on_the_keyframe_and_keeps_the_chosen_tracks`, `mixed_tracks_become_one_titled_track`, `a_cancelled_or_too_short_export_leaves_no_file`, `a_precise_cut_starts_on_the_exact_frame` (`--ignored`, NVENC, réussi) ; UI vue avec Palawi pendant `/design` (#45) |
| Bibliothèque (STO-04, GAL-08) | 🤖 | ✅ | Tests `the_quota_deletes_the_oldest_clips_but_never_a_favorite`, `sync_indexes_new_clips_forgets_vanished_ones_and_keeps_favorites`, `changes_arrive_together_once_the_folder_is_quiet` ; 5 000 clips copiés hors de l'app indexés avec miniature en 32 s |
| Sélection d'un clip (GAL-05) | 👤 | ✅ | Retour de Palawi : un clic sélectionne (bordure, raccourcis dessus), double-clic ou `Entrée` ouvre. Vérifié par CDP puis par Palawi : « bon » |
| Ni `todo!()` ni `EngineError::NotImplemented` utilisés | 🤖 | ✅ | `grep` : la variante `NotImplemented` n'est que déclarée (`error.rs`), aucun usage, aucun `todo!()` / `unimplemented!()` |
| Suite automatique | 🤖 | ✅ | `cargo test --workspace` : 149 + 32 + 2 réussis, 1 ignoré (matériel, lancé à part) ; `cargo clippy -D warnings`, `npm run build`, `npm test` (4) ; CI verte sur #46, #47, #48 |

## Exigences SPEC couvertes

REC-01 à REC-06, STO-02, STO-04, STO-05, GAL-01 à GAL-08 ; AUD-01 précisé (ADR-029, ADR-030).

## Écarts et corrections

- **RAM du lecteur** : la variable CSS `--progress` écrite à chaque image sur la racine du
  lecteur recalculait les styles de tout le sous-arbre ; WebView2 accumulait ~330 Mo par
  minute de lecture (3,3 Go sur l'heure), rendus seulement au repos. Têtes de lecture et barre
  lue mises à jour directement (#46). Isolé par élimination : pistes audio seules, `<video>`
  seul par `palaclips://` et par HTTP local ne grossissaient pas.
- **Défilement** : la grille ne se re-rend plus qu'à chaque déplacement de 200 px de la fenêtre
  visible : 24 px/image de 121 à 226 i/s, 120 px/image de 79 à 117 i/s (#46).
- **Isolation des tests** : avec `PALACLIPS_CONFIG_DIR`, miniatures, pistes extraites et buffer
  de replay vont dans `<dossier>\local\` ; le banc vérifie que `%LOCALAPPDATA%\PalaClips\cache`
  n'a pas bougé (#46).
- **Piste 1** : micro ajouté (ADR-029, #47), puis piste 1 = somme des pistes choisies par
  défaut, « Audio du bureau » en option (ADR-030, #48), à la demande de Palawi.
- **Disque plein** : diskpart échouait au formatage et le script laissait le VHDX attaché ;
  formatage par les cmdlets Storage, détachement garanti (#47).
- **Premier lancement guidé** (APP-06, phase 8) : proposera le micro dans la piste 1, l'audio
  du bureau et le dossier des clips.

## Méthode

App isolée pilotée par le protocole de débogage de WebView2 (`scripts/cdp.mjs`, port local,
clics et commandes IPC dans la page, aucune touche simulée), fenêtre sans focus sur l'écran
principal. Pistes du lecteur en sourdine pendant les mesures. RAM : `WorkingSetPrivate` de
l'app et de ses processus WebView2.
