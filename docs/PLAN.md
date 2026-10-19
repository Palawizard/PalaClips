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

## Phase 1 — Vidéo (CAP-01, 03, 05–08, 10 ; ENC-01–05) ✅ ([rapport](reports/phase-1.md))

- [x] Énumération écrans/fenêtres (`windows-capture`), CLI `list-targets`.
- [x] Session WGC → texture D3D11 → conversion NV12 sur GPU → encodeur matériel (FFmpeg
      `d3d11va`), sans copie CPU.
- [x] Pacer CFR sur horloge QPC.
- [x] Mise à l'échelle GPU, curseur, bordure, changement de résolution à chaud.
- [x] Détection des encodeurs par encodage de test ; message si aucun.
- [x] Repli fenêtre → écran (CAP-04).
- [x] CLI `capture-video --target <...> --fps 60 --seconds 10 --out clip.mp4`.

Critères :
- [x] 🤖 Le MP4 se lit ; `ffprobe` montre un fps constant (`r_frame_rate` = `avg_frame_rate`) et
  la bonne durée (±1 image).
- [x] 🤖 Profil : aucune lecture CPU de la texture (pas de `Map` de staging dans le chemin nominal).
- [x] 👤 Capture de Fortnite en fenêtre et en plein écran : image correcte, pas de saccade visible.
- [x] 👤 Écran HDR activé : couleurs correctes (CAP-09) — sinon implémenter la conversion et refaire le test (conversion implémentée, ADR-015).

## Phase 2 — Audio système + micro (AUD-01, 02, 08, 10, 11 moteur) ✅ ([rapport](reports/phase-2.md))

- [x] Loopback du périphérique de sortie (mix complet), micro, horodatage QPC.
- [x] Combleur de silences, gestion `AUDCLNT_BUFFERFLAGS_SILENT`, pas d'arrêt sur timeout.
- [x] Encodeurs AAC par piste, gestion du délai d'amorçage, titres des pistes.
- [x] Suivi du périphérique par défaut à chaud.
- [x] CLI `capture-av --seconds N --out f.mp4`.
- [x] Décalage de synchro par piste (AUD-11) dans la config et le moteur, options CLI.

Critères :
- 👤 Test du clap (`docs/TESTING.md` § T2) sur **1 h** : décalage < 1 image au début, au
  milieu et à la fin, sur les deux pistes.
- 👤 Silence de 2 min au milieu : synchro intacte.
- 👤 Débrancher/rebrancher le casque pendant la capture : pas de crash, la piste continue.

## Phase 3 — Pistes par logiciel (AUD-03, 05–07, 09) ✅ ([rapport](reports/phase-3.md))

- [x] `sources` : sessions audio actives + crêtes, processus, appli au premier plan.
- [x] Process loopback sur PID parent, `include_tree = true`, une piste par appli.
- [x] Rattachement automatique par nom d'exe (attente → silence → reprise).
- [x] Piste « Jeu » automatique.
- [x] Détection piste muette anormale (`app_track_silent`).
- [x] CLI `list-audio-sessions` et `capture-apps --app discord.exe --app chrome.exe ...`.

Critères :
- 👤 Test d'isolation (§ T3) : deux applis jouent deux sons différents → chacun sur sa piste,
  rien ailleurs (sauf piste 1).
- 👤 Test fait sur le Windows 11 à jour de Palawi : pistes non muettes (vérif `ffmpeg astats`).
- 👤 Fermer/relancer Discord pendant la capture : la piste se rattache, fichier valide.

## Phase 4 — Replay (RPL-02–05, 07, 08 ; STO-01, 03) ✅ ([rapport](reports/phase-4.md))

- [x] `Engine` / `EngineHandle` : orchestration des threads, fan-out des paquets.
- [x] `PacketRing` branché sur tous les flux ; stockage RAM et disque.
- [x] Sauvegarde : début sur image clé, alignement audio, écriture dans un thread dédié,
      nommage par jeu.
- [x] Redémarrage propre du buffer quand sources/zone changent.
- [x] CLI `replay --duration 60` (touche Entrée = sauvegarder, affiche RAM et latence).

Critères :
- 🤖 Sauvegarde < 500 ms (60 s, 40 Mb/s, 4 pistes) mesurée sur 20 sauvegardes.
- 🤖 Durée du clip = durée demandée ± 1 intervalle d'images clés ; pistes de même durée.
- 🤖 RAM stable (test accéléré, ADR-020) : 30 min armé, une sauvegarde toutes les 5 s ;
  10 premières minutes de mise en régime exclues ; sur 10–30 min, pente < 20 Mo/h et
  < 0,1 Mo par sauvegarde.
