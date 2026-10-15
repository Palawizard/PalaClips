# PLAN — PalaClips V1

> Ordre de travail. Une phase n'est terminée que quand **tous** ses critères sont vérifiés et
> qu'un rapport `docs/reports/phase-N.md` existe (`/valider-phase`). Les identifiants
> (`CAP-01`...) renvoient à `docs/SPEC.md`. À la fin de la phase 8, **toutes** les exigences de
> la SPEC sont couvertes — rien n'est reporté.
>
> Légende des critères : 🤖 vérifiable par Claude Code (tests, CLI, mesure) · 👤 nécessite
> Palawi sur sa machine (jeu, Resolve, écoute) — protocole dans `docs/TESTING.md`.

## Phase 0 — Mise en route ✅ ([rapport](reports/phase-0.md))

But : un environnement qui compile, une CI verte, les outils de design prêts.

- [x] `pwsh scripts/check-env.ps1` tout vert (installer ce qui manque).
- [x] `pwsh scripts/fetch-ffmpeg.ps1` ; ajouter `rsmpeg = "0.18"` (cfg windows) à l'engine et
      prouver la liaison : sous-commande `palaclips-cli ffmpeg-info` qui liste les encodeurs
      matériels disponibles (`h264_nvenc`...). Si les bindings 8.0 de rsmpeg 0.18 ne compilent
      pas avec les en-têtes 8.1, appliquer le plan B de l'ADR-004 et le noter.
- [x] Icônes : déjà générées depuis `app-icon.svg` et commitées. Si le logo change :
      `cd app && npm run tauri icon src-tauri/icons/app-icon.svg`, supprimer `android/` et `ios/`.
- [x] `npm run tauri dev` ouvre la fenêtre et affiche « Moteur 0.1.0 ».
- [x] CI GitHub Actions verte (sur les PR vers `dev` : `main` ne reçoit que les versions).
- [x] Skills de design installés et `/impeccable init` lancé avec `docs/DESIGN-BRIEF.md`
      → `PRODUCT.md` commité, direction visuelle choisie (`.impeccable/surfaces/`). `DESIGN.md`
      est produit avec le premier écran (phase 5).

Critères :
- 🤖 `cargo clippy --workspace --all-targets -- -D warnings` et `cargo test --workspace` passent sur Windows.
- 🤖 `palaclips-cli ffmpeg-info` affiche au moins un encodeur matériel sur la machine de Palawi.
- 👤 Palawi voit la fenêtre de l'app.

## Phase 1 — Vidéo (CAP-01, 03, 05–08, 10 ; ENC-01–05)

- [ ] Énumération écrans/fenêtres (`windows-capture`), CLI `list-targets`.
- [ ] Session WGC → texture D3D11 → conversion NV12 sur GPU → encodeur matériel (FFmpeg
      `d3d11va`), sans copie CPU.
- [ ] Pacer CFR sur horloge QPC.
- [ ] Mise à l'échelle GPU, curseur, bordure, changement de résolution à chaud.
- [ ] Détection des encodeurs par encodage de test ; message si aucun.
- [ ] Repli fenêtre → écran (CAP-04).
- [ ] CLI `capture-video --target <...> --fps 60 --seconds 10 --out clip.mp4`.

Critères :
- 🤖 Le MP4 se lit ; `ffprobe` montre un fps constant (`r_frame_rate` = `avg_frame_rate`) et
  la bonne durée (±1 image).
- 🤖 Profil : aucune lecture CPU de la texture (pas de `Map` de staging dans le chemin nominal).
- 👤 Capture de Fortnite en fenêtre et en plein écran : image correcte, pas de saccade visible.
- 👤 Écran HDR activé : couleurs correctes (CAP-09) — sinon implémenter la conversion et refaire le test.

## Phase 2 — Audio système + micro (AUD-01, 02, 08, 10)

- [ ] Loopback du périphérique de sortie (mix complet), micro, horodatage QPC.
- [ ] Combleur de silences, gestion `AUDCLNT_BUFFERFLAGS_SILENT`, pas d'arrêt sur timeout.
- [ ] Encodeurs AAC par piste, gestion du délai d'amorçage, titres des pistes.
- [ ] Suivi du périphérique par défaut à chaud.
- [ ] CLI `capture-av --seconds N --out f.mp4`.

Critères :
- 👤 Test du clap (`docs/TESTING.md` § T2) sur **1 h** : décalage < 1 image au début, au
  milieu et à la fin, sur les deux pistes.
- 👤 Silence de 2 min au milieu : synchro intacte.
- 👤 Débrancher/rebrancher le casque pendant la capture : pas de crash, la piste continue.

## Phase 3 — Pistes par logiciel (AUD-03, 05–07, 09)

- [ ] `sources` : sessions audio actives + crêtes, processus, appli au premier plan.
- [ ] Process loopback sur PID parent, `include_tree = true`, une piste par appli.
- [ ] Rattachement automatique par nom d'exe (attente → silence → reprise).
- [ ] Piste « Jeu » automatique.
- [ ] Détection piste muette anormale (`app_track_silent`).
- [ ] CLI `list-audio-sessions` et `capture-apps --app discord.exe --app chrome.exe ...`.

Critères :
- 👤 Test d'isolation (§ T3) : deux applis jouent deux sons différents → chacun sur sa piste,
  rien ailleurs (sauf piste 1).
- 👤 Test fait sur le Windows 11 à jour de Palawi : pistes non muettes (vérif `ffmpeg astats`).
- 👤 Fermer/relancer Discord pendant la capture : la piste se rattache, fichier valide.

