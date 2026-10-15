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
**Statut :** acceptée, à confirmer en phase 0
- Alternatives : `ffmpeg-next` (mode maintenance seulement — écarté), Media Foundation seul
  (pas de MP4 hybride, multi-pistes plus pénible), SDK NVENC direct (NVIDIA seulement).
- Décision : `rsmpeg` 0.18 (feature `ffmpeg8`, liaison dynamique via `FFMPEG_DLL_PATH`,
  bindings via `FFMPEG_INCLUDE_DIR` + libclang). DLL : BtbN `ffmpeg-n8.1-latest-win64-lgpl-shared`.
- Risque : rsmpeg 0.18 date d'août 2025 (FFmpeg 8.0) ; la build actuelle est 8.1.
- **Plan B**, dans l'ordre : (1) épingler une build BtbN 8.0 datée ; (2) utiliser directement
  `rusty_ffmpeg` 0.17 (feature `ffmpeg8_1`) avec une fine couche sûre écrite dans `encode`/`mux`.
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