- 👤 Import dans DaVinci Resolve : pistes séparées et synchronisées (STO-01 ; noms non affichés
  par Resolve, ADR-021).

## Phase 5 — Application et sélecteurs (APP-01–05, 07, 13 ; CAP-02 ; AUD-04, 11 ; RPL-01, 03, 06) ✅ ([rapport](reports/phase-5.md))

Front : workflow `/design` obligatoire (impeccable + emil-design-eng).

- [x] Premier écran construit selon la direction de `.impeccable/surfaces/`, puis `DESIGN.md`
      (et `.impeccable/design.json`) produits par le documenteur impeccable, commités.
- [x] Shell Tauri : zone de notification + menu, instance unique, fenêtre créée/détruite,
      `prevent_exit`, raccourcis globaux personnalisables avec conflits.
- [x] Toasts sans vol de focus, son de confirmation.
- [x] Persistance des réglages (`settings.json` versionné) + migration + réinitialisation.
- [x] Écrans : État (accueil), Zone de capture (miniatures en direct), Sources audio
      (vumètres, recherche, appli non lancée, renommage), Replay & qualité (estimation RAM
      en direct), Raccourcis, Stockage, Général.
- [x] Contrat IPC complet pour ces écrans (`ipc.ts` à jour).
- [x] Assistant de calibrage de la synchro (AUD-11) dans Sources audio : émetteur flash + bip
      (logique de `probe-av-emitter`), capture de test, mesure automatique du décalage par
      piste, valeur proposée puis enregistrée ; relançable en un clic, car le retard d'une
      chaîne Voicemeeter/VST change à chaque lancement (`RESEARCH.md` § Audio).

Critères :
- 🤖 Fenêtre fermée : un seul processus PalaClips (aucun `msedgewebview2.exe` enfant).
- 🤖 `/impeccable audit` sans problème bloquant ; navigation clavier complète.
- 👤 Sauvegarde en plein écran dans Fortnite : le jeu garde le focus, le toast apparaît.
- 👤 Palawi valide le ressenti des sélecteurs (« propre et sympa »).

## Phase 6 — Enregistrement, galerie, lecteur (REC-01–06 ; STO-02, 04, 05 ; GAL-01–08) ✅ ([rapport](reports/phase-6.md))

- [x] Enregistrement manuel partageant l'encodeur, MP4 hybride, découpage, récupération.
      Fait : enregistrement (bouton, raccourci, zone de notification), MP4 hybride relisible
      après un arrêt brutal (test unitaire), replay au début, découpage, récupération au
      démarrage (REC-06, journal `recordings.txt`) ; découpage et arrêt brutal vérifiés sur la
      build (`scripts/recording-crash.ps1`, 2026-10-19) ; essai réel en jeu fait avec Palawi.
- [x] Disque plein pendant un enregistrement : arrêt propre, fichier lisible jusqu'au dernier
      fragment, alerte ; jamais un fichier cassé (STO-05, consigne de Palawi). Tests unitaires
      par injection d'erreur dans l'écriture, puis vrai test sur un petit disque virtuel
      (VHDX de quelques Go) créé et supprimé par un script, avec l'accord de Palawi. Jamais
      remplir le disque C:.
- [x] Quota (STO-04) appliqué : jamais un favori supprimé ; ce qui sera supprimé (les plus
      anciens) dit clairement, confirmation demandée la première fois. Suppression
      **définitive** (la Corbeille ne libère rien), dite clairement ; la suppression manuelle
      dans la galerie va à la Corbeille (GAL-05).
- [x] Bibliothèque : index SQLite, miniatures, surveillance du dossier, quota. Fait : index,
      synchro au démarrage, surveillance (`notify`), quota, favori / renommer / Corbeille /
      Explorateur, miniature des clips ajoutés hors de l'app.
- [x] Protocole `palaclips://` avec `Range` (plages de 4 Mo, chemins limités à la
      bibliothèque et au cache des pistes) ; clip de 4,26 Go : déplacement max 288 ms.
- [x] Lecteur : vidéo + pistes via Web Audio (mute/solo/volume), proxy H.264 si besoin.
      ADR-008 précisé : pistes extraites en `.m4a` par copie de flux dans un cache disque
      limité, lues en continu par des `<audio>` branchés sur Web Audio, recalées au-delà de
      ~40 ms ; jamais de piste entière en `AudioBuffer`. RAM stable sur 1 h à 6 pistes.
