# HANDOFF — où on en est

> Mis à jour à la fin de **chaque** session (`/handoff`). Une nouvelle session doit pouvoir
> reprendre en lisant uniquement `CLAUDE.md` + ce fichier.

## État actuel

- **Phase en cours :** 1 — Vidéo (pas commencée). Phase 0 validée ([rapport](reports/phase-0.md)).
- **Branche :** `chore/dev-environment` (PR #2 vers `dev`, CI verte, à merger par Palawi).
  PR #1 `chore/dev-branch-workflow` → `dev` aussi ouverte (règles Git à deux branches).
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

## Prochaines actions (dans l'ordre)

1. Palawi merge les PR #1 et #2 dans `dev`. Puis `git switch dev && git pull`.
2. Phase 1 sur une branche `feature/video-capture` depuis `dev` : énumération écrans/fenêtres
   (`windows-capture`), CLI `list-targets`.
3. Session WGC → texture D3D11 → NV12 GPU → encodeur `d3d11va` sans copie CPU ; partir de
   `encode::hardware_video_encoders()` pour la détection par encodage de test réel (ENC-01).
4. Pacer CFR sur QPC, CLI `capture-video`, vérification `ffprobe` (`r_frame_rate` =
   `avg_frame_rate`).

## Bloquants

Aucun.

## Points d'attention

- Retirer `third_party/rusty_ffmpeg` et le `[patch.crates-io]` dès qu'une version de rsmpeg
  dépend de `rusty_ffmpeg ≥ 0.17` (rsmpeg inactif depuis août 2025).
- Tout exécutable lié à l'engine a besoin des DLL FFmpeg dans le PATH : passer par
  `cargo run`/`cargo test` (runner) ; le bundle NSIS (phase 8) devra les livrer à côté de l'exe.
- `ffmpeg-info` laisse FFmpeg écrire ses erreurs d'ouverture sur stderr (AMF/QSV absents) :
  router les logs FFmpeg vers `tracing` en phase 1.
- La capture audio par appli a un rapport de piste muette sur Windows 11 build 26200 (avec
  NAudio, cause non établie) : à tester dès la phase 3 sur la machine de Palawi.
- Tauri 3 est en alpha sur crates.io : ne pas suivre les suggestions de mise à jour vers 3.x.
- Branche locale `devc` (même commit que `dev`) : probablement une faute de frappe, à
  supprimer par Palawi s'il le confirme.

## Journal des sessions

| Date | Phase | Fait | Suite |
|---|---|---|---|
| 2026-10-15 | — | Création du repo, docs, squelette, CI | Phase 0 |
| 2026-10-15 | 0 | Règles Git `main`/`dev` (PR #1) ; env Windows vert, rsmpeg 0.18 ↔ FFmpeg 8.1 (bindgen 0.72.1), `ffmpeg-info`, `PRODUCT.md`, direction visuelle ; phase 0 validée (PR #2) | Phase 1 |
