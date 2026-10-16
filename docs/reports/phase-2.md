# Phase 2 — Audio système + micro

- Date : 2026-10-15, T2 le 2026-10-16
- Commit : `2946d3e` (`feature/system-audio`, PR #8)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF ; sortie par défaut « Voicemeeter Input » (Voicemeeter vers une
  interface USB Apogee BOOM), micro par défaut « CABLE Output » (VB-Cable)

**Statut : ✅ validée** (T2 d'une heure le 2026-10-16, journal complet :
`docs/reports/phase-2/t2-av-sync-1h.txt`).

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Test du clap (T2) sur **1 h** : décalage < 1 image au début, au milieu et à la fin, sur les deux pistes | 👤 | ✅ | `pwsh scripts\t2-av-sync.ps1` le 2026-10-16 de 11:12 à 12:14, écouteur collé au micro physique, chaîne Voicemeeter/LightHost non relancée. Calibrage : mix +148,3 ms, micro +419,1 ms → `--mix-offset-ms -148 --mic-offset-ms -419`. Moyennes son − image (19 à 20 paires chacune) : **mix +4,7 / +0,1 / −3,7 ms**, **micro +5,1 / −0,3 / −3,7 ms** à 0, 30 et 60 min, 6 × `OK` (< 16,7 ms) ; extrêmes −11,9 / +8,9 ms. `ffprobe` : vidéo et 2 pistes de `duration=3640.000000`, `start_time=0`, 218 400 images |
| Silence de 2 min au milieu : synchro intacte | 👤 | ✅ | Silence de 28:00 à 30:00 (rappel du script) ; mesure de 30:00 juste après : mix +0,1 ms, micro −0,3 ms, `OK` |
| Débrancher/rebrancher le casque pendant la capture : pas de crash, la piste continue | 👤 | ✅ | Sortie par défaut Apogee BOOM, `capture-av --target primary --seconds 60`, USB débranché vers 15 s puis rebranché vers 35 s par Palawi : `flux audio interrompu … 0x88890004` à 21:26:21.510, `périphérique audio ouvert … Voicemeeter Input` à 21:26:21.522, retour sur l'Apogee à 21:26:54 ; 2 alertes « la capture suit maintenant … » ; `ffprobe` : vidéo 60,000 s / 3600 images, pistes de 60 s ; `ffmpeg -map 0 -f null -` code 0. Journal : `docs/reports/phase-2/capture-av-unplug.txt`. La fin de piste dépassait de 3,7 ms sur l'Apogee (horodatages en avance) → corrigé (`b461dfc`), revérifié : 480 000 échantillons exacts sur 2 essais |

## Tâches

| Tâche | Statut | Preuve |
|---|---|---|
| Loopback de la sortie par défaut, micro, horodatage QPC | ✅ | `AudioTrack` (`audio/track.rs`), `pu64QPCPosition` ; sondes `probe-audio` (`docs/reports/phase-2/probe-audio-loopback*.txt`) |
| Combleur de silences, `AUDCLNT_BUFFERFLAGS_SILENT`, pas d'arrêt sur timeout | ✅ | `GapFiller` + 7 tests (tremblement ±2 ms sans correction, trou → silence, chevauchement coupé, avant t0, fin exacte, dérive ±200 ppm sur 1 h sans trou) ; drapeau SILENT → zéros ; attente sans délai, piste qui continue sans périphérique |
| Encodeurs AAC par piste, amorçage, titres des pistes | ✅ | `AacEncoder` + test (premier PTS = −1024, fin = dernier échantillon) ; `start_time=0.000000` dans `ffprobe` ; `handler_name=Mix complet` / `Micro` |
| Suivi du périphérique par défaut à chaud | ✅ | `OnDefaultDeviceChanged` → réouverture ; débranchement ci-dessus ; changement manuel du défaut vu par `probe-audio` (réouverture en ~210 ms) |
| CLI `capture-av --seconds N --out f.mp4` | ✅ | 3 flux de durée identique (10,000 s, 45,000 s, 60,000 s, 160,000 s) |
| Décalage de synchro par piste (AUD-11, moteur) | ✅ | Config + validation (test), `--mix-offset-ms` ; Voicemeeter +117 ms → +16,9 ms avec −100 ms |

## Exigences SPEC couvertes

AUD-01, AUD-02 (périphérique, gain, suivi du défaut ; choix dans l'UI en phase 5), AUD-08, AUD-10, AUD-11 (moteur ; assistant de calibrage en phase 5).

## Écarts et corrections

- **Voicemeeter** : son capturé ~100 ms en retard sur l'image, non exposé par Windows
  (`GetStreamLatency` = 0) → AUD-11 ajouté à la SPEC sur décision de Palawi (ADR-017).
- **Fin de piste** : dépassement de quelques ms quand le périphérique horodate en avance →
  délai de grâce compté depuis la fin, signal d'arrêt 100 ms avant (`b461dfc`).
- **Écran capturé par erreur** : les noms GDI `\\.\DISPLAYn` ont été renumérotés pendant la
  session ; une capture de 160 s a pris l'écran de Discord de Palawi au lieu de VLC. Fichier
  supprimé aussitôt ; mesures refaites avec `--target primary`. Noté dans `RESEARCH.md` pour
  CAP-03.
- **Script T2** : priorité de la virgule sur `*` en PowerShell (120 émetteurs au lieu de 3) ;
  corrigé avant commit, essai à blanc refait.
- **Chaîne micro de Palawi** (Voicemeeter → LightHost → VB-Cable) : +473 puis +410 ms d'un
  lancement à l'autre, stable pendant un lancement ; hors moteur, compensé par AUD-11 ;
  assistant de calibrage relançable en un clic en phase 5 (`RESEARCH.md`). Retard au-delà de
  ±400 ms → `measure-av-sync.py` apparie les bips jusqu'à +800 ms (`1da1d79`).
- **Pente commune aux deux pistes** : −8,4 ms en 1 h sur le mix comme sur le micro (+4,7 →
  −3,7 ms), dans la tolérance. Même pente sur deux périphériques différents, dont le micro
  sans aucune correction de dérive : la cause est plutôt côté vidéo ou horloge partagée. À
  surveiller pour les enregistrements de plusieurs heures (≈ −25 ms à 3 h), noté dans
  `HANDOFF.md`.
- Journal du T2 : mix 68 corrections de dérive et 140 ms de chevauchement coupé ; micro
  0 correction et 400 ms coupés ; aucun silence comblé.