- [x] Découpe rapide / précise, export pistes choisies ou mixées, copier dans le presse-papiers.
      Fait côté moteur et commandes (tests : coupe rapide sur image clé, pistes choisies, mix,
      annulation ; coupe précise à l'image près sur NVENC, test `--ignored`) et UI (#45).
      Collage dans Discord vérifié avec Palawi, voix comprise (ADR-029).
- [x] Galerie fluide (virtualisation) avec 5 000 clips (`scripts/gallery-bench.ps1`).
- [x] Galerie : liste des pistes de chaque clip, noms lus dans le fichier (Resolve ne les
      montre pas, ADR-021).
- [x] Tout texte visible en clé i18n, français et anglais (ADR-028).

Critères :
- 🤖 Tuer le processus pendant un enregistrement : fichier lisible, finalisé au redémarrage
  (automatisé : `scripts/recording-crash.ps1`).
- 🤖 Clip de 4 Go : déplacement dans le lecteur < 300 ms.
- 🤖 5 000 clips factices (uniquement dans le dossier isolé : `PALACLIPS_CONFIG_DIR` + dossier
  de clips de test, supprimés à la fin) : défilement à 60 fps, ouverture de la galerie
  < 500 ms.
- 🤖 RAM du lecteur mesurée sur un enregistrement d'1 h (6 pistes), sans croissance avec la
  durée (ADR-008).
- 👤 Glisser un clip copié dans Discord : envoi OK, son = mix complet avec le micro (ADR-029).

## Phase 7 — Robustesse et performances (CAP-08, 09, 11 ; AUD-10 ; STO-05 ; APP-12 ; PERF-01–04)

- [ ] Changement de résolution, alt-tab, plein écran exclusif, casque débranché, appli qui
      crashe, disque plein, redémarrage du pilote GPU (TDR) : jamais de crash, alerte claire.
      Fait : reprise automatique après une panne passagère (`encoder_reset`, ADR-031), disque
      plein (phase 6), changement de résolution de la source (convertisseur recréé). Reste :
      scénarios T7 sur la machine de Palawi.
- [x] Alerte `capture_source_rate_limited` sur la cadence de composition DWM (CAP-11, ADR-018) :
      lue chaque seconde par le fil d'encodage, testée unitairement ; essai G-Sync réel dans T7.
- [x] Panneau de performances (APP-12) : section repliable sous le roster de l'écran État,
      mesures seulement panneau ouvert ; CPU machine, RAM hors tampon (calcul PERF-02), tampon,
      encodeur, images répétées (10 s) et en retard (total, dernière minute), décalage max.
- [ ] Mesures PERF sur build release installée. Fait le 2026-10-19 (`docs/reports/phase-7.md`) :
      PERF-01 4,9 Mo, PERF-02 67,3 Mo hors tampon, PERF-03 CPU 0,39 %, PERF-04 538 ms à chaud.
      Reste : perte de fps en jeu GPU-bound (PERF-03) avec Palawi.

Critères :
- 👤 Chaque scénario de `docs/TESTING.md` § T7 passé, consigné dans le rapport.
- 🤖/👤 PERF-01 à 04 mesurés et respectés (valeurs dans le rapport).
- 👤 PERF-03 (perte de fps ≤ 2 %) validé **GPU-bound**, sur l'essai « écran 1440p60 + 4 pistes »
  (configuration de la SPEC) : limite d'images du jeu levée ou scène
  lourde, GPU à ~99 % (`fps-impact.ps1`, GPU relevé dans le journal) ; sous le plafond de
  225 i/s, la marge masque le coût de la capture (`RESEARCH.md` § Capture vidéo).

## Phase 8 — Release V1 (APP-06, 08–11)

- [ ] Premier lancement guidé (APP-06), avec `/impeccable onboard` : micro et « Aussi dans Mix
      complet », « Audio du bureau », dossier des clips, replay, raccourcis, test de sauvegarde.
- [x] Socle i18n FR/EN fait avant la phase 6 (ADR-028) : clés typées, langue du système par
      défaut, choix dans Réglages > Général, codes + paramètres côté Rust, test contre les
      chaînes en dur.
- [ ] i18n FR/EN complète (aucune chaîne en dur) : relecture de l'anglais de toute l'app.
- [ ] Journaux + export de diagnostic.
- [x] Bundle NSIS avec DLL FFmpeg + licences (LGPL), installation par utilisateur (avancé en
      phase 7 pour les mesures PERF : 7 DLL à côté de l'exe, LGPL v3 + GPL v3 + notice tiers dans
      `licenses\`, page de licence, case de désinstallation qui dit ce qui part).
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
