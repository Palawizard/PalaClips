# Phase 1 — Vidéo

- Date : 2026-10-15
- Commit : `14a7a2c` (branche `feature/video-capture`)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF (sans iGPU), écrans 2560×1440 240 Hz (principal), 2560×1440 165 Hz,
  1920×1080 60 Hz

**Statut : en attente des tests 👤.** Tous les critères 🤖 sont ✅ ; la phase ne sera cochée
dans `docs/PLAN.md` qu'avec les résultats de Palawi.

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Le MP4 se lit ; `ffprobe` : `r_frame_rate` = `avg_frame_rate`, bonne durée (±1 image) | 🤖 | ✅ | `capture-video --target primary --fps 60 --seconds 10` → `600 images écrites` ; `ffprobe` : `codec_name=h264 width=2560 height=1440 r_frame_rate=60/1 avg_frame_rate=60/1 duration=10.000000 nb_frames=600` ; `ffmpeg -i … -f null -` code 0. Aussi `--fps 144 --codec hevc --height 1080 --seconds 5` → `r_frame_rate=144/1 avg_frame_rate=144/1 duration=5.000000 nb_frames=720` |
| Aucune lecture CPU de la texture (pas de `Map` de staging dans le chemin nominal) | 🤖 | ✅ | `grep -rnE "\.Map\(\|Unmap\|STAGING\|CPU_ACCESS_READ\|\.buffer\(\|buffer_crop\|hwframe_transfer" crates/` → aucun résultat ; dans `windows-capture` 2.0.1 le seul `Map` est dans `Frame::buffer()`/`buffer_crop()`, jamais appelés. Chemin : `CopyResource` GPU → Video Processor → frame `d3d11` → NVENC. CPU mesuré pendant une capture 1440p60 : 0,4 % de la machine (391 ms CPU sur 5 s, build debug) |
| Capture de Fortnite en fenêtre et en plein écran : image correcte, pas de saccade | 👤 | ❌ en attente | Protocole `docs/TESTING.md` T1 points 4–6 |
| Écran HDR : couleurs correctes (CAP-09) | 👤 | ❌ en attente | Protocole `docs/TESTING.md` T1 point 7 |

## Tâches

| Tâche | Résultat | Preuve |
|---|---|---|
| Énumération écrans/fenêtres, `list-targets` | ✅ | 3 écrans (`monitor:\\.\DISPLAY1..3`, nom, définition, Hz, principal) + fenêtres `window:<exe>` ; nom d'exe via `PROCESS_QUERY_LIMITED_INFORMATION` (processus protégés) ; `resolve-target --target window:VESKTOP.exe` retrouve la fenêtre par nom d'exe sans tenir compte de la casse |
| WGC → D3D11 → NV12 GPU → encodeur matériel sans copie CPU | ✅ | `probe-wgc` : `ID3D11VideoDevice` et `ID3D11VideoContext1` disponibles sur le périphérique WGC ; voir critères 🤖 |
| Pacer CFR sur QPC | ✅ | `CfrClock` + 6 tests (échéance exacte à 30/60/144/165/240 i/s, ticks manqués réémis, 600 images en 10 s, pas de dérive sur 1 h) ; nombres d'images exacts ci-dessus |
| Mise à l'échelle GPU | ✅ | `--height 1080` → 1920×1080, `--height 720` → 1280×720 depuis 2560×1440 |
| Curseur, bordure | ✅ code / 👤 visuel | Réglages `IsCursorCaptureEnabled` / `IsBorderRequired` appliqués ; la bordure jaune n'est pas dans l'image capturée (DWM), vérification visuelle T1 points 8–9 |
| Changement de résolution à chaud | ✅ | Bloc-notes capturé à 790×596 puis agrandi à 1500×700 et réduit pendant la capture : `2 changements de résolution`, 480 images pour 8 s, image agrandie mise en bandes noires (frame extraite à 4,2 s) |
| Multi-écrans | ✅ | `DISPLAY1` 2560×1440 → 120 images / 2 s ; `DISPLAY3` 1920×1080 → 120 images / 2 s |
| Détection des encodeurs par encodage de test ; message si aucun | ✅ | `ffmpeg-info` : `h264_nvenc`, `hevc_nvenc`, `av1_nvenc` disponibles (5 images converties + encodées chacun) ; `*_amf` « DLL amfrt64.dll failed to open », `*_qsv` « Error creating a MFX session: -9. » ; sans encodeur → `EngineError::NoHardwareEncoder` (« aucun encodeur vidéo matériel n'a fonctionné … Mets à jour le pilote … ») ; stderr vide (logs FFmpeg routés vers `tracing`) |
| Repli fenêtre → écran (CAP-04) | ✅ | Fenêtre réduite → « La fenêtre « … – Bloc-notes » ne peut pas être capturée : PalaClips capture son écran à la place. », 90 images 2560×1440 ; alerte `capture_fallback_monitor` |
| CLI `capture-video` | ✅ | Voir critères ; options `--target --fps --seconds --out --codec --height --quality --bitrate-mbps --no-cursor --border` |
| Préréglages de débit (ENC-02) | ✅ | Test `quality_presets_scale_bitrate_with_pixels_per_second` ; 1440p60 Léger → `bit_rate=17695440`, Qualité → `66355365`, `--bitrate-mbps 8` → `8000413` |

Automatique sur `14a7a2c` : `cargo fmt --check` ok, `cargo clippy --workspace --all-targets -- -D warnings` ok,
`cargo test --workspace` : 27 + 2 tests ok, `npm run build` ok. Aucun `todo!()` ni
`EngineError::NotImplemented` dans le code.

## Exigences SPEC couvertes

CAP-01, CAP-03, CAP-04, CAP-05, CAP-06, CAP-07 (visuel 👤), CAP-08 (multi-écrans et changement
de résolution ; mise à l'échelle Windows 150 % : point 10 facultatif de T1), CAP-09 (👤),
CAP-10 (WGC uniquement, aucune injection), ENC-01 (NVENC vérifié ; AMF et QSV codés et
détectés, non testables sur cette machine), ENC-02, ENC-03 (GOP = intervalle × fps, images
clés à 0, 1, 2… s dans `ffprobe -show_entries packet=pts_time,flags`), ENC-04, ENC-05.

## Écarts et corrections

- **rsmpeg** : `AVCodecContext::open(Some(dict))` libère deux fois le dictionnaire quand
  l'ouverture échoue (plantage pendant la détection d'AMF) ; options privées passées par
  `av_opt_set`.
- **QSV** : FFmpeg 8.1 n'accepte que des frames `qsv` ; dérivation D3D11 → QSV + `av_hwframe_map`.
  Non testé sur du matériel Intel ni AMD (pas d'iGPU sur le 13600KF) : la détection par
  encodage réel écartera l'encodeur s'il ne fonctionne pas.
- **Pacer** : un test a révélé une échéance de tick qui n'était pas « due » à son instant exact
  (arrondi) ; corrigé avant commit.
- **Arrêt** : l'envoi des paquets ne bloque plus l'arrêt si plus personne ne les lit.
- Capture lancée dans le Bloc-notes de Palawi pour le test de redimensionnement : il affichait
  une boîte « Conserver les modifications non enregistrées ? » sur un de ses fichiers, laissée
  telle quelle.
