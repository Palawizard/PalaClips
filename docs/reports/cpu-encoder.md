# Rapport — Encodage par le processeur (v1.1, ENC-05, ENC-06, ADR-035)

Date : 2026-10-22. Machine de Palawi : i5-13600KF (20 processeurs logiques), RTX 4070 Super,
écran 1440p 240 Hz, Windows 11 26200. Journaux : `docs/reports/cpu-encoder/`.

## Choix

Recherche (`RESEARCH.md` § Encodage par le processeur) puis choix de Palawi : **D**, H.264 par
l'encodeur logiciel de Windows (`h264_mf`) et AV1 par SVT-AV1 (`libsvtav1`, temps réel, `lp=2`),
tous deux déjà dans le FFmpeg LGPL livré. H.264 reste le codec CPU par défaut malgré la mémoire
de `h264_mf` (décision de Palawi, ci-dessous).

## Critères

| Critère | Résultat | Preuve |
|---|---|---|
| Encodeurs CPU détectés par encodage de test réel | ✅ `h264_mf` et `libsvtav1` « disponible » à côté de NVENC | `palaclips-cli ffmpeg-info` |
| Coût et qualité mesurés, 720p30 / 1080p30 / 1080p60 | ✅ | `cpu-encoders-2026-10-22_13-31.txt`, tableau dans RESEARCH |
| Conversion GPU (Video Processor → NV12 → une relecture), repli swscale | ✅ couleurs BT.709 limitées en NV12 et yuv420p, bandes noires, Video Processor sous WARP | tests `encode::readback` (WARP, en CI) |
| Fils en priorité « en dessous de la normale », nombre limité | ✅ fils repérés par module (`mfh264enc.dll` : 16 à la 1re image ; `avcodec-62.dll` : SVT à l'ouverture) ; SVT `lp=2` | `probe-encoder-threads`, test `encode::threads` |
| Même contrat que le matériel : CFR, images clés, file bornée | ✅ capture CLI 10 s en 1080p30 : 300 images, CFR, 0 en retard (H.264 et AV1) ; au-delà d'une demi-seconde de retard les échéances les plus anciennes sont sautées, alerte `cpu_encoder_overloaded` | tests `capture::pacer`, `capture::rate` |
| Replay, enregistrement, découpe précise et export sur le CPU | ✅ replay de l'app isolée en CPU forcé ; découpe précise AV1 sur le CPU à l'image près (test en CI) ; l'enregistrement passe par le même pipeline que le replay (REC-02), pas essayé à part en CPU | test `a_precise_cut_on_the_processor_starts_on_the_exact_frame` |
| App : réglage Encodeur, avertissement, 1080p30, estimation, premier lancement | ✅ vu dans l'app isolée (sombre FR, clair EN) ; critique + audit impeccable, 0 constat du détecteur, corrections appliquées | PR #94 |
| Capture + replay CPU forcé sur la machine de Palawi, clip lisible | ✅ clip de 60,7 s : 1 821 images à 30 i/s, 3 pistes AAC de 60,707 s, décodage sans erreur | app isolée, 2026-10-22 14:27 |
| A/V aligné | ✅ décalage de la piste mix 52,5 ms (NVENC) contre 48,8 ms (CPU) sur le même banc : écart 3,7 ms < 1 image (33 ms) ; les ~50 ms viennent de la chaîne Voicemeeter | `t2-av-sync.ps1 -CalibrateOnly -VideoArgs …` |
| CPU de PalaClips | ✅ 0,36 cœur = 1,8 % de la machine (bureau, 1080p30 H.264, replay armé) ; panneau Performances 1,64 % (max 3,4 %) | app isolée |
| Perte de fps dans Fortnite, CPU forcé | ✅ mesurée : 0 % en moyenne (jeu plafonné à 225 i/s, GPU 24–34 %) ; 1 % bas −5 % (H.264 1080p30), −4 % (AV1 1080p30), −3 % (H.264 720p30), 0 % (NVENC 1080p30) ; 0 image en retard ; écrite dans le README | `fps-impact-2026-10-22_14-49.txt` |
| Mise à jour 1.0.0 → 1.1.0 (serveur local, installeur signé du brouillon `test-v1.1.0`) | ✅ fausse signature refusée, 1.1.0 installée, notes affichées, données de Palawi identiques (593 fichiers) ; `latest.json` du brouillon : version 1.1.0, signature, lien au format public `releases/download/<tag>/…` | `update-test-2026-10-22_15-27.txt` |
| VM Windows 11 sans GPU : capture et sauvegarde d'un clip par le CPU | ⏳ protocole T11 (`TESTING.md`), installeur de la release v1.1.0 | à rapporter par Palawi |

## Mémoire

`h264_mf` réserve beaucoup de mémoire, quels que soient ses modes et son nombre de fils : 366 /
720 / 1 122 Mo en 720p / 1080p / 1440p (SVT-AV1 : 87 / 150 / 238 Mo). Dans l'app, panneau
Performances : 470 Mo hors tampon en `h264_mf` contre 73 Mo en NVENC. Décision de Palawi :
H.264 reste le défaut sur le CPU ; Réglages → Replay & qualité ajoute « encodeur ≈ X Mo en RAM »
à l'estimation, et le README le dit. PERF-02 reste défini pour le chemin matériel.

## Limites connues

- Fortnite était plafonné à 225 i/s : la perte moyenne d'un jeu limité par le processeur sans
  plafond n'a pas été mesurée (choix de Palawi de garder ce passage).
- Le nombre de fils de `h264_mf` n'est pas réglable par FFmpeg (16 sur cette machine).
- Éditions N de Windows : `h264_mf` demande le Media Feature Pack ; AV1 marche sans.
