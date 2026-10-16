# Décisions d'architecture (ADR)

> Une décision = un bloc. Pour changer une décision : nouveau bloc qui la remplace, l'ancienne
> passe en « Remplacée par ADR-XXX ». Ne jamais réécrire l'historique.

## ADR-001 — Rust, Windows uniquement
**Statut :** acceptée (2026-10-15)
- Contexte : outil qui tourne en permanence pendant les jeux ; il doit être léger et proche
  des API Windows (WGC, WASAPI, D3D11).
- Décision : Rust (edition 2024), cible `x86_64-pc-windows-msvc` uniquement.
- Conséquences : pas de Linux/macOS (hors périmètre, voir SPEC § 3).

## ADR-002 — Capture vidéo par Windows Graphics Capture (`windows-capture` 2.x)
**Statut :** acceptée
- Alternatives : DXGI Desktop Duplication (écran entier seulement, pas de capture de fenêtre
  masquée), injection/hook DirectX (interdit : anti-cheat).
- Décision : WGC via `windows-capture` (MIT, maintenu, 2.0.1 publiée le 2026-10-13). On utilise
  sa capture (texture D3D11 brute + horodatage QPC), **pas** son encodeur intégré.
- Conséquences : WGC n'émet que sur changement → pacer CFR maison. Bordure jaune désactivable
  (`DrawBorderSettings`).

## ADR-003 — Audio par logiciel via WASAPI process loopback (`wasapi` 0.26)
**Statut :** acceptée
- Décision : `AudioClient::new_application_loopback_client(pid, include_tree = true)` sur le
  PID **parent** ; loopback classique pour le mix complet ; capture standard pour le micro.
- Conséquences : Windows 10 build 20348 minimum. Le mode exclusion ne retire qu'un arbre de
  processus → pas de piste « tout le reste » propre, d'où la piste 1 = mix complet.

## ADR-004 — Encodage et mux via FFmpeg (`rsmpeg` 0.18) + build BtbN LGPL partagée
**Statut :** acceptée, confirmée en phase 0 (2026-10-15) avec un correctif de build
- Alternatives : `ffmpeg-next` (mode maintenance seulement — écarté), Media Foundation seul
  (pas de MP4 hybride, multi-pistes plus pénible), SDK NVENC direct (NVIDIA seulement).
- Décision : `rsmpeg` 0.18 (feature `ffmpeg8`, liaison dynamique via `FFMPEG_DLL_PATH`,
  bindings via `FFMPEG_INCLUDE_DIR` + libclang). DLL : BtbN `ffmpeg-n8.1-latest-win64-lgpl-shared`.
- Risque : rsmpeg 0.18 date d'août 2025 (FFmpeg 8.0) ; la build actuelle est 8.1.
- **Plan B**, dans l'ordre : (1) épingler une build BtbN 8.0 datée ; (2) utiliser directement
  `rusty_ffmpeg` 0.17 (feature `ffmpeg8_1`) avec une fine couche sûre écrite dans `encode`/`mux`.
