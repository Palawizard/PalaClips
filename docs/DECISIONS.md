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
- Noms par défaut des pistes (Mix complet / Full mix, Micro / Mic, Jeu / Game) : écrits dans la
  langue du système à la création ou à la réinitialisation des réglages, puis traités comme
  des noms choisis par l'utilisateur (ils vont dans les fichiers). Le dossier des clips hors
  jeu garde le nom « Bureau » sur le disque.
- Garde-fou : `npm test` (`app/tests/i18n.test.mjs`, CI) échoue si un texte visible est écrit
  en dur dans `app/src` (texte JSX, attributs `aria-label`/`title`/`placeholder`/`alt`, props
  `label`/`help`/`error`/`unit`…, toute chaîne avec une lettre accentuée), si `fr.json` et
  `en.json` n'ont pas les mêmes clés et paramètres, ou si un code `Localized::new("…")` du
  Rust n'a pas de texte dans les deux langues.
- Conséquence : à partir de la phase 6, tout texte visible s'écrit directement en clé.
