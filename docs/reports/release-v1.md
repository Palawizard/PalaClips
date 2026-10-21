# Revue finale V1

- Date : 2026-10-21 ; branche `dev` après #79 (dernière PR de la phase 8 avant la release).
- Méthode : chaque exigence de `docs/SPEC.md` reprise une à une, avec la preuve trouvée dans les rapports de phase (`docs/reports/phase-N.md`), leurs journaux (`docs/reports/phase-N/`), les tests du code ou les PR.
- Statuts : ✅ prouvé (preuve citée) · ⏳ travail de phase 8 encore en cours · ❌ non tenu ou sans preuve.

## 1. Pour qui, pour quoi (exigences sans identifiant)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| — | Plateforme : Windows 11 (et Server 2022, build ≥ 20348), x64 | ✅ | SPEC corrigée et alerte `unsupported_windows_build` au démarrage (PR #77, test `only_a_known_build_below_20348_is_too_old`) ; aucune dépendance au Visual C++ redistribuable (`phase-8/dumpbin-2026-10-21.txt`) ; installeur essayé dans une VM Windows 11 26H2 vierge, sans GPU (`phase-8/vm-t10-2026-10-21.txt`). |
| — | GPU avec encodeur matériel (NVENC, AMF, QSV) | ✅ | Voir ENC-01 : NVENC prouvé ; AMF et QSV écartés par l'encodage de test quand ils échouent (`phase-1.md`, tâche « Détection des encodeurs »). |
| — | Léger | ✅ | Voir PERF-01 à PERF-04 (`phase-7.md`). |
| — | Gratuit, open source | ✅ | `LICENSE` (MIT, ADR-011). |
| — | Sans compte ni télémétrie | ✅ | Aucune bibliothèque HTTP dans les `Cargo.toml` ; CSP de `tauri.conf.json` limitée à `self`/`ipc:` ; seul accès réseau : l'updater vers GitHub Releases (ADR-034, PR #75). Le diagnostic n'est créé que sur demande et reste local (PR #71). |

## 2.1 Capture vidéo (CAP)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| CAP-01 | Écran ou fenêtre au choix, par WGC | ✅ | `phase-1.md`, critères « Le MP4 se lit… » (`capture-video --target primary`) et « Capture de Fortnite en fenêtre et en plein écran ». |
| CAP-02 | Sélecteur avec miniatures en direct, nom et icône | ✅ | `phase-5.md` § Livrables (Réglages > Capture, ~0,35 % CPU) et critère « Palawi valide le ressenti des sélecteurs » (« Tout est bon ») ; repris dans le premier lancement (PR #72). |
| CAP-03 | Cible mémorisée (écran par nom de périphérique, fenêtre par nom d'exe), retrouvée au redémarrage | ✅ | Fenêtre : `phase-1.md`, tâche « Énumération » (`resolve-target --target window:VESKTOP.exe`). Écran : les noms GDI se renumérotant, il est aussi mémorisé par son chemin stable de moniteur (PR #79) ; les 3 écrans de Palawi retrouvés par ce chemin, un chemin absent refusé (`phase-8/stable-monitor-2026-10-21.txt`) ; tests `a_known_monitor_is_found_under_its_new_gdi_name`, `an_unplugged_monitor_is_not_replaced_by_another_one`, `a_monitor_keeps_its_stable_path_and_old_settings_still_load`. |
| CAP-04 | Repli sur l'écran si la fenêtre échoue, avec message | ✅ | `phase-1.md`, tâche « Repli fenêtre → écran » (alerte `capture_fallback_monitor`). |
| CAP-05 | fps constant 30…240 ou libre 10–240 | ✅ | `phase-1.md` : 60 i/s (`r_frame_rate=60/1 avg_frame_rate=60/1`), 144 i/s, 240 i/s (§ Suivi, 7 200 images) ; test `out_of_range_fps_is_rejected` ; valeur libre dans Réglages > Replay & qualité (`phase-5.md`). |
| CAP-06 | Résolution native ou mise à l'échelle sur le GPU | ✅ | `phase-1.md`, tâche « Mise à l'échelle GPU » (`--height 1080` / `720`) ; valeur libre et plafond à la source (`phase-5.md`, CAP-06). |
| CAP-07 | Curseur et bordure jaune au choix | ✅ | `phase-1.md`, tâche « Curseur, bordure » (vérifié par Palawi). |
| CAP-08 | Multi-écrans, mise à l'échelle 100–200 %, résolution à chaud | ✅ | `phase-1.md` (multi-écrans, `DISPLAY3` à 150 %, redimensionnement à chaud) ; `phase-7.md`, « T7 changement de résolution » (`phase-7/resolution-2026-10-20_17-59.txt`). Seul 150 % a été essayé dans la plage 100–200 %. |
| CAP-09 | Écran HDR : SDR aux couleurs correctes | ✅ | `phase-1.md`, critère « Écran HDR » (après correction, ADR-015/016) ; `phase-7.md`, « HDR basculé pendant une capture » (`phase-7/hdr-toggle-2026-10-20_18-16.txt`). |
| CAP-10 | Aucune injection ; compatible anti-triches | ✅ | `phase-1.md`, critère « Aucune lecture CPU » et § Exigences (WGC seul) ; essais dans Fortnite (Easy Anti-Cheat) en phases 1, 5 et 7. BattlEye et Vanguard non essayés (même mécanisme). |
| CAP-11 | Alerte cadence DWM sous le fps demandé ; jeu plafonné ne la déclenche pas | ✅ | `phase-7.md`, critère « Alerte G-Sync (CAP-11) » (`capture_source_rate_limited { rate: 165, fps: 240 }`, aucune alerte jeu plafonné) ; retrait de l'alerte : « T7 alerte G-Sync retirée » (#64). |

## 2.2 Encodage (ENC)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| ENC-01 | H.264, HEVC, AV1 matériels, détectés par encodage de test réel | ✅ | `phase-1.md`, tâche « Détection des encodeurs » (`h264_nvenc`, `hevc_nvenc`, `av1_nvenc` ; raisons d'échec AMF et QSV). AMF et QSV jamais essayés sur du vrai matériel (pas d'iGPU ni de carte AMD). |
| ENC-02 | Débit 1–200 Mb/s, préréglages + libre | ✅ | `phase-1.md`, tâche « Préréglages de débit » (test `quality_presets_scale_bitrate_with_pixels_per_second`, débits mesurés) ; UI dans `phase-5.md`. |
| ENC-03 | Intervalle d'images clés réglable (1 s par défaut) | ✅ | `phase-1.md` § Exigences (images clés à 0, 1, 2… s dans `ffprobe`) ; réglage dans `phase-5.md`. |
| ENC-04 | Aucune copie CPU des images | ✅ | `phase-1.md`, critère « Aucune lecture CPU de la texture » (grep sans résultat, 0,4 % CPU). |
| ENC-05 | Sans encodeur : message clair au démarrage | ✅ | VM sans GPU (`phase-8/vm-t10-2026-10-21.txt`, captures `vm-t10-checks.png` et `vm-t10-home.png`) : avertissement à l'étape Vérifications, « Continuer » grisé, diagnostic proposé ; alerte à l'accueil ; ponctuation et coupure du texte corrigées (PR « fix: vm test findings »). |

## 2.3 Audio (AUD)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| AUD-01 | Mix complet en piste 1 (somme des pistes choisies ou audio du bureau) | ✅ | `phase-2.md` ; `phase-6.md`, critère Discord (ADR-029 : voix présente ; ADR-030 : piste 1 identique à la piste de l'appli, crête −43,08 dB des deux côtés) ; tests `the_full_mix_adds_up_the_chosen_tracks_unless_it_takes_the_desktop`, `several_tracks_add_up_and_the_slowest_one_sets_the_pace`. |
| AUD-02 | Micro : périphérique, gain ±20 dB, suivi du défaut | ✅ | `phase-2.md` § Exigences et tâche « Suivi du périphérique par défaut » ; choix et gain dans Réglages > Audio (`phase-5.md`, AUD-02). |
| AUD-03 | Une piste par logiciel (6 par défaut, 12 max), arbre de processus | ✅ | `phase-3.md`, critère T3 (`phase-3/t3-zen-vlc.txt`, `t3-auto-tones.txt`) ; plusieurs instances d'un même exe : PR #76 (`phase-8/multi-instance-2026-10-20_23-01.txt`, Vesktop de Palawi). |
| AUD-04 | Sélecteur : applis sonores en tête, icône, nom, vumètre, recherche, appli non lancée | ✅ | `phase-5.md` § Livrables (Audio : vumètres, recherche, appli non lancée) et critère « ressenti des sélecteurs ». |
| AUD-05 | Pistes renommables, nom dans les métadonnées | ✅ | `phase-2.md` (`handler_name=Mix complet` / `Micro`) ; `phase-4.md`, critère Resolve (noms dans `hdlr` et `udta/name`, ADR-021) ; renommage : `phase-5.md` et test `renamed_tracks_keep_their_order_and_offsets`. |
| AUD-06 | Piste Jeu automatique qui suit le jeu, pas le premier plan | ✅ | `phase-3.md`, tâche « Piste Jeu automatique » (`phase-3/game-track-alt-tab.txt` : 440 Hz les 30 s, jamais 880 Hz) ; processus fixé : `CurrentGame` (ADR-019). |
| AUD-07 | Appli relancée : piste rattachée, silence entre-temps | ✅ | `phase-3.md`, critère Discord (`phase-3/discord-reattach.txt`) ; `phase-7.md`, « T7 appli capturée qui crashe » (`phase-7/app-crash-2026-10-20_16-51.txt`). |
| AUD-08 | Synchro A/V < 1 image sur 1 h, avec longs silences | ✅ | `phase-2.md`, critères T2 et « Silence de 2 min » (`phase-2/t2-av-sync-1h.txt`, 6 × `OK`). |
| AUD-09 | Alerte si une piste reste muette alors que l'appli joue | ✅ | Tests `zero_track_with_a_loud_session_alerts_once`, `zero_track_with_a_quiet_session_never_alerts`, `signal_on_the_track_rearms_the_alert` (`phase-3.md`, tâche « Détection piste muette ») ; sessions de toutes les instances (PR #76). Aucun vrai cas positif provoqué (`phase-3.md` § Écarts). |
| AUD-10 | Changement de périphérique par défaut suivi à chaud | ✅ | `phase-2.md`, critère casque (`phase-2/capture-av-unplug.txt`) ; `phase-7.md`, « T7 casque débranché ». |
| AUD-11 | Décalage par piste ±1000 ms + assistant de calibrage | ✅ | `phase-2.md`, tâche « Décalage de synchro » (test `sync_offsets_beyond_one_second_are_rejected`) ; `phase-5.md` § Livrables (assistant, PR #33/#34, trois mesures) ; `phase-7.md` § Calibrage du micro. |

## 2.4 Replay instantané (RPL)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| RPL-01 | Armé au démarrage (option), par raccourci et zone de notification | ✅ | `phase-5.md` § Exigences et Livrables (PR #19 à #27 : `arm_on_startup`, `toggle_replay`, menu Armer/Désarmer) ; test `default_hotkeys_are_valid`. |
| RPL-02 | Durée 5 s à 60 min | ✅ | `phase-4.md` § Exigences ; réglage dans `phase-5.md`. |
| RPL-03 | Estimation RAM en direct (+7 %) | ✅ | `phase-4.md` § PERF-02 (+7 % mesurés) ; test `estimate_covers_the_whole_window_with_measured_overhead` ; affichage dans `phase-5.md`. |
| RPL-04 | Tampon en RAM ou sur disque | ✅ | `phase-4.md`, tâche « `PacketRing`… RAM et disque » (essai `--disk`) ; tests `disk_packets_read_back_identical`, `purged_segments_are_deleted_from_disk`. |
| RPL-05 | Fichier prêt < 500 ms (60 s, 40 Mb/s), image clé, pistes alignées | ✅ | `phase-4.md`, critère « Sauvegarde < 500 ms » (`phase-4/replay-save-20.txt` : max 200 ms) et critère durée/pistes. |
| RPL-06 | Jamais de vol de focus ; toast + son désactivables | ✅ | `phase-5.md`, critère Fortnite plein écran et § « Confirmation de sauvegarde ». |
| RPL-07 | Changer sources ou zone redémarre le tampon, l'UI prévient | ✅ | `phase-4.md`, tâche « Redémarrage du tampon » ; test `only_capture_or_source_changes_restart_the_replay` ; confirmation dans l'UI (`phase-5.md`, RPL-07). |
| RPL-08 | RAM stable en replay armé | ✅ | `phase-4.md`, critère RAM (`phase-4/replay-ram-30min.txt` : −0,002 Mo par sauvegarde, −1,8 Mo/h ; ADR-020). |
| RPL-09 | Clips successifs sans doublon ; < 3 s : pas de clip, message discret | ✅ | Tests `the_next_clip_starts_where_the_saved_one_ended`, `a_save_one_second_after_another_is_too_short_to_keep`, `a_saved_clip_clears_the_too_short_notice` ; PR #24 et #25 (ADR-025, `HANDOFF.md`, journal du 2026-10-17 : 20,2 s puis 10,2 s). Absente des listes « Exigences couvertes » des rapports. |

## 2.5 Enregistrement manuel (REC)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| REC-01 | Démarrer/arrêter par raccourci et zone de notification, icône différente | ✅ | `phase-6.md` § Autres vérifications, « Enregistrement réel en jeu » ; test `the_recording_icon_keeps_the_size_and_adds_a_red_dot`. |
| REC-02 | Partage capture et encodeur du replay | ✅ | Sessions NVENC relevées par `nvidia-smi` : 0 au repos, 1 replay armé, toujours 1 pendant l'enregistrement, fichier écrit (`scripts/shared-encoder.ps1`, `phase-8/shared-encoder-2026-10-20_23-31.txt`) ; conception : PR #40 (`phase-4.md`, « fan-out »). |
| REC-03 | Fichier lisible après crash ou coupure (MP4 hybride) | ✅ | `phase-6.md`, critère « Tuer le processus » (`phase-6/recording-crash-2026-10-19.txt`) ; disque plein (`phase-6/disk-full-2026-10-19.txt`). |
| REC-04 | Option « replay au début de l'enregistrement » | ✅ | `phase-6.md`, « Replay inclus au début » (15,1 s avec, 5,1 s sans). |
| REC-05 | Découpage par durée ou taille | ✅ | `phase-6.md`, « Découpage » (60,03 s + 15,33 s ; tests `a_duration_split_waits_for_the_next_video_keyframe`, `a_size_split_counts_the_bytes_of_the_current_file`). |
| REC-06 | Récupération au démarrage, finalisé dans la galerie | ✅ | `phase-6.md`, critère « Tuer le processus » (relance → clip `recording` de 12,01 s) ; test `a_full_disk_stops_the_recording_and_the_file_is_recovered_at_startup`. |

## 2.6 Fichiers et stockage (STO)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| STO-01 | MP4, H.264/HEVC/AV1, AAC 48 kHz stéréo, une piste par source, CFR, importable dans Resolve | ✅ | `phase-4.md`, critère Resolve (4 pistes séparées et alignées) ; CFR dans `phase-1.md` ; AAC 48 kHz stéréo (`phase-2.md`, `AacEncoder`). |
| STO-02 | Dossier réglable, sous-dossier par jeu | ✅ | `phase-5.md` § Exigences (Stockage) et § Confirmation (`Vidéos\PalaClips\Fortnite\…`) ; test `game_is_the_folder_under_the_clips_root`. |
| STO-03 | Nom `<Jeu>_<date>[_replay].mp4` | ✅ | `phase-4.md`, tâche « Sauvegarde… nom par jeu » (4 tests `paths`) ; `phase-5.md` (`Fortnite_2026-10-17_12-28-33_replay.mp4`). |
| STO-04 | Quota : suppression des plus anciens non favoris | ✅ | `phase-6.md`, « Bibliothèque » (test `the_quota_deletes_the_oldest_clips_but_never_a_favorite`). |
| STO-05 | Vérification d'espace, alerte et arrêt propre si disque plein | ✅ | `phase-6.md`, « Disque plein » (`phase-6/disk-full-2026-10-19.txt`). |

## 2.7 Galerie et lecteur (GAL)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| GAL-01 | Grille : miniature, durée, jeu, date, taille, type | ✅ | PR #45 (grille), `phase-6.md` § Exigences ; test `replays_and_recordings_are_told_apart_by_name` ; miniatures des 5 001 clips (`phase-6/gallery-bench-2026-10-19.txt`). |
| GAL-02 | Filtres, tri, recherche ; fluide avec 5 000 clips | ✅ | `phase-6.md`, critère « 5 000 clips » (ouverture 74 ms, 226 i/s) ; test `queries_filter_search_and_sort`. |
| GAL-03 | Lecteur fluide, y compris 4 Go | ✅ | `phase-6.md`, critère « Clip de 4 Go » (max 288 ms). |
| GAL-04 | Chaque piste : muet, solo, volume | ✅ | PR #45 (une voie par piste, M / S, volume) ; `phase-6.md`, critère RAM du lecteur (6 pistes sur 1 h) et « Enregistrement réel en jeu » (pistes séparées). |
| GAL-05 | Renommer, favori, Corbeille, emplacement, copier le fichier | ✅ | `phase-6.md`, critère Discord (copier/coller) et « Sélection d'un clip » ; tests `a_renamed_clip_keeps_its_favorite_and_refuses_taken_or_invalid_names` ; PR #45 (Corbeille avec Annuler). |
| GAL-06 | Découpe rapide ou précise | ✅ | `phase-6.md`, « Découpe et export » (tests `a_fast_cut_starts_on_the_keyframe_and_keeps_the_chosen_tracks`, `a_precise_cut_starts_on_the_exact_frame`). |
| GAL-07 | Export pistes choisies ou mixées | ✅ | `phase-6.md`, « Découpe et export » (test `mixed_tracks_become_one_titled_track`). |
| GAL-08 | Clips ajoutés hors de l'app visibles seuls | ✅ | `phase-6.md`, « Bibliothèque » (tests `sync_indexes_new_clips_forgets_vanished_ones_and_keeps_favorites`, `changes_arrive_together_once_the_folder_is_quiet`). |

## 2.8 Application (APP)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| APP-01 | Zone de notification avec le menu complet | ✅ | `phase-5.md` § Exigences et Livrables ; entrée d'enregistrement dans `phase-6.md`, « Enregistrement réel en jeu ». |
| APP-02 | Fenêtre détruite à la fermeture, un seul processus | ✅ | `phase-5.md`, critère « Fenêtre fermée » ; `phase-7.md`, PERF-01 (1 seul processus). |
| APP-03 | Instance unique | ✅ | `phase-5.md` § Livrables ; `phase-7.md`, PERF-04 (relance par l'instance unique → premier rendu). |
| APP-04 | Démarrage avec Windows, en zone de notification | ✅ | PR #26 (`HANDOFF.md`, journal du 2026-10-17 : 0 WebView2) ; `phase-7.md`, PERF-01 (`--autostart`, `phase-7/perf-installed-2026-10-19.txt`). |
| APP-05 | Raccourcis personnalisables, conflits, touches réservées refusées | ✅ | `phase-5.md` § Exigences ; tests `reserved_and_game_breaking_shortcuts_are_refused`, `one_shortcut_cannot_serve_two_actions` ; conflit réel Alt+Shift+F10 (overlay NVIDIA, `HANDOFF.md`). |
| APP-06 | Premier lancement guidé complet, test de sauvegarde | ✅ | PR #72 (6 étapes, brief validé, parcouru par Palawi, test `only_a_first_launch_needs_onboarding`) ; 27,5 s jusqu'au clip de test (`phase-8/first-launch-2026-10-20.txt`). |
| APP-07 | Réglages persistants, versionnés, migration, réinitialisation | ✅ | Tests `corrupt_file_is_backed_up_and_reset`, `newer_format_is_left_untouched`, `version_one_names_equal_to_a_default_become_default_again`, `missing_fields_take_their_defaults` ; migration des dossiers (ADR-032, `phase-7.md` § Installation) ; `phase-5.md`, APP-07. |
| APP-08 | Français et anglais, langue du système par défaut | ✅ | ADR-028 (PR #37) ; PR #73 (relecture de l'anglais, `npm test` : mêmes clés et paramètres, aucune chaîne en dur). |
| APP-09 | Journaux avec rotation ; export de diagnostic | ✅ | PR #71 (7 jours gardés, zip anonymisé, tests d'anonymisation, contenu vu par Palawi). |
| APP-10 | Mises à jour automatiques signées, avec notes | ✅ | PR #75 (ADR-034) ; T8 automatisé (`scripts/update-test.ps1`, `phase-8/update-test-2026-10-21_14-25.txt`) : signature altérée refusée, 1.0.0 → 1.0.1 installée avec les notes, app relancée en 1.0.1, données de Palawi identiques ; installeurs signés par le workflow en release brouillon, supprimée ensuite. |
| APP-11 | Installeur NSIS par utilisateur ; désinstallation garde les clips | ✅ | `phase-7.md` § Installation et désinstallation (sans et avec la case, empreintes SHA-256, clip témoin intact). |
| APP-12 | Panneau de performances | ✅ | PR #55 (mesure réelle : 37,8 Mo hors tampon contre 41,8 Mo dans Windows ; vu dans l'app isolée) ; `phase-7.md`, « Panneau de performances ». |
| APP-13 | Clavier complet, contrastes, `prefers-reduced-motion` | ✅ | `phase-5.md`, critère « `/impeccable audit` » (18/20, navigation clavier) ; audit de contraste de la galerie (PR #45) ; `@media (prefers-reduced-motion: reduce)` dans `app/src/styles.css` et `lib/levels.ts`. |

## 2.9 Performances (PERF), sur la build installée

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| PERF-01 | Zone de notification, désarmé : < 30 Mo, 0 % CPU | ✅ | `phase-7.md`, PERF-01 : 4,9 Mo, 0,00 % (`phase-7/perf-installed-2026-10-19.txt`). |
| PERF-02 | Armé : moteur hors tampon < 150 Mo ; tampon affiché | ✅ | `phase-7.md`, PERF-02 : 67,3 Mo (ADR-023) ; tampon affiché sur l'écran État et dans le panneau (PR #55). |
| PERF-03 | 1440p60 NVENC + 4 pistes : < 2 % CPU, perte ≤ 2 % | ✅ | `phase-7.md` : CPU 0,39 % (mesuré avec 3 pistes) ; perte 0,7 % GPU-bound en priorité « jeu », par défaut (`phase-7/fps-impact-2026-10-20_17-43.txt`). En priorité « capture », réglage au choix : 7,5 % (ADR-033). |
| PERF-04 | UI < 1 s à chaud | ✅ | `phase-7.md`, PERF-04 : 534 à 563 ms. |

## 3. Hors périmètre (vérifié)

| ID | Exigence (courte) | Statut | Preuve |
|---|---|---|---|
| — | Pas de streaming, cloud, compte, partage par lien | ✅ | Aucune bibliothèque HTTP ; seul réseau : l'updater (ADR-034). |
| — | Pas d'éditeur au-delà de la découpe et de l'export | ✅ | GAL-06/07 seulement (PR #45). |
| — | Pas d'overlay en jeu | ✅ | CAP-10 ; retour en jeu par toast et carillon (`phase-5.md`). |
| — | Pas de suppression de bruit ni d'effets, pas de webcam, ni Linux ni macOS | ✅ | ADR-001 ; aucune source de ce type dans `AudioSource` / `CaptureTarget`. |
| — | Pas de signature Authenticode, SmartScreen documenté | ✅ | `README.md` § Installation (étape 3, SmartScreen) et `README.fr.md`. |

## 4. Critères de la phase 8 (`docs/PLAN.md`)

| Critère | Statut | Preuve ou manque |
|---|---|---|
| Premier lancement guidé → clip en < 2 min (app isolée, profil vierge, temps lu dans le journal) | ✅ | 27,5 s (`phase-8/first-launch-2026-10-20.txt`). |
| Mise à jour 1.0.0 → 1.0.1 et fausse signature refusée (T8) | ✅ | `phase-8/update-test-2026-10-21_14-25.txt`. |
| Dépendances de l'exe et des DLL (`dumpbin /dependents`) | ✅ | `phase-8/dumpbin-2026-10-21.txt` : aucun runtime VC++. |
| Installeur dans une VM Windows 11 sans GPU (T10) | ✅ | 8 lignes OK (`phase-8/vm-t10-2026-10-21.txt`). |
| `EngineError::NotImplemented` supprimée, grep vide | ✅ | PR #78 ; `grep -rn "todo!\|NotImplemented\|TODO\|FIXME" crates app/src app/src-tauri/src` ne renvoie rien. |
| Test Minecraft Java | ✅ | F11 figé (plein écran exclusif, 1 image en 30 s, alerte levée), réglage NVIDIA sans effet, fenêtre agrandie et mod Borderless Fullscreen capturés (`phase-8/minecraft-*.txt`) ; conseil propre à Minecraft dans l'alerte et la FAQ. |
| Tag `v1.0.0` | ⏳ | Pas posé. |

## Reste à faire avant v1.0.0

1. **Release** : branche `release/v1.0.0` (version 1.0.0 dans `Cargo.toml`, `tauri.conf.json` et `package.json`, section `## [1.0.0]` du CHANGELOG), PR vers `main` puis tag `v1.0.0`, **seulement avec l'accord de Palawi** ; le workflow crée la release en brouillon, Palawi la publie.

Limites connues, non bloquantes au regard de la SPEC : AMF et QSV jamais essayés sur du vrai matériel (ENC-01) ; mise à l'échelle Windows essayée à 150 % seulement (CAP-08) ; BattlEye et Vanguard non essayés (CAP-10) ; aucun vrai cas positif d'`app_track_silent` provoqué (AUD-09) ; PERF-03 côté CPU mesuré avec 3 pistes au lieu de 4.