- **Phase 0 (2026-10-15)** : rsmpeg 0.18 compile avec FFmpeg 8.1 (plan B inutile), mais son
  `rusty_ffmpeg 0.16` épingle bindgen 0.71, cassé avec LLVM ≥ 22. Choix de Palawi : copie de
  `rusty_ffmpeg 0.16.7` dans `third_party/rusty_ffmpeg`, seule modification bindgen `0.72.1`,
  branchée par `[patch.crates-io]` (MIT, `binding.rs` pré-généré retiré). Écartés : épingler
  LLVM 21 (fragile), plan B sans rsmpeg (plus d'unsafe à maintenir). À retirer dès qu'une
  version de rsmpeg dépend de `rusty_ffmpeg ≥ 0.17`.
- DLL au lancement : runner Cargo `scripts/with-ffmpeg.cmd` (PATH préfixé) pour `cargo run`,
  `cargo test` et `tauri dev` ; le bundle (phase 8) les place à côté de l'exécutable.
- Licence : liaison dynamique aux DLL LGPL, fichiers de licence livrés dans le bundle, lien vers
  les sources FFmpeg dans « À propos ».

## ADR-005 — UI : Tauri 2 (stable) + React, WebView détruit à la fermeture
**Statut :** acceptée
- Alternatives : egui (plus léger fenêtre ouverte mais pas de lecteur vidéo → galerie trop
  coûteuse à faire), Slint, Tauri 3 (alpha → écarté).
- Décision : Tauri 2.12+, fenêtre créée à la demande et détruite à la fermeture,
  `prevent_exit` pour rester en zone de notification.
- Conséquences : fenêtre ouverte = coût WebView2 (accepté) ; fenêtre fermée = un processus natif.

## ADR-006 — Formats : MP4 hybride pour l'enregistrement, MP4 classique pour le replay
**Statut :** acceptée
- MP4 fragmenté brut écarté (mal supporté par éditeurs/lecteurs, cf. OBS). MKV écarté (remux
  nécessaire pour Resolve).
- Enregistrement : `movflags +hybrid_fragmented` (FFmpeg, juin 2024). Replay : écrit d'un bloc
  depuis la RAM, MP4 classique.

## ADR-007 — Lecture des clips via un protocole `palaclips://` avec `Range`
**Statut :** acceptée
- Contexte : l'asset protocol de Tauri v1 plantait sur des vidéos de plusieurs Go (issue #6375).
- Décision : protocole personnalisé asynchrone (`register_asynchronous_uri_scheme_protocol`)
  qui sert les fichiers par plages. Testé en phase 6 sur 4 Go.

## ADR-008 — Écoute des pistes séparées dans le lecteur via Web Audio
**Statut :** acceptée
- Contexte : Chromium/WebView2 n'expose la sélection de piste audio (`audioTracks`) que derrière
  l'option « Experimental Web Platform features ».
- Décision : `<video muted>` + une source Web Audio par piste (fichiers extraits à la demande,
  mis en cache), synchronisés sur `currentTime`.

## ADR-009 — Index de la bibliothèque en SQLite (`rusqlite`, feature `bundled`)
**Statut :** acceptée
- Contexte : GAL-02 exige la fluidité avec 5 000 clips, filtres et tri.
- Décision : SQLite embarqué, le dossier de clips reste la source de vérité (réindexation
  possible à tout moment).

## ADR-010 — Front : React 19 + TypeScript + Tailwind 4, design via impeccable + emil-design-eng
**Statut :** acceptée
- Stack habituelle de Palawi. Composants accessibles sans style imposé (choix précis de la
  bibliothèque headless fait en phase 5 avec `/impeccable shape`, consigné ici).
- Tout le front passe par les skills `impeccable` et `emil-design-eng` (CLAUDE.md § 2).

## ADR-011 — Licence MIT
**Statut :** acceptée (modifiable par Palawi avant la première release publique)
- Compatible avec les DLL FFmpeg LGPL en liaison dynamique.

## ADR-012 — Mises à jour via GitHub Releases, sans signature Authenticode
**Statut :** acceptée
- `tauri-plugin-updater` avec signature Tauri (clé privée hors repo, en secret GitHub).
- Pas de certificat Authenticode (payant) : SmartScreen avertit au premier lancement,
  documenté dans le README.

## ADR-013 — Deux branches longues : `main` (versions) et `dev` (intégration)
**Statut :** acceptée (2026-10-15, PR #1)
- `feature/`, `fix/`, `chore/` partent de `dev` et y reviennent par PR ; `main` ne reçoit que
  des PR `dev` ou `release/vX.Y.Z` (versions publiées) ; `hotfix/` part de `main` et est
  mergée dans `main` et `dev`. Jamais de commit direct sur `main` ni `dev`.
- La CI tourne sur les PR et sur les pushs vers `main` et `dev`.

## ADR-014 — Direction visuelle « Écran d'après-match »
**Statut :** acceptée (2026-10-15, choix de Palawi sur le tirage impeccable `cabeab00`)
- Les pistes audio forment un roster présenté comme un tableau des scores ; une couleur
  mi-saturée par piste, langue fixe dans toute l'app ; rouge réservé à « ça enregistre » ;
  fond bleu-noir. Contrat détaillé : `.impeccable/surfaces/app-src-app-tsx.md`.
- Écartées : « Carte mémoire » (familière, risque rétro), « Console graphite » (générique),
  standard de la catégorie.
- Build code-led (pas de génération d'image) ; `DESIGN.md` produit avec le premier écran
  (phase 5).

## ADR-015 — Pipeline vidéo GPU : Video Processor D3D11 et pacer sur ticks QPC
**Statut :** acceptée (2026-10-15, phase 1) ; écrêtage des hautes lumières HDR remplacé par ADR-016
- Conversion BGRA → NV12, mise à l'échelle et bandes noires par `ID3D11VideoProcessor`, sur le
  périphérique de WGC, directement dans la frame matérielle de l'encodeur. Alternatives :
  shader maison (plus de code), entrée BGRA dans NVENC (NVIDIA seulement, pas d'échelle).
- Pacer piloté par un tick QPC (`t0 + n/fps`) qui encode la dernière image reçue : WGC n'émet
  que sur changement et son horodatage peut devancer le QPC de quelques ms (RESEARCH), donc pas
  de sélection par horodatage. PTS = `n` : CFR par construction, sans dérive.
- Pacer et encodeur dans le même fil (`palaclips-video`) : un encodage lent retarde le tick,
  qui émet ensuite les ticks manqués.
- Sortie de taille fixe pendant une session : un changement de taille de la source est
  absorbé par le Video Processor (bandes noires).
- Repli fenêtre → écran : si la session de fenêtre ne démarre pas ou n'envoie aucune image en
  3 s, alerte `capture_fallback_monitor`.
- Écran HDR (CAP-09) : capture en FP16 scRGB, division par le blanc SDR de l'écran,
  écrêtage à 1 et encodage sRGB par un pixel shader (compilé à l'exécution par
  `d3dcompiler_47`) vers une texture BGRA8, puis Video Processor comme en SDR. Les hautes
  lumières HDR (HDR automatique, vidéos HDR) sont écrêtées au blanc SDR : le rendu SDR reste
  fidèle à ce qu'affiche un écran SDR. Écarté : la conversion BGRA8 de Windows (délavée).
- Débit : préréglages Qualité / Équilibré / Léger = 0,30 / 0,18 / 0,08 bit par pixel et par
  image (borné à 1–200 Mb/s), ou débit libre ; CBR.

## ADR-016 — Hautes lumières HDR atténuées en douceur au lieu d'être écrêtées
**Statut :** acceptée (2026-10-15)
- Contexte : l'écrêtage au blanc SDR (ADR-015) transformait tout ce qui dépasse le blanc SDR
  (ciel en HDR automatique, soleil, explosions) en aplats blancs sans détail.
- Décision : dans le shader HDR → SDR, après division par le blanc SDR, on calcule
  `m = max(R, G, B)` ; si `m ≤ 0,9`, rien ne change ; au-dessus, les trois canaux sont
  multipliés par `f(m) / m` avec `f(m) = s + h·e / (e + h)` (`s = 0,9`, `h = 1 − s`,
  `e = m − s`). La courbe a une pente de 1 au seuil (pas de cassure visible) et tend vers 1
  sans jamais l'atteindre.
- Pourquoi sur `max(R, G, B)` et pas par canal : le facteur commun garde la teinte (une
  explosion reste orange, un ciel reste bleu) ; par canal, les couleurs vives virent au blanc.
- Pourquoi 0,9 : le contenu SDR (interface, jeu sous le blanc SDR) reste identique jusqu'à
  90 % du blanc ; un seuil plus bas garderait plus de détail dans les hautes lumières mais
  assombrirait le blanc des interfaces. Réglable par `ROLL_OFF_START` (`encode/hdr.rs`).
- Écarté : tone mapping complet (Reinhard ou ACES sur toute la plage), qui modifie aussi les
  tons moyens et donc le rendu des jeux SDR sur écran HDR.
- Vérification (2026-10-15, scène HDR10 de synthèse jouée par VLC sur l'écran 2, blanc SDR
  280 nits) : rampe de gris identique jusqu'à 250 nits (242/255), puis 248 → 254 de 280 à
  1500 nits au lieu de 255 dès 280 nits ; ciel, disque du soleil et couleur de l'explosion
  conservés au lieu d'aplats blancs (`docs/reports/phase-1.md`).

## ADR-017 — Décalage de synchro par piste (AUD-11)
**Statut :** acceptée (2026-10-15, décision de Palawi)
- Contexte : mesuré avec `probe-av-emitter` (flash et bip émis ensemble, sans compensation,
  comme un jeu) et `capture-av` : sortie par défaut directe (Apogee BOOM) → son +15,7 ms
  après l'image, constant (tampon de rendu de l'émetteur) ; sortie par défaut « Voicemeeter
  Input » → +103 à +136 ms. Le loopback du périphérique virtuel reçoit le son ~100 ms plus
  tard, et Windows ne l'expose pas (`GetStreamLatency` = 0 sur le loopback). Aucune dérive
  dans les deux cas : le pipeline QPC est juste.
- Décision : un décalage par piste en ms (−1000 à +1000, positif = son retardé, comme le
  « Sync Offset » d'OBS), ajouté aux horodatages QPC des paquets de la piste avant le
  combleur. À l'arrêt, la piste attend en plus `max(0, −décalage)` pour recevoir le son
  jusqu'à la fin. Valeurs dans `AudioSettings` (`full_mix_sync_offset_ms`,
  `microphone_sync_offset_ms`, `AppTrack::sync_offset_ms`).
- Assistant de calibrage en phase 5 (même principe que `probe-av-emitter` + mesure).
- Écarté : compensation automatique d'après la latence annoncée par Windows (nulle, donc
  inexploitable) ; capture d'un autre point de la chaîne Voicemeeter (sorties ASIO/KS hors
  du moteur audio de Windows).

## ADR-018 — Alerte de cadence limitée sur l'horloge de composition DWM (CAP-11)
**Statut :** acceptée (2026-10-16, décision de Palawi)
- Contexte : avec G-Sync et des écrans à fréquences différentes, DWM se cale sur un autre
  écran et WGC ne livre qu'une image par composition (Fortnite à 225 i/s → 165 i/s reçus,
  horloge DWM 164,9 Hz ; `RESEARCH.md` § Capture vidéo). La sortie CFR reste juste mais
  duplique des images, sans que l'utilisateur sache pourquoi. Aucun contournement sans
  injection.
