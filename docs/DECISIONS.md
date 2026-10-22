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
- Conséquences : build 20348 minimum, soit Windows 11 ou Windows Server 2022 (Windows 10
  grand public s'arrête au build 19045). Le mode exclusion ne retire qu'un arbre de
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
- Précision (2026-10-18, consigne de Palawi) : **jamais** une piste entière décodée en
  `AudioBuffer` (1 h × 6 pistes ≈ 8 Go en float 32 bits). Chaque piste est extraite en `.m4a`
  par copie de flux (sans réencodage), à la demande, dans un cache disque limité en taille et
  nettoyé (les plus anciens d'abord, et au démarrage). Lecture en continu par un `<audio>` par
  piste branché sur Web Audio (`MediaElementAudioSourceNode` → gain, mute, solo), recalé sur la
  vidéo quand l'écart dépasse ~40 ms. La RAM du lecteur est mesurée sur un enregistrement
  d'1 h (rapport de la phase 6).

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
**Statut :** acceptée (2026-10-15, décision de Palawi) ; référence de mesure corrigée par
ADR-027.
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
- **Mise en œuvre (2026-10-19).** Le champ lu est `rateRefresh` de
  `DwmGetCompositionTimingInfo(NULL)`, celui que `probe-wgc` affiche et qui a donné les
  164,9 Hz de la mesure (le texte ci-dessus disait `rateCompose` par erreur). Le fil
  d'encodage vidéo le lit une fois par seconde (`capture::RateWatch`) ; « cadence qui change »
  = valeur arrondie au Hz différente de la dernière alerte.
- **Révision du 2026-10-19 (revue de Palawi).** Écran immobile : Windows 11 peut baisser la
  fréquence quand rien ne change (fréquence d'actualisation dynamique). Seules les secondes où
  WGC livre de nouvelles images comptent pour les 5 s ; une période sans image ne compte pas et
  ne remet pas le compteur à zéro. Mesure sur la machine de Palawi : fenêtre immobile et
  fenêtre animée capturées à 240 i/s, DWM reste à 240 Hz, aucune alerte (la baisse vient donc
  d'autres machines, couverte par le test `a_still_screen_never_alerts_even_when_windows_slows_its_refresh`).

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

## ADR-020 — Mémoire du replay vérifiée par un test accéléré (RPL-08)
**Statut :** acceptée (2026-10-16, décision de Palawi) ; remplace le test de 2 h armé.
- Contexte : RPL-08 demandait une RAM stable sur 2 h de replay armé, mesurée entre 30 min et
  2 h. Un tel test ne voit presque rien : sans sauvegarde, le tampon est purgé à taille
  constante et rien d'autre n'alloue. Pendant 20 sauvegardes, la mémoire privée est montée de
  ~1 Mo par sauvegarde (612 → 633 Mo) : c'est la sauvegarde qui peut fuir, pas le temps.
- Décision : RPL-08 devient « pas de croissance avec le temps ni avec le nombre de
  sauvegardes ». Test : 20 min armé (60 s, 40 Mb/s, 4 pistes), une sauvegarde toutes les
  5 s (~240), mémoire privée relevée à chaque sauvegarde ; après la première minute de
  tampon plein, pente < 0,1 Mo par sauvegarde et < 20 Mo/h (régression linéaire).
- Pourquoi : 240 sauvegardes couvrent bien plus d'usage réel qu'une soirée de jeu ; une fuite
  par sauvegarde de 0,1 Mo se voit (24 Mo sur le test) alors qu'un test de 2 h sans sauvegarde
  ne la verrait jamais. Une fuite lente avec le temps sous 20 Mo/h reste sans effet réel
  (< 100 Mo sur une soirée de 5 h).
- Limite : à cadence fixe, nombre de sauvegardes et temps avancent ensemble ; les deux pentes
  viennent des mêmes mesures (20 Mo/h ≈ 0,028 Mo par sauvegarde à une toutes les 5 s), le
  seuil horaire est donc le plus strict.
- Écarté : test de 2 h armé sans sauvegarde (long, sans surveillance, peu informatif).
- **Correction du 2026-10-16 (décision de Palawi)** : le premier essai (20 min) a donné
  +0,045 Mo par sauvegarde mais +32 Mo/h, avec une montée de 626 à 642 Mo jusqu'à 9 min puis
  un palier plat de 10 à 20 min. Le critère incluait la mise en régime du tas : c'était une
  erreur de conception du test, pas un échec du code. Critère corrigé, fixé avant de relancer :
  30 min armé, une sauvegarde toutes les 5 s ; les 10 premières minutes (mise en régime) sont
  exclues ; sur 10–30 min, pente dans le temps < 20 Mo/h et pente par sauvegarde < 0,1 Mo ;
  toutes les mesures gardées dans le journal. La cause des ~16 Mo de mise en régime n'est
  cherchée que si le nouveau test montre encore une pente.

## ADR-021 — Noms de pistes : métadonnées du fichier seulement (AUD-05)
**Statut :** acceptée (2026-10-16, décision de Palawi).
- Contexte : AUD-05 demandait que le nom de chaque piste soit visible dans Resolve. Le MP4 le
  porte aux deux endroits standard (`hdlr` et `udta/name` de chaque piste), mais Resolve
  affiche « Audio 1 » à « Audio N » partout, en MP4 comme en MOV (`RESEARCH.md`).
- Décision : les noms restent écrits dans le fichier (VLC, ffprobe, MPC les lisent) ; Resolve
  n'est pas tenu de les afficher. L'ordre fixe des pistes (mix, micro, applis, Jeu,
  `ARCHITECTURE.md` § 4) et l'écoute suffisent à les reconnaître en quelques secondes.
- Écarté : fichier de timeline OpenTimelineIO à côté de chaque clip (Resolve en garde les noms
  de pistes, d'après Palawi) et export d'un WAV par piste : du travail et des fichiers en plus
  pour un confort que Palawi juge inutile.

## ADR-022 — Réglages NVENC pour la mémoire (PERF-02)
**Statut :** acceptée (2026-10-16) ; le seuil de PERF-02 reste à trancher (`HANDOFF.md`).
- Contexte : avec les réglages par défaut de FFmpeg, une session `*_nvenc` 1440p engage
  ~175 Mo de mémoire système (DPB auto, 4 surfaces, `delay` 3) : la moitié du moteur hors
  tampon (`RESEARCH.md` § Mémoire du pipeline).
- Décision : `dpb_size=2` pour tous les encodeurs NVENC ; `surfaces=2` et `delay=1` jusqu'à
  60 i/s, défauts de FFmpeg (`surfaces=4`, `delay=3`) au-delà (`nvenc_pipeline_depth`).
- Pourquoi : −110 Mo engagés pour −0,3 dB de PSNR à 40 Mb/s ; le pipeline court tient à
  60 i/s (retard max 3,7 ms) mais pas à 240 i/s (jusqu'à 15,8 ms), d'où le seuil.
- Écarté : `dpb_size=1` (−7 Mo de plus pour −0,8 dB) ; `tune=ll`/`ull` (aucun effet mémoire).
- Limite : 120 et 144 i/s prennent les défauts sans mesure du pipeline court ; AMF et QSV ne
  sont pas réglés (pas de matériel pour mesurer).
- **Vérifié sur Fortnite (2026-10-16, demande de Palawi)** : 30 s de partie capturées en
  1440p60 à 150 Mb/s, réencodées à 40 Mb/s avec les deux réglages (`scripts/nvenc-quality.ps1`) :
  VMAF 99,37 contre 99,37, PSNR 40,43 contre 40,44 dB, SSIM 0,97112 contre 0,97115. Pendant la
  capture, jeu en charge : retard max d'encodage 0,79 ms, 1800 images sur 1800. Pas de perte
  visible ni de retard : réglages gardés.

## ADR-023 — RAM des critères PERF : mémoire privée réellement occupée
**Statut :** acceptée (2026-10-16, décision de Palawi) ; lève le bloquant PERF-02.
- Contexte : PERF-02 (moteur hors tampon < 150 Mo) mesuré en mémoire privée engagée
  (`PrivateUsage`) donnait 172 Mo hors tampon après ADR-022, dont ~136 Mo pour le pilote
  NVIDIA : il engage en mémoire système une réserve de la taille de chaque texture et des
  tampons de la session NVENC, sans jamais y toucher (10 textures 1440p : +55 Mo engagés,
  +1 Mo de working set ; `RESEARCH.md` § Mémoire du pipeline). Aucun réglage de notre côté ne
  passe sous 150 Mo dans cette mesure.
- Décision : PERF-01 et PERF-02 se mesurent sur la mémoire privée réellement occupée
  (*private working set*, colonne « Mémoire » du Gestionnaire des tâches), hors pression
  mémoire. Le rapport garde la mémoire engagée pour information.
- Pourquoi : c'est la RAM que PalaClips retire vraiment aux jeux et que l'utilisateur voit ;
  les réserves engagées du pilote ne coûtent que de l'espace dans le fichier d'échange.
  Mesuré : 102 Mo pour le replay de 5 s, 61 Mo sans son tampon.
- Limite : sous forte pression mémoire, Windows retire des pages du working set ; la mesure
  n'a de sens que machine non saturée (`TESTING.md` § PERF). Une fuite qui engage sans
  toucher ne se verrait pas : RPL-08 (ADR-020) reste mesuré en mémoire engagée.
- Écarté : relever le seuil en mémoire engagée (~200 Mo) ; `dpb_size=1` (−7 Mo seulement).

## ADR-024 — Nom du jeu : métadonnées de l'exe, figé par exe (STO-02, STO-03)
**Statut :** acceptée (2026-10-17, décision de Palawi).
- Contexte : les clips étaient rangés sous le nom d'exe (`FortniteClient-Win64-Shipping\`).
  L'exe de Fortnite n'a ni `ProductName` ni `FileDescription` (constaté sur la machine de
  Palawi), comme beaucoup de jeux Unreal.
- Décision : nom = `ProductName` de l'exe, sinon `FileDescription`, sinon nom d'exe nettoyé
  (suffixes de build Unreal `-Win64-Shipping`, `-WinGDK-Shipping`, `_EAC_EOS`…, puis
  « Client » final) ; les noms génériques sont ignorés (vide, « Unreal Engine », « UE4 »,
  « Shipping », noms de moteurs, « Launcher »…) ; nettoyage pour le système de fichiers (™,
  ®, ©, `:` et caractères interdits, entités XML décodées). Le nom est **figé par exe** à la
  première sauvegarde (`storage.game_names`, clé = exe en minuscules, `settings.json`), jamais
  recalculé ; modifiable dans Réglages > Stockage ; même nom pour le dossier, le fichier et la
  galerie (`engine::games`).
- Prudence : si le processus n'est plus là au moment de la sauvegarde (métadonnées
  illisibles), le nom d'exe nettoyé sert pour ce clip **sans être figé**.
- Écarté : lire les manifestes des lanceurs (Epic `.item`, Steam `appmanifest`) : plus de code
  et de cas pour un gain que le nettoyage du nom d'exe couvre (choix de Palawi).

## ADR-025 — Pas de doublon entre deux clips (RPL-09)
**Statut :** acceptée (2026-10-17, demande de Palawi).
- Contexte : chaque sauvegarde reprenait toute la durée du replay, même si un clip venait
  d'être sauvegardé : deux sauvegardes à 10 s d'écart répétaient 50 s.
- Décision : après une sauvegarde, le tampon oublie tout ce qui précède la fin du clip, en
  gardant la dernière image clé avant cette fin (`replay::forget_saved`). Le clip suivant
  commence donc là où le précédent s'est arrêté, sans rien perdre ; il peut en reprendre au
  plus un intervalle d'images clés (≤ 1 s par défaut), puisqu'un clip doit commencer sur une
  image clé. « En mémoire » repart de zéro et la RAM du tampon est libérée.
- Vérifié : deux sauvegardes à 10 s d'écart → 20,2 s puis 10,2 s (`replay`, CLI).
- **Complément (2026-10-17, Palawi)** : un double appui sur `Alt+F10` créait un clip d'~1 s.
  Moins de 3 s à sauvegarder (`replay::MIN_CLIP`, tampon vide compris) : pas de clip,
  alerte d'information `clip_too_short` dans l'app, sans notification ni carillon ; elle
  s'efface au clip suivant.
- Écarté : commencer à la première image clé après la fin du clip précédent (jusqu'à 1 s
  perdue entre les deux clips) ; garder la durée complète à chaque fois (comportement de
  ShadowPlay, refusé par Palawi).

## ADR-026 — Assistant de calibrage de la synchro (AUD-11)
**Statut :** acceptée (2026-10-17, défauts annoncés à Palawi) ; calcul des propositions
remplacé par ADR-027.
- Contexte : il faut une valeur proposée pour chaque piste, sans Python ni FFmpeg en ligne de
  commande chez l'utilisateur, en moins de 30 s.
- Décision : le moteur arrête le replay et lance une capture temporaire de l'écran principal
  (H.264 360p 60 i/s, sans les pistes d'applis, piste Jeu épinglée sur PalaClips) ; une
  fenêtre plein écran sans activation (`WS_EX_NOACTIVATE`) flashe en blanc avec un bip de
  50 ms chaque seconde pendant 10 s, sur la sortie par défaut comme un jeu ; le tampon est
  écrit dans un MP4 temporaire, relu (luminance moyenne 64×36, échantillons par piste),
  supprimé, puis le replay est réarmé s'il l'était. Mesure par piste : médiane des écarts
  flash → bip (≥ 5 paires, dispersion ≤ 20 ms), décalage proposé = décalage actuel −
  retard mesuré (`engine::calibrate`).
- Les pistes d'applis n'entendent pas le bip (il sort du processus PalaClips) : elles
  reçoivent la mesure de la piste Jeu, qui passe par la même capture par processus. Le micro
  n'est mesuré que s'il entend les bips (haut-parleurs) ; sinon « non mesuré ».
- L'appli prévient avant (écran principal, bips, replay redémarré) et rappelle que le retard
  d'une chaîne Voicemeeter/LightHost change à chaque relance ; « Recalibrer » reste visible.
- Écarté : mesurer dans le replay en cours (la cible peut être une fenêtre ou un autre écran)
  ; jouer le bip dans chaque appli (impossible sans injection).

## ADR-027 — Calibrage : la référence est la capture par processus de l'émetteur
**Statut :** acceptée (2026-10-17, décision de Palawi) ; corrige ADR-017 et ADR-026 sur ce
point.
- Contexte : ADR-017 et le premier assistant visaient « son pile avec l'image » (proposition
  = − retard mesuré). Le premier calibrage sur la machine de Palawi a mesuré +33,4 ms sur la
  piste Jeu, capturée par processus au plus près de l'émetteur : c'est le tampon de rendu de
  l'émetteur lui-même (comme les +15,7 ms de l'Apogee en ADR-017), pas un défaut de la
  chaîne. Un jeu a son propre tampon, différent ; le compenser décalerait tous les clips.
- Décision : la référence est la mesure de la piste Jeu (capture par processus de
  l'émetteur), ramenée à un décalage nul : `référence = mesure Jeu − décalage Jeu actuel`.
  Proposition pour une piste = `décalage actuel + référence − mesure de la piste`, bornée à
  ±1000 ms (`calibrate::proposed_offset_ms`). Jeu et applis (même capture par processus) :
  0 ms. Mix : seul l'excédent de la chaîne est compensé (Palawi : −(138 − 33) ≈ −105 ms).
  Micro : même règle s'il est mesuré. Sans référence mesurée, rien n'est proposé.
- Micro au casque : l'assistant demande de coller un écouteur contre le micro (comme le T2)
  avant de démarrer, et le rappelle dans le résultat s'il n'a rien entendu.
- Tests : `a_track_is_aligned_on_the_reference_not_on_zero`,
  `the_emitter_buffer_measured_by_the_game_track_is_not_compensated`,
  `earlier_wrong_offsets_are_corrected_from_their_current_value`,
  `nothing_is_proposed_without_a_reference`.

## ADR-028 — Socle i18n : clés côté front, codes + paramètres côté Rust (APP-08)
**Statut :** acceptée (2026-10-17, demande de Palawi : avant la phase 6, sans attendre la
phase 8).
- Contexte : APP-08 (français et anglais, langue du système par défaut) était prévu en phase 8.
  Le faire après la galerie aurait obligé à repasser sur toutes les chaînes de la phase 6.
- Front : `app/src/locales/fr.json` et `en.json`, module maison `app/src/lib/i18n.ts` sans
  dépendance (une centaine de lignes) : `t(clé, paramètres)` avec des clés typées depuis
  `fr.json` (une clé inconnue ne compile pas ; `en.json` doit avoir la même forme), pluriels
  `{ one, other }` choisis par `Intl.PluralRules`, `{nom}` remplacé par le paramètre,
  `useT()` (re-rendu au changement de langue par `useSyncExternalStore`). Nombres, dates et
  tris suivent la langue (`Intl`, `localeCompare`). Écartés : i18next + react-i18next (lourds
  pour deux langues, typage des clés à configurer), Lingui (macros et étape de compilation).
- Langue : réglage `language` (`system` | `fr` | `en`) dans `settings.json`, choisi dans
  Réglages > Général. `system` = langue d'affichage de Windows (`GetUserDefaultUILanguage`) :
  français si la langue principale est le français, anglais sinon. Le shell la résout
  (`get_language` renvoie le réglage et la langue retenue) ; le front attend au plus 250 ms au
  démarrage, comme pour le thème.
- Moteur et shell : tout texte affiché part en `Localized { code, params }`
  (`palaclips_engine::events::Localized`). Les alertes portent `code` + `params` en plus du
  `message` français (journaux, CLI) ; les erreurs des commandes Tauri sont des `Localized` ;
  `EngineError::localized()` donne le code de chaque erreur (les variantes utiles à
  l'utilisateur sont structurées : cible introuvable, sortie refusée, piste arrêtée…). Un
  paramètre peut être un autre `Localized` (raison d'un échec), traduit à son tour. Le texte
  est produit côté front sous `messages.<code>`. Seul le détail technique d'une erreur
  système (`{detail}` : message de Windows ou de FFmpeg) reste tel quel.
- Le shell lit les mêmes fichiers (`include_str!`) pour ce qu'il affiche sans WebView : menu de
  la zone de notification (reconstruit au changement de langue), notifications, titres des
  sélecteurs de fichier.
- Noms par défaut des pistes (Mix complet / Full mix, Micro / Mic, Jeu / Game), corrigé le
  2026-10-18 à la demande de Palawi : `settings.json` ne garde que les noms renommés ; un nom
  par défaut vaut `null`, s'affiche dans la langue courante et s'écrit dans le fichier dans la
  langue courante au moment de la sauvegarde (le shell passe les noms de la langue au moteur,
  `AudioSettings::default_names`, jamais enregistré ; les titres des pistes sont relus à chaque
  clip). Renommer une piste ou changer de langue ne redémarre plus le replay
  (`AudioSettings::same_sources`, événement `tracks_renamed`). Taper le nom par défaut de la
  langue courante le remet à `null`. Migration (format 2) : un nom identique au défaut français
  ou anglais redevient `null`. Le dossier des clips hors jeu garde le nom « Bureau » sur le
  disque.
- Garde-fou : `npm test` (`app/tests/i18n.test.mjs`, CI) échoue si un texte visible est écrit
  en dur dans `app/src` (texte JSX, attributs `aria-label`/`title`/`placeholder`/`alt`, props
  `label`/`help`/`error`/`unit`…, toute chaîne avec une lettre accentuée), si `fr.json` et
  `en.json` n'ont pas les mêmes clés et paramètres, ou si un code `Localized::new("…")` du
  Rust n'a pas de texte dans les deux langues.
- Conséquence : à partir de la phase 6, tout texte visible s'écrit directement en clé.

## ADR-029 — Le micro est aussi dans la piste 1 (AUD-01)
**Statut :** acceptée (2026-10-19, demande de Palawi après l'essai Discord de la phase 6).
- Contexte : la piste 1 ne contenait que la sortie audio (« tout ce que l'on entend »). Un clip
  collé dans Discord, qui ne lit que la piste 1, n'avait donc pas la voix du joueur.
- Décision : comme la piste 1 d'OBS envoyée au stream, la piste 1 contient tout, micro compris.
  Les pistes 2 et suivantes restent séparées pour le montage (dans Resolve, on coupe la piste 1
  et on monte les autres). Réglage `audio.microphone_in_full_mix` (Réglages > Audio, ligne
  Micro : « Aussi dans Mix complet »), activé par défaut ; il sera proposé par le premier
  lancement guidé (APP-06, phase 8).
- Mixage dans le moteur, échantillon par échantillon (`audio::MixBus`), après le comblement des
  trous : les deux pistes partagent l'origine QPC et l'index d'échantillon, donc le décalage
  de synchro du micro (AUD-11) s'applique aussi dans la piste 1. Somme bornée à ±1, sans
  limiteur. Le micro ne retient jamais la piste 1 plus de 200 ms. Le calibrage désactive le
  mixage.
- Écartés : mixer seulement à la copie (le clip lu ailleurs que par Ctrl+C restait sans
  voix) ; une piste 1 « mix + micro » en plus des autres (une piste de plus dans chaque
  fichier, et Resolve affiche déjà des pistes sans nom, ADR-021).
- **Révision du 2026-10-19 (revue de Palawi, `fix/full-mix-sum`).** Attente : le plafond fixe
  de 200 ms coupait une piste dont le décalage négatif est plus grand (micro de Palawi à
  ~−410/−470 ms une fois calibré : sa voix arrive ~450 ms après le reste). La piste 1 attend
  maintenant `max(200 ms, plus grand décalage négatif des pistes ajoutées + 150 ms)`, borné à
  1 s (plage AUD-11 : ±1 000 ms) ; le clip se termine là où toutes les pistes ont des données
  (`plan_clip`), comme pour une piste décalée seule. Saturation : la somme passe par un
  limiteur doux sur la piste 1 seulement (anticipation 3 ms, plafond −1 dBFS, retour 80 ms,
  gain identique sur les deux canaux) ; en dessous du plafond le signal passe inchangé. Tests :
  micro à −450 ms entièrement mixé (attente 600 ms), deux sinus à −3 dBFS → crête ≤ −1 dBFS,
  aucun écrêtage. Vérification réelle (app isolée, micro à −450 ms, piste d'appli à 300 Hz) :
  micro −36,2 dB RMS sur sa piste, −36,2 dB dans la piste 1, résidu après soustraction du
  micro aligné −76,3 dB (−33,2 dB s'il était décalé de 450 ms).

## ADR-030 — Mix complet = somme des pistes choisies (AUD-01)
**Statut :** acceptée (2026-10-19, demande de Palawi pendant la validation de la phase 6).
- Contexte : la piste 1 capturait toute la sortie audio (notifications, musique, navigateur…)
  en plus des pistes choisies. Pour Palawi, « Mix complet » doit être l'ensemble des pistes
  réellement enregistrées et choisies.
- Décision : par défaut, la piste 1 est la somme des pistes enregistrées (micro si « Aussi dans
  Mix complet », Jeu, applis), sans capture propre. Réglage « Audio du bureau »
  (`desktop_in_full_mix`, désactivé par défaut) : la piste 1 capture toute la sortie et n'ajoute
  que le micro, sans les pistes Jeu et applis (déjà dans la sortie, elles seraient doublées).
  Le premier lancement guidé (APP-06) proposera ce choix.
- Conséquence assumée : sans jeu détecté ni appli choisie, la piste 1 ne contient que le micro.
- Même mécanisme que ADR-029 (`MixBus`, une file par piste ajoutée), généralisé à plusieurs
  pistes : la piste la plus lente fixe le rythme, dans la limite de l'attente d'ADR-029.
- **Révision du 2026-10-19 (revue de Palawi, `fix/full-mix-sum`).** Doublon : si l'exe suivi
  par la piste Jeu est aussi une piste d'appli ajoutée à la somme, la piste Jeu envoie du
  silence au mixage tant que dure ce doublon (la piste Jeu elle-même reste enregistrée). Le
  test se fait à chaque bloc, car la piste Jeu change de jeu en cours de route ; casse ignorée.

## ADR-031 — Reprise automatique après une panne du pipeline (phase 7)
**Statut :** acceptée (2026-10-19, décision de Palawi pour l'enregistrement en cours).
- Contexte : toute erreur du pipeline (redémarrage du pilote graphique, capture perdue après
  une veille, piste arrêtée) désarmait le replay avec `replay_stopped` ; il fallait le réarmer
  à la main, et un enregistrement en cours s'arrêtait.
- Décision : pour les erreurs passagères (`EngineError::is_transient` : capture, encodage,
  audio, piste ou capture vidéo arrêtée), alerte `encoder_reset { reason }`, désarmement (le
  fichier en cours est finalisé, lisible jusqu'à la panne), puis réarmement après 1 s, 2 s et
  5 s. Le tampon de replay repart vide : les paquets d'avant viennent d'un autre encodeur. Un
  enregistrement en cours reprend tout seul dans un nouveau fichier, comme un découpage (choix
  de Palawi), en gardant l'état « replay armé ou non » d'avant la panne. Après trois échecs :
  `replay_stopped`, comme avant.
- Non passagères, donc pas de relance : fenêtre capturée fermée (choix de l'utilisateur),
  disque plein (STO-05), absence d'encodeur, réglages invalides.
- Le fil du moteur attend au plus 8 s pendant la relance ; les commandes reçues entre-temps
  sont traitées après.

## ADR-032 — Données sous l'identifiant de l'app (phase 7, avant l'installateur)
**Statut :** acceptée (2026-10-19, décision de Palawi).
- Contexte : l'installateur NSIS par utilisateur de Tauri installe dans
  `%LOCALAPPDATA%\<productName>` = `%LOCALAPPDATA%\PalaClips`, le dossier de nos caches et du
  buffer de replay. Et la case « supprimer les données » du désinstalleur efface
  `%APPDATA%\<identifiant>` et `%LOCALAPPDATA%\<identifiant>`, pas nos dossiers `PalaClips` :
  cochée, elle ne remettait pas l'app à zéro.
- Décision : réglages, bibliothèque et journal des enregistrements sous
  `%APPDATA%\com.palawi.palaclips\` ; caches et buffer de replay sous
  `%LOCALAPPDATA%\com.palawi.palaclips\` (à côté des données de WebView2). Les clips restent dans
  le dossier choisi (`Vidéos\PalaClips` par défaut), que le désinstalleur ne touche jamais.
- Migration automatique au premier lancement (`migrate::migrate_data`) : déplacement sans
  écraser, chemins des miniatures réécrits dans la base ; seuls les fichiers et sous-dossiers de
  données connus bougent. `%APPDATA%\PalaClips` est supprimé seulement vide ;
  `%LOCALAPPDATA%\PalaClips`, qui est aussi le dossier d'installation, n'est jamais supprimé ni
  renommé (révision du 2026-10-19 : test avec un faux programme installé, puis vérification sur
  une copie de la vraie installation, exe et entrée de désinstallation intacts). Vérifiée par tests et sur une copie des données réelles de Palawi
  (`scripts/migration-check.ps1` : chaque fichier retrouvé identique en SHA-256).

## ADR-033 — Capture sous GPU saturé : priorité temps réel et file bornée (phase 7)
**Statut :** acceptée (2026-10-20, défaut 1 de la séance T7).
- Contexte : un jeu au premier plan qui prend tout le GPU passe avant PalaClips ; la
  conversion d'image attendait 30–45 ms par image et l'encodeur accumulait jusqu'à 14 s de
  retard (vidéo qui saute, A/V décalé). Mesures dans `RESEARCH.md` § Capture vidéo.
- Décision : (1) le processus demande la classe d'ordonnancement GPU `REALTIME`
  (`D3DKMTSetProcessSchedulingPriorityClass`) au démarrage du moteur ; refusée, elle est
  seulement journalisée. Seul réglage efficace mesuré ; accordé sans admin avec HAGS.
  (2) File vidéo bornée : à chaque réveil, seule l'échéance la plus récente reçoit une image
  convertie, les échéances manquées répètent la dernière image encodée. Le retard ne dépasse
  plus la durée d'une conversion et l'image encodée a moins d'une période d'écart avec son
  instant. (3) Alerte `capture_gpu_saturated` quand 10 % des images restent en retard pendant
  5 s, retirée après 5 s à l'heure.
- Conséquence : notre travail GPU (conversion d'image, ~1 ms par image) passe avant celui du
  jeu ; coût sur les fps du jeu mesuré par PERF-03 GPU-bound.
- **Révision du 2026-10-20 (décision de Palawi après PERF-03)** : Kombustor fenêtré, GPU à
  100 %, « écran 1440p60 + 4 pistes » : perte de fps **7,5 %** avec la priorité temps réel
  (2 046 images source en 26 s, 0 en retard) contre **0,7 %** en priorité normale (393 images
  source, 852 images en retard sur 1 559, alerte `capture_gpu_saturated`). La priorité devient
  un réglage : `video.gpu_priority` = `game` (défaut, priorité normale, PERF-03 tenu) ou
  `capture` (temps réel), Réglages > Replay & qualité « Quand le GPU est à fond ». Appliqué à
  chaque démarrage de capture. L'alerte de saturation renvoie à ce réglage. File bornée et
  alerte inchangées.

## ADR-034 — Mises à jour et releases (phase 8)
**Statut :** acceptée (2026-10-20).
- Contexte : APP-10 demande des mises à jour automatiques depuis GitHub Releases, signature
  vérifiée, avec notes de version. La clé privée reste à Palawi : secret GitHub seulement,
  jamais sur le disque du dépôt ni lue par l'agent.
- Décision :
  - `tauri-plugin-updater` 2.14, appelé côté Rust uniquement (aucune permission JS). Recherche
    au démarrage puis toutes les 6 h (fil qui dort, pas d'attente active) sur
    `releases/latest/download/latest.json`. Une version nouvelle lève l'alerte
    `update_available` (info) ; l'installation se fait seulement sur clic dans Réglages →
    Général, jamais pendant un enregistrement, jamais en pleine partie sans action du joueur.
    Téléchargement d'abord, arrêt du moteur ensuite, installateur NSIS en mode `passive`, puis
    redémarrage.
  - Clé publique dans `tauri.conf.json` (`plugins.updater.pubkey`) ;
    `createUpdaterArtifacts` produit la signature `.sig` de l'installeur.
  - Workflow `release.yml` : un tag `vX.Y.Z` (version égale à `tauri.conf.json`, sinon échec)
    construit, signe et crée une release **brouillon** avec `latest.json` et les notes tirées
    de la section `## [X.Y.Z]` du CHANGELOG ; Palawi la publie. Un tag `test-vX.Y.Z` fait la
    même chose avec la version forcée, pour l'essai 1.0.0 → 1.0.1, en brouillon nommé « TEST »,
    supprimé avec son tag après l'essai. `workflow_dispatch` était écarté : GitHub ne l'accepte
    que pour un workflow présent sur `main`.
  - `PALACLIPS_UPDATE_URL` remplace l'adresse de `latest.json` pour l'essai sur serveur local
    (les fichiers d'une release brouillon ne sont pas téléchargeables sans authentification).
    Seule une adresse `http://127.0.0.1` ou `http://localhost` est acceptée, toute autre est
    ignorée (test `only_a_loopback_http_feed_can_replace_github`) ; même redirigé, l'updater
    n'installe qu'un paquet signé par la clé de Palawi. De plus, le plugin refuse toute adresse
    `http` en build release tant que `dangerousInsecureTransportProtocol` est faux, ce qui est le
    cas de `tauri.conf.json` : dans la build publiée, la redirection locale ne marche donc pas.
    Seule la build 1.0.0 de l'essai active cette option, par le fichier passé à `--config`.
- Conséquence : un build local avec installeur (`tauri build` sans `--no-bundle`) exige
  désormais la clé ; pour les mesures locales, ajouter
  `--config '{"bundle":{"createUpdaterArtifacts":false}}'`.

## ADR-035 — Encodeur vidéo sur le processeur (v1.1)
**Statut :** acceptée (2026-10-22, choix de Palawi : D).
- Contexte : sans encodeur matériel, PalaClips ne capture rien (ENC-05) ; c'est le cas des VM
  et de beaucoup de petits PC. La feuille de route (§ 3) prévoit un encodeur sur le processeur.
  On livre FFmpeg LGPL : x264/x265 (GPL) sont exclus. Mesures, licences et compatibilité :
  `RESEARCH.md` § Encodage par le processeur.
- Options :
  - **A. SVT-AV1 seul** (`libsvtav1` M10/M12 rtc, déjà dans nos DLL). Libre de redevances
    (licence AOM ; Sisvel ne vise que les appareils). Meilleure qualité par bit ; 1,3 à 1,7 cœur
    en 1080p30, 1080p60 à la limite. Lecture : Resolve, WebView2 et Android 12+ oui ; Discord
    desktop via les codecs de Windows ; iPhone seulement A17 Pro et après. Pas de H.264 sur CPU.
  - **B. OpenH264 par le binaire de Cisco**, téléchargé à la demande. H.264 (lecture partout),
    0,4 cœur en 1080p30, mais qualité la plus basse (VMAF NEG 85,8 contre 89–91). Coût : notre
    FFmpeg contient un OpenH264 compilé non couvert, et FFmpeg ne sait pas charger la DLL de
    Cisco → construire FFmpeg nous-mêmes ou écrire une interface Rust à l'API C d'OpenH264 ;
    téléchargement + vérification SHA-256, interrupteur, mention « OpenH264 Video Codec provided
    by Cisco Systems, Inc. », licence de Cisco dans les mentions. Couverture limitée à l'usage
    personnel non rémunéré (gênant pour un créateur payé).
  - **C. Encodeur H.264 de Windows** (`h264_mf`, MFT logiciel de Microsoft, déjà dans nos DLL).
    H.264 (lecture partout), rien à télécharger ; 1,0 cœur en 1080p30, 1,9 en 1080p60,
    VMAF NEG 89,3 en 1080p30 (entre OpenH264 et SVT-AV1 M12). Brevets : l'encodeur fait partie
    de Windows (Via LA facture les codecs intégrés au système), sans déclaration écrite de
    Microsoft pour les applis qui l'appellent. Limites : éditions N sans Media Feature Pack,
    Windows Server à vérifier, fils et priorité non réglables par FFmpeg (priorité abaissée en
    retrouvant les fils créés à l'ouverture).
  - **D. C + A selon le codec choisi** : H.264 → `h264_mf`, AV1 → `libsvtav1`. Le réglage
    « Codec » existe déjà ; « Automatique » prend le premier qui passe l'encodage de test
    (H.264 d'abord). Si l'encodeur de Windows manque (édition N), AV1 prend le relais.
  - Écartés : VP9 (pas de gain sur AV1, peu courant en MP4), libaom (ne suit pas en 1080p),
    rav1e (pas temps réel), kvazaar (brevets HEVC, lecture HEVC payante sous Windows), vvenc
    (VVC illisible).
- Recommandation de Claude : **D**. H.264 par défaut pour que les clips s'ouvrent partout
  (Discord, téléphones, Resolve), AV1 au choix pour la qualité et la taille, aucun téléchargement
  ni licence à afficher, le tout déjà dans nos DLL ; risque brevets au plus bas des options
  H.264. B coûte le plus cher pour la plus mauvaise image.
- Décision : **D**. H.264 sur le CPU par `h264_mf` (MFT logiciel de Windows), AV1 sur le CPU
  par `libsvtav1` en mode temps réel ; HEVC reste matériel seulement. Détection par encodage de
  test réel, comme pour NVENC/AMF/QSV.
- Mise en œuvre (2026-10-22) :
  - Réglage `video.encoder` = `auto` (matériel dès qu'un encodeur matériel marche, sinon CPU),
    `hardware` ou `cpu`. Erreur `no_video_encoder` quand rien ne marche ; `no_hardware_encoder`
    seulement si l'utilisateur a forcé le matériel.
  - File bornée : une image dupliquée coûte 50 à 70 % d'une vraie image sur le CPU (mesuré),
    dupliquer ne suffit donc pas à rattraper un retard. Au-delà d'une demi-seconde de ticks en
    retard, les plus anciens sont sautés : trou dans les PTS (le lecteur garde l'image
    précédente), synchro A/V intacte, alerte `cpu_encoder_overloaded`. Le CFR strict n'est
    rompu que quand le processeur ne suit plus, ce que l'alerte dit.
  - Priorité : fils de l'encodeur repérés par leur point de départ (`mfh264enc.dll` : 16 fils à
    la première image ; `avcodec-*.dll` : SVT-AV1 à l'ouverture) et passés « en dessous de la
    normale ». SVT-AV1 en `lp=2` (0,82 cœur contre 0,97 par défaut en 1080p30, 1,84× le temps
    réel). Le nombre de fils de `h264_mf` n'est pas réglable par FFmpeg.
  - Mémoire : `h264_mf` réserve ~720 Mo en 1080p (SVT-AV1 150 Mo), insensible aux modes et au
    nombre de fils. Palawi garde H.264 par défaut (2026-10-22) ; l'estimation de RAM de
    Réglages ajoute la mémoire de l'encodeur CPU, et le README la donne. PERF-02 (< 150 Mo hors
    tampon) reste défini pour le chemin matériel.
