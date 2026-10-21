# Phase 0 — Mise en route

- Date : 2026-10-15
- Commit : `6ac2712` (branche `chore/dev-environment`, PR #2 vers `dev`)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` et `cargo test --workspace` passent sur Windows | 🤖 | ✅ | Machine de Palawi, après `cargo clean -p rusty_ffmpeg` et sans contournement : clippy code 0, `test result: ok. 17 passed` ; CI Windows verte sur la PR #2 ([run 38766784543](https://github.com/Palawizard/PalaClips/actions/runs/38766784543)) |
| `palaclips-cli ffmpeg-info` affiche au moins un encodeur matériel sur la machine de Palawi | 🤖 | ✅ | `cargo run -p palaclips-cli -- ffmpeg-info` → `FFmpeg n8.1.3-18-g46d899d7a0-20261015`, `h264_nvenc`, `hevc_nvenc`, `av1_nvenc` disponibles (et `h264_mf`, `hevc_mf`) |
| Palawi voit la fenêtre de l'app | 👤 | ✅ | `npm run tauri dev` lancé, fenêtre « Moteur 0.1.0 » confirmée par Palawi le 2026-10-15 |

## Tâches

| Tâche | Résultat | Preuve |
|---|---|---|
| `pwsh scripts/check-env.ps1` tout vert | ✅ | 13 lignes `[OK]`, « Environnement prêt. » |
| FFmpeg + `rsmpeg` 0.18 lié, `ffmpeg-info` | ✅ | Voir critère ci-dessus ; ADR-004 mis à jour |
| Icônes | ✅ | Présentes depuis la création du repo, logo inchangé |
| `npm run tauri dev` affiche « Moteur 0.1.0 » | ✅ | Confirmé par Palawi |
| CI verte | ✅ | Verte sur les PR #1 et #2 vers `dev` (voir écarts) |
| Skills de design + `/impeccable init` | ✅ | `PRODUCT.md` commité, direction « Écran d'après-match » choisie par Palawi (seed `cabeab00`), contrat dans `.impeccable/surfaces/app-src-app-tsx.md` |

## Exigences SPEC couvertes

Aucune exigence fonctionnelle : phase d'environnement.

## Écarts et corrections

- **bindgen ↔ LLVM** : `rsmpeg 0.18` compile avec FFmpeg 8.1, mais `rusty_ffmpeg 0.16` épingle
  bindgen 0.71, qui génère des structs opaques avec LLVM ≥ 22 (rust-bindgen #3264). Copie de
  `rusty_ffmpeg 0.16.7` avec bindgen 0.72.1 dans `third_party/rusty_ffmpeg` (choix de Palawi,
  ADR-004).
- **DLL FFmpeg au lancement** : `STATUS_DLL_NOT_FOUND` pour les binaires de test ; runner Cargo
  `scripts/with-ffmpeg.cmd`.
- **Machine de Palawi** : LLVM ajouté au PATH utilisateur ; charge « Développement Desktop en
  C++ » ajoutée à VS 2022 Community, que clang choisissait sans outils C++. `check-env.ps1`
  détecte maintenant ce cas.
- **CI « verte sur `main` »** : depuis l'adoption de la branche d'intégration `dev`, `main` ne
  reçoit que les versions publiées. Le critère est vérifié sur les PR vers `dev` ; la CI
  tourne aussi sur les pushs vers `dev`.
- **`DESIGN.md`** : avec impeccable 4.3, `init` n'écrit que `PRODUCT.md` ; `DESIGN.md` est
  produit à partir du premier écran construit. Décision de Palawi : il est produit en phase 5
  avec le premier écran (tâche ajoutée à la phase 5 du plan).
- `EngineError::NotImplemented` existe toujours dans `error.rs` mais n'est utilisé par aucun
  code de cette phase ; sa suppression est prévue en phase 8.