- Décision : alerte `capture_source_rate_limited` (avertissement) quand la cadence de
  composition DWM (`DwmGetCompositionTimingInfo`, `rateCompose`, comme `probe-wgc`) est
  inférieure de plus de 2 % au fps demandé pendant 5 s ; une seule fois par capture, et de
  nouveau seulement si la cadence change. Message : « Windows ne compose ton écran qu'à
  N Hz alors que tu enregistres à M i/s : la vidéo répétera des images. Désactive G-Sync
  pour cet écran, ou passe tes autres écrans à au moins M Hz. »
- Écarté : déclenchement sur le nombre d'images reçues par WGC, qui alerterait à tort pour un
  jeu plafonné, une scène statique ou un menu.
- Code en phase 7.

## ADR-019 — La piste Jeu suit le jeu, pas le premier plan (AUD-06)
**Statut :** acceptée (2026-10-16, décision de Palawi) ; remplace la piste Jeu « appli au
premier plan » de la phase 3.
- Contexte : la piste Jeu suivait toute appli au premier plan. Un Alt+Tab vers Discord ou le
  navigateur pendant une partie rouvrait la piste sur cette appli : le son du jeu
  disparaissait de sa piste.
- Décision : une seule notion de « jeu en cours » (`sources::CurrentGame`), réutilisée pour
  la piste Jeu, le nom des fichiers (STO-03) et la galerie (GAL-01/02) :
  - processus fixé par l'utilisateur (sélecteur AUD-04) ;
  - sinon, capture d'une fenêtre → l'appli de cette fenêtre ;
  - sinon, la dernière appli passée au premier plan en plein écran ou sans bordure couvrant
    un écran (fenêtre qui recouvre tout son écran et n'a pas de barre de titre). Fenêtres
    normales ou agrandies, bureau, shell et PalaClips ne la changent pas.
  - Premier plan suivi par événement (`SetWinEventHook` : `EVENT_SYSTEM_FOREGROUND`, et
    `EVENT_OBJECT_LOCATIONCHANGE` de la fenêtre au premier plan pour un passage en plein
    écran sans changement de premier plan), plus de vérification toutes les 500 ms.
- Écarté : liste de jeux connus (jamais complète) ; détection par l'usage GPU (coûteuse,
  pas fiable) ; garder le premier plan avec une liste d'applis exclues (Discord, navigateurs…),
  jamais complète non plus.
- Limite : une vidéo en plein écran dans le navigateur (F11) compte comme un jeu.
