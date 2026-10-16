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
- Capture vidéo (phase 1) : écrans et fenêtres par Windows Graphics Capture, fenêtre retrouvée
  par nom d'exe, conversion NV12 et mise à l'échelle sur le GPU, encodage matériel H.264 /
  HEVC / AV1 sans copie CPU, fps constant sur l'horloge QPC, curseur et bordure réglables,
  changement de résolution à chaud, repli fenêtre → écran.
- Encodeurs détectés par un encodage de test réel (NVENC, AMF, Quick Sync) avec la raison d'un
  échec ; message clair si aucun ; logs FFmpeg dans les journaux au lieu de la console.
- Préréglages de débit Qualité / Équilibré / Léger et débit libre.
- Écran HDR : la vidéo garde les couleurs de l'écran au lieu d'être délavée.
- CLI : `list-targets`, `resolve-target`, `probe-wgc`, `probe-hdr`, `capture-video`.
- Documentation : SPEC V1, architecture, plan par phases, handoff, ADR, recherche, protocoles
  de test, brief design.
- Audio (phase 2) : piste « Mix complet » (tout ce qui sort par défaut) et piste « Micro »
  (gain réglable) en AAC dans le MP4, synchronisées sur la même horloge que la vidéo ; les
  silences et les coupures sont comblés sans décaler la synchro ; la capture suit le
  périphérique par défaut quand on change de sortie ou qu'on débranche le casque.
- Décalage de synchro par piste, pour compenser les périphériques virtuels comme Voicemeeter.
- CLI : `capture-av`, `probe-audio`, `probe-av-emitter` ; script de test de synchro sur une
  heure (`scripts/t2-av-sync.ps1`).

### Corrigé
- Écran SDR avec gestion automatique des couleurs : il n'est plus pris pour un écran HDR.
- Écran HDR : les hautes lumières (ciel, soleil, explosions) gardent leurs détails et leur
  couleur au lieu de virer au blanc uniforme.
- Capture à 240 i/s : plus de retard au démarrage (première image encodée avant le départ de
  l'horloge).