## Phase 4 — Replay (RPL-02–05, 07, 08 ; STO-01, 03)

- [ ] `Engine` / `EngineHandle` : orchestration des threads, fan-out des paquets.
- [ ] `PacketRing` branché sur tous les flux ; stockage RAM et disque.
- [ ] Sauvegarde : début sur image clé, alignement audio, écriture dans un thread dédié,
      nommage par jeu.
- [ ] Redémarrage propre du buffer quand sources/zone changent.
- [ ] CLI `replay --duration 60` (touche Entrée = sauvegarder, affiche RAM et latence).

Critères :
- 🤖 Sauvegarde < 500 ms (60 s, 40 Mb/s, 4 pistes) mesurée sur 20 sauvegardes.
- 🤖 Durée du clip = durée demandée ± 1 intervalle d'images clés ; pistes de même durée.
- 🤖 2 h armé : RAM stable (écart < 5 % entre 30 min et 2 h).
- 👤 Import dans DaVinci Resolve : pistes séparées, nommées, synchronisées (STO-01).

## Phase 5 — Application et sélecteurs (APP-01–05, 07, 13 ; CAP-02 ; AUD-04 ; RPL-01, 03, 06)

Front : workflow `/design` obligatoire (impeccable + emil-design-eng).

- [ ] Premier écran construit selon la direction de `.impeccable/surfaces/`, puis `DESIGN.md`
      (et `.impeccable/design.json`) produits par le documenteur impeccable, commités.
- [ ] Shell Tauri : zone de notification + menu, instance unique, fenêtre créée/détruite,
      `prevent_exit`, raccourcis globaux personnalisables avec conflits.
- [ ] Toasts sans vol de focus, son de confirmation.
- [ ] Persistance des réglages (`settings.json` versionné) + migration + réinitialisation.
- [ ] Écrans : État (accueil), Zone de capture (miniatures en direct), Sources audio
      (vumètres, recherche, appli non lancée, renommage), Replay & qualité (estimation RAM
      en direct), Raccourcis, Stockage, Général.
- [ ] Contrat IPC complet pour ces écrans (`ipc.ts` à jour).

Critères :
- 🤖 Fenêtre fermée : un seul processus PalaClips (aucun `msedgewebview2.exe` enfant).
- 🤖 `/impeccable audit` sans problème bloquant ; navigation clavier complète.
- 👤 Sauvegarde en plein écran dans Fortnite : le jeu garde le focus, le toast apparaît.
- 👤 Palawi valide le ressenti des sélecteurs (« propre et sympa »).

## Phase 6 — Enregistrement, galerie, lecteur (REC-01–06 ; STO-02, 04, 05 ; GAL-01–08)

- [ ] Enregistrement manuel partageant l'encodeur, MP4 hybride, découpage, récupération.
- [ ] Bibliothèque : index SQLite, miniatures, surveillance du dossier, quota.
- [ ] Protocole `palaclips://` avec `Range`.
- [ ] Lecteur : vidéo + pistes via Web Audio (mute/solo/volume), proxy H.264 si besoin.
- [ ] Découpe rapide / précise, export pistes choisies ou mixées, copier dans le presse-papiers.
- [ ] Galerie fluide (virtualisation) avec 5 000 clips.

Critères :
- 👤 Tuer le processus pendant un enregistrement : fichier lisible, finalisé au redémarrage.
- 🤖 Clip de 4 Go : déplacement dans le lecteur < 300 ms.
- 🤖 5 000 clips factices : défilement à 60 fps, ouverture de la galerie < 500 ms.
- 👤 Glisser un clip copié dans Discord : envoi OK, son = mix.

## Phase 7 — Robustesse et performances (CAP-08, 09 ; AUD-10 ; STO-05 ; APP-12 ; PERF-01–04)

- [ ] Changement de résolution, alt-tab, plein écran exclusif, casque débranché, appli qui
      crashe, disque plein, redémarrage du pilote GPU (TDR) : jamais de crash, alerte claire.
- [ ] Panneau de performances.
- [ ] Mesures PERF sur build release installée.

Critères :
- 👤 Chaque scénario de `docs/TESTING.md` § T7 passé, consigné dans le rapport.
- 🤖/👤 PERF-01 à 04 mesurés et respectés (valeurs dans le rapport).

## Phase 8 — Release V1 (APP-06, 08–11)

- [ ] Premier lancement guidé.
- [ ] i18n FR/EN complète (aucune chaîne en dur).
- [ ] Journaux + export de diagnostic.
- [ ] Bundle NSIS avec DLL FFmpeg + licences (LGPL), installation par utilisateur.
- [ ] Updater (clé de signature Tauri générée par Palawi, jamais commitée), workflow de release
      GitHub Actions (`tauri-action`) qui publie l'installeur et `latest.json`.
- [ ] README utilisateur (installation, SmartScreen, FAQ), CHANGELOG, captures d'écran.
- [ ] Revue finale : chaque exigence de la SPEC cochée dans `docs/reports/release-v1.md`
      avec la preuve (rapport de phase, test, capture).

Critères :
- 👤 Installation sur une session Windows propre : premier lancement guidé → clip sauvegardé
  en moins de 2 minutes.
- 🤖 Mise à jour 1.0.0 → 1.0.1 de test via l'updater.
- 🤖 La variante `EngineError::NotImplemented` est supprimée, et
  `grep -rn "todo!\|NotImplemented\|TODO\|FIXME" crates app/src app/src-tauri/src` ne renvoie rien.
- Tag `v1.0.0`.
