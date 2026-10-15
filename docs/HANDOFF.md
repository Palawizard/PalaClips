# HANDOFF — où on en est

> Mis à jour à la fin de **chaque** session (`/handoff`). Une nouvelle session doit pouvoir
> reprendre en lisant uniquement `CLAUDE.md` + ce fichier.

## État actuel

- **Phase en cours :** 1 — Vidéo : code terminé et critères 🤖 ✅, **en attente des tests 👤 de
  Palawi** (Fortnite fenêtré/plein écran, HDR) — [rapport](reports/phase-1.md).
- **Branche :** `dev` (PR #5 `feature/video-capture` mergée et supprimée s'il ne reste rien à
  corriger ; sinon voir `gh pr list`).
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
  - CLI : `list-targets [--json]`, `resolve-target`, `probe-wgc`, `capture-video`.

## Prochaines actions (dans l'ordre)

1. Récupérer les résultats 👤 de Palawi (protocole `docs/TESTING.md` T1 points 4–10) et les
   reporter dans `docs/reports/phase-1.md`.
2. Si le HDR donne des couleurs délavées ou sombres : capturer en `ColorFormat::Rgba16F` sur
   un écran HDR et convertir scRGB → SDR (tone mapping) dans le Video Processor ou un shader,
   sur une branche `fix/hdr-capture` ; refaire T1 point 7.
3. Si Fortnite saccade ou échoue : analyser la dernière ligne de la CLI (images dupliquées,
   retard max) ; plein écran → vérifier que le repli sur l'écran s'affiche.
4. Tout ✅ → cocher les critères de la phase 1 dans `docs/PLAN.md`, puis phase 2 (audio
   système + micro) sur `feature/system-audio` depuis `dev`.
## Bloquants

Aucun.

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
| 2026-10-15 | 1 | Pipeline vidéo GPU (WGC → Video Processor NV12 → NVENC/AMF/QSV), pacer CFR QPC, repli, préréglages de débit, CLI `capture-video` ; 🤖 ✅ (600 images, 60/1) ; PR #5 | Tests 👤 Fortnite + HDR |
