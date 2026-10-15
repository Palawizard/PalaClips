# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

### Ajouté
- Création du repo : workspace Cargo (`palaclips-engine`, `palaclips-cli`, app Tauri 2),
  front React 19 + TypeScript + Tailwind 4, CI Windows.
- Moteur : horloge `Hns` (QPC), `EngineConfig` validée, contrat `EngineEvent`, file de replay
  `PacketRing` purgée par âge, estimation de RAM — avec tests.
- CLI `palaclips-cli ffmpeg-info` : version de FFmpeg et encodeurs vidéo matériels
  disponibles (NVENC, AMF, QSV, Media Foundation).
- Liaison FFmpeg 8.1 LGPL via `rsmpeg` 0.18 ; `cargo run`/`cargo test` trouvent les DLL seuls.
- `check-env.ps1` détecte les problèmes clang/Visual Studio et les skills de design.
- Contexte produit `PRODUCT.md` et direction visuelle « Écran d'après-match ».
- Documentation : SPEC V1, architecture, plan par phases, handoff, ADR, recherche, protocoles
  de test, brief design.
