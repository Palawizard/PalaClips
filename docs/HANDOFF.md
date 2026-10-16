# HANDOFF — où on en est

> Mis à jour à la fin de **chaque** session (`/handoff`). Une nouvelle session doit pouvoir
> reprendre en lisant uniquement `CLAUDE.md` + ce fichier.

## État actuel

- **Phase en cours :** 2 — Audio système + micro : code terminé, **pas validée** (T2 d'une
  heure à faire par Palawi). Phase 1 validée ; ses suites (PR #7) mergées.
- **Branche :** `feature/system-audio`, **PR #8 ouverte vers `dev`**, à merger seulement
  quand le T2 est vert (critère de la tâche).
- **Dernière mise à jour :** 2026-10-15

## Ce qui existe

- Workspace Cargo (`crates/engine`, `crates/cli`, `app/src-tauri`), edition 2024.
- Moteur : `clock` (`Hns`), `config` (`EngineConfig`), `events` (`EngineEvent`), `error`,
  `replay` (`PacketRing`, estimation RAM), et `encode` (Windows) :
  `ffmpeg_version()`, `hardware_video_encoders()` (liste les encodeurs matériels/hybrides et
  tente de les ouvrir en NV12 720p60).
- FFmpeg : `rsmpeg 0.18` + DLL BtbN LGPL n8.1 dans `third_party/ffmpeg/` (gitignoré,
  `pwsh scripts/fetch-ffmpeg.ps1`). `third_party/rusty_ffmpeg/` = copie de
  `rusty_ffmpeg 0.16.7` avec bindgen 0.72.1, branchée par `[patch.crates-io]` (ADR-004).
- `.cargo/config.toml` : `FFMPEG_INCLUDE_DIR`, `FFMPEG_DLL_PATH`, et runner
  `scripts/with-ffmpeg.cmd` qui met les DLL dans le PATH pour `cargo run`/`test`/`tauri dev`.
- CLI : `info`, `estimate`, `ffmpeg-info` (sur la 4070 Super : `h264_nvenc`, `hevc_nvenc`,
  `av1_nvenc` disponibles).
- `scripts/check-env.ps1` : vérifie aussi que clang trouve MSVC/Windows SDK et trouve les
  skills de design là où ils sont réellement installés. Tout vert sur la machine de Palawi.
- App Tauri 2.12 minimale (fenêtre « Moteur 0.1.0 » vue par Palawi), front Vite + React 19 +
  Tailwind 4, écran provisoire.
- Design : `PRODUCT.md` (impeccable init, tutoiement, logo provisoire) ; direction
  « Écran d'après-match » (ADR-014) dans `.impeccable/surfaces/app-src-app-tsx.md`.
  Pas encore de `DESIGN.md` (phase 5).
- CI Windows : fmt, clippy `-D warnings`, tests, build front ; sur PR et pushs `main`/`dev`.
- Code sans commentaires ; conventions dans `CLAUDE.md` § 4 (modèle `main`/`dev` : ADR-013).
- **Phase 1 (2026-10-15)**, pipeline vidéo GPU (ADR-015) :
  - `capture/targets.rs` : `list_targets()`, `resolve_target(&CaptureTarget)` (fenêtre par nom
    d'exe, casse ignorée, `title_hint`) ; `capture/pacer.rs` : `CfrClock` (ticks QPC,
    6 tests) ; `capture/video.rs` : `VideoCapture::start(target, &VideoSettings, encoder)` →
    `stream()` (`VideoStream` : codecpar, time_base, fps, taille, `origin` QPC),
    `packets()` (canal borné d'`AVPacket`), `alert()` (repli), `stop()` → `VideoCaptureStats`.
  - `encode/` : `hardware_video_encoders()` (encodage de test réel, raison d'échec),
    `pick_encoder`, `VideoEncoder` (frames `d3d11`, QSV dérivé), `GpuConverter` (Video
    Processor), `route_ffmpeg_logs()` (FFmpeg → `tracing`, cible `ffmpeg`).
  - `mux.rs` : `Mp4Writer` (MP4 classique vidéo seule). `clock` : `Hns::now()` (QPC).
  - `config` : `VideoQuality` (Qualité/Équilibré/Léger/Custom), `bitrate_kbps_for(w, h)`.
  - Erreurs : `TargetNotFound`, `Capture`, `Encoder`, `Mux`, `CodecUnavailable`.
  - CLI : `list-targets [--json]`, `resolve-target`, `probe-wgc`, `probe-hdr`, `capture-video`.
  - HDR (ADR-015) : `capture/display.rs` (`display_color(device_name)` : HDR actif, blanc
    SDR), `encode/hdr.rs` (`HdrToSdr` : pixel shader FP16 scRGB → BGRA8 sRGB). Écran HDR →
    capture FP16 ; SDR → BGRA8.
- Tests 👤 de la phase 1 faits avec Palawi : Fortnite fenêtré et plein écran fluides, HDR
  correct après correction, bordure, curseur, mise à l'échelle 150 %.
- **Suites de la phase 1 (PR #7, 2026-10-15)** : `SendCom` limité aux types D3D11 ;
  toolchain épinglée 1.94.1 ; HDR détecté par `activeColorMode`
  (`GET_ADVANCED_COLOR_INFO_2`, `display_color_info`) ; hautes lumières HDR atténuées
  (ADR-016, `ROLL_OFF_START = 0.9`) ; horloge vidéo démarrée après le premier encodage ;
  `probe-wgc` affiche l'horloge DWM ; limite G-Sync documentée.
- **Phase 2 (`feature/system-audio`, PR #8)** :
  - `audio/` (Windows) : `AudioSource` (`SystemMix`, `Microphone { gain_db }`),
    `AudioTrack::start(source, titre, origin, sync_offset)` → `stream()` (`AudioStream`),
    `packets()`, `alerts()`, `finish_at(fin)`, `stop(fin)` ; un thread par source, attente
    `WaitForMultipleObjects` (arrêt, données, défaut changé), réouverture sur changement de
    défaut / `AUDCLNT_E_DEVICE_INVALIDATED`, alertes `audio_device_changed` et
    `audio_device_unavailable` ; `GapFiller` (`filler.rs`, 7 tests).
  - `encode/audio.rs` : `AacEncoder` (AAC 48 kHz stéréo 192 kb/s, PTS = échantillons).
  - `mux.rs` : `Mp4Writer::create(path, &[MuxStream])`, `write(index, packet)`, titres.
  - Config : `full_mix_sync_offset_ms`, `microphone_sync_offset_ms`,
    `AppTrack::sync_offset_ms` (AUD-11, ADR-017), `ipc.ts` à jour ; `EngineError::Audio`.
  - CLI : `probe-audio [--mic] [--seconds]`, `probe-av-emitter [--seconds]`,
    `capture-av` (mêmes options que `capture-video` + `--no-mix`, `--no-mic`,
    `--mic-gain-db`, `--mix-offset-ms`, `--mic-offset-ms`).
  - Scripts : `scripts/t2-av-sync.ps1` (T2 automatisé, `-Minutes`, `-NoPause`),
    `scripts/measure-av-sync.py` (Python 3, flashs ↔ bips).
  - Preuves : `docs/reports/phase-2/` (journaux `probe-audio`, débranchement), rapport
    `docs/reports/phase-2.md`.

## Prochaines actions (dans l'ordre)

0. **T2 suspendu** : décision de Palawi sur le retard du micro (« Décisions en attente »).
   Mesure courte : `pwsh scripts\t2-av-sync.ps1 -CalibrateOnly -OutDir target\clips\mic-N`.
1. Récupérer de Palawi la sortie de `pwsh scripts\t2-av-sync.ps1` (T2, 1 h ; voir
   `docs/TESTING.md` § T2). Attendu : 3 × `OK` mix et 3 × `OK` micro. Si le micro « n'a
   entendu aucun bip », lui faire coller l'écouteur au micro et relancer.
2. T2 vert → compléter `docs/reports/phase-2.md` (critères 1 et 2 avec la sortie), cocher
   la phase 2 dans `docs/PLAN.md`, attendre la CI de la PR #8, merger, supprimer la branche.
   T2 rouge → corriger sur `feature/system-audio` avant toute autre chose.
3. Phase 3 (`docs/PLAN.md`) sur une nouvelle branche depuis `dev` : process loopback
   (`AudioClient::new_application_loopback_client(pid_parent, true)`) en réutilisant
   `AudioTrack`/`GapFiller` (ajouter une variante `AudioSource::Application`), `sources`
   (sessions + crêtes), rattachement par nom d'exe, `app_track_silent`. Commencer par une
   sonde CLI sur la piste muette signalée sous build 26200.

## Bloquants

Aucun.

## Décisions en attente de Palawi

- **Retard de la chaîne micro instable d'un lancement à l'autre (2026-10-16, bloque le T2)** :
  micro +473 ms puis +410 ms après relance de LightHost et Voicemeeter (−63 ms ; tolérance
  du T2 : 16,7 ms), mix stable (+146 / +148 ms). Détails dans `RESEARCH.md` § Audio. Un
  décalage fixe (AUD-11) ne peut pas garder la piste micro synchro d'un lancement à l'autre.
  Options :
  1. **Recalibrer à chaque lancement de la chaîne** : AUD-11 inchangé ; l'assistant de
     calibrage (phase 5) se relance à la main après chaque démarrage de Voicemeeter/LightHost.
     Le T2 se fait en une session (calibrage puis capture sans relancer la chaîne) et reste
     valide pour le moteur. Inconvénient : un oubli = micro décalé de plusieurs dizaines de ms.
  2. **Capturer le micro physique direct** dans PalaClips (périphérique d'entrée physique au
     lieu de « CABLE Output ») : plus de retard Voicemeeter/LightHost à l'enregistrement,
     mais la voix enregistrée n'a pas le traitement VST (la chaîne reste pour Discord).
  3. **Isoler d'abord la cause** : une mesure de 35 s avec le micro physique comme micro par
     défaut dit si la variation vient de l'entrée (Voicemeeter/LightHost/VB-Cable) ou de la
     sortie de Voicemeeter vers le casque. La sortie compte dans la mesure (le bip passe par
     l'air) mais pas dans l'usage réel (la voix ne passe pas par la sortie) : si elle explique
     la variation, le vrai retard du micro est peut-être stable.
  Recommandation de Claude : 3, puis 1 ou 2 selon le résultat.
- Alerte G-Sync validée le 2026-10-16 : CAP-11, ADR-018, code en phase 7.

## Points d'attention

- Retirer `third_party/rusty_ffmpeg` et le `[patch.crates-io]` dès qu'une version de rsmpeg
  dépend de `rusty_ffmpeg ≥ 0.17` (rsmpeg inactif depuis août 2025).
- Tout exécutable lié à l'engine a besoin des DLL FFmpeg dans le PATH : passer par
  `cargo run`/`cargo test` (runner) ; le bundle NSIS (phase 8) devra les livrer à côté de l'exe.
- AMF et QSV sont codés mais jamais testés sur du vrai matériel (Palawi : 13600KF sans iGPU,
  pas de carte AMD). La détection par encodage réel les écarte s'ils échouent.
- `rsmpeg` : ne jamais passer de dictionnaire à `AVCodecContext::open` (double libération si
  l'ouverture échoue) → `av_opt_set` sur `priv_data`.
- Cibles `monitor:\\.\DISPLAYn` : lancer la CLI depuis PowerShell (Git Bash retire une barre
  oblique). `cargo run` passe aussi par `with-ffmpeg.cmd`.
- La CLI n'est pas « DPI aware » : les tailles de fenêtres de `list-targets` peuvent être
  virtualisées sous mise à l'échelle Windows (la capture, elle, est en pixels physiques).
- Toolchain épinglée sur Rust 1.94.1 (`rust-toolchain.toml`, la CI fait `rustup toolchain
  install`) : la monter volontairement, dans un commit `build(repo)`, jamais par la CI seule.
- G-Sync + écrans à fréquences différentes : WGC ne livre que la cadence de l'autre écran
  (~165 i/s pour un jeu à 225 sur la machine de Palawi, `RESEARCH.md`). Sortie CFR correcte.
- Machine de Palawi : sortie par défaut « Voicemeeter Input » (loopback ~100 ms en retard →
  `--mix-offset-ms` ≈ −100 à −120, AUD-11), interface USB Apogee BOOM, micro par défaut
  « CABLE Output ». Les applis en pause envoient des zéros **sans** drapeau SILENT.
- Les noms GDI `\\.\DISPLAYn` se renumérotent pendant une session : pour les tests,
  `--target primary` ; CAP-03 (mémoriser un écran) devra utiliser un identifiant stable
  (chemin du moniteur / EDID), pas le nom GDI.
- Tests avec fenêtre (VLC, `probe-av-emitter`) : toujours demander à Palawi avant, ils
  couvrent l'écran principal. Ne jamais capturer un autre écran que celui demandé.
- PowerShell : `@(a, b, c * 60)` multiplie le **tableau** (priorité de la virgule) ; mettre
  des parenthèses.
- Tests 👤 : Claude lance lui-même les commandes et ne demande à Palawi que les actions
  physiques et son avis visuel, via le widget de questions.
- HDR basculé pendant une capture : pris en compte seulement au démarrage suivant (à traiter
  en phase 7, robustesse).
- Phase 4 : le replay devra cloner les `AVPacket` (refcount) pour le fan-out, et utiliser
  `VideoStream::origin` comme `t0` commun avec l'audio.
- La capture audio par appli a un rapport de piste muette sur Windows 11 build 26200 (avec
  NAudio, cause non établie) : à tester dès la phase 3 sur la machine de Palawi.
- Tauri 3 est en alpha sur crates.io : ne pas suivre les suggestions de mise à jour vers 3.x.
- Fin de tâche : merger soi-même la PR vers `dev` quand la CI est verte, puis supprimer la
  branche (`CLAUDE.md` § 4 Git, « Merge et ménage »).

## Journal des sessions

| Date | Phase | Fait | Suite |
|---|---|---|---|
| 2026-10-15 | — | Création du repo, docs, squelette, CI | Phase 0 |
| 2026-10-15 | 0 | Règles Git `main`/`dev` (PR #1) ; env Windows vert, rsmpeg 0.18 ↔ FFmpeg 8.1 (bindgen 0.72.1), `ffmpeg-info`, `PRODUCT.md`, direction visuelle ; phase 0 validée (PR #2) | Phase 1 |
| 2026-10-15 | 1 | Pipeline vidéo GPU (WGC → Video Processor NV12 → NVENC/AMF/QSV), pacer CFR QPC, repli, préréglages de débit, CLI `capture-video` ; PR #5. Tests 👤 avec Palawi : Fortnite ✅, HDR délavé → correction FP16 + shader (`fix/hdr-capture`) ✅ ; phase 1 validée | Phase 2 |
| 2026-10-15 | 1→2 | Suites phase 1 (PR #7 mergée : HDR par `activeColorMode`, atténuation des hautes lumières, horloge après le 1er encodage, 240 i/s, limite G-Sync). Phase 2 : `probe-audio`, `AudioTrack`/`GapFiller`/AAC/mux multi-pistes, `capture-av`, débranchement ✅, Voicemeeter +100 ms → AUD-11 (décalage par piste), T2 automatisé ; PR #8 ouverte | T2 1 h par Palawi, puis merge et phase 3 |
