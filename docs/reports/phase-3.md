# Phase 3 — Pistes par logiciel

- Date : 2026-10-16
- Commit : `c55a616` (`feature/per-app-audio`, PR #10)
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF ; sortie par défaut « Voicemeeter Input »

**Statut : ✅ validée.**

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Test d'isolation (T3) : deux applis jouent deux sons différents → chacun sur sa piste, rien ailleurs (sauf piste 1) | 👤 | ✅ | Zen (page Web Audio 440 Hz, lancée par un clic de Palawi) + VLC (`tone880.wav`) ; `capture-apps --app zen.exe --app vlc.exe --seconds 30` ; `measure-tones.py --hz 440 --hz 880` à chaque seconde : piste `zen` 440 Hz −11 dB et aucun 880 Hz, piste `vlc` 880 Hz −39 dB et aucun 440 Hz, mix les deux. Journal : `docs/reports/phase-3/t3-zen-vlc.txt`. Variante automatique (deux copies de la CLI, `play-tone` vers une sortie sans haut-parleurs) : fuite de l'autre fréquence < −70 dB, seulement aux transitions (`t3-auto-tones.txt`) |
| Test sur le Windows 11 à jour de Palawi : pistes non muettes (`ffmpeg astats`) | 👤 | ✅ | Build 26200 : `astats` piste `zen` crête −13,9 dB / RMS −17,0 dB, piste `vlc` crête −41,5 dB / RMS −45,1 dB (volume propre de VLC bas) ; `probe-audio --app` : crête de la piste = crête de la session (0,250). Le problème de piste muette signalé avec NAudio ne se reproduit pas |
| Fermer/relancer Discord pendant la capture : la piste se rattache, fichier valide | 👤 | ✅ | `capture-apps --app Discord.exe --seconds 45` ; Discord tué à +10 s, relancé à +20 s (`Update.exe --processStart`) : `application rattachée … PID 27164` puis `… PID 32712` 1 s après la relance ; `ffprobe` : 3 flux de 45,000 s ; `ffmpeg -v error -f null -` : aucune erreur. Journal : `docs/reports/phase-3/discord-reattach.txt` |

## Tâches

| Tâche | Statut | Preuve |
|---|---|---|
| `sources` : sessions audio actives + crêtes, processus, appli au premier plan | ✅ | `sources.rs` (Toolhelp + dates de création, sessions de toutes les sorties, `GetForegroundWindow`) ; 6 tests (racine par exe, arbre, cycles, PID parent réattribué) ; `list-audio-sessions` |
| Process loopback sur PID parent, `include_tree = true`, une piste par appli | ✅ | `AudioSource::Application`, `new_application_loopback_client(pid_racine, true)` ; T3 ci-dessus |
| Rattachement automatique par nom d'exe (attente → silence → reprise) | ✅ | Handle de processus attendu (le flux continue d'envoyer des zéros après la fermeture) ; Discord ci-dessus ; `tone-b` fermé puis relancé : silence de 10 à 14 s puis reprise, pistes de 30,000 s |
| Piste « Jeu » automatique | ✅ | `AudioSource::Foreground` ; `capture-apps --game` avec Palawi passant Zen → terminal → Zen : 440 Hz de 0 à 11 s, rien de 12 à 20 s, 440 Hz de 22 à 29 s, 3 rattachements (`docs/reports/phase-3/game-track.txt`) |
| Détection piste muette anormale (`app_track_silent`) | ✅ | `SilenceWatch` + 3 tests (alerte une fois, jamais sans signal de session, réarmée après du signal) ; deux fausses alertes trouvées pendant le test de la piste Jeu et corrigées (voir Écarts) : plus aucune au dernier essai |
| CLI `list-audio-sessions` et `capture-apps --app … ` | ✅ | Sorties ci-dessus ; aussi `probe-audio --app`, `play-tone` |

`todo!()` / `EngineError::NotImplemented` : aucun usage dans le code (la variante reste définie dans `error.rs`).

## Exigences SPEC couvertes

AUD-03 (moteur ; plafond 6/12 validé par la config), AUD-05 (titre de piste écrit dans les métadonnées ; renommage dans l'UI en phase 5), AUD-06, AUD-07, AUD-09.

## Écarts et corrections

- **Fausse alerte `app_track_silent`** sur le terminal qui lançait la CLI : un client de
  capture loopback crée une session de rendu « active » dont le vumètre reproduit la sortie.
  PalaClips ignore ses propres sessions. Limite connue (ARCHITECTURE) : une appli suivie qui
  enregistre elle-même la sortie en loopback peut déclencher l'alerte.
- **PID parent réattribué** (Toolhelp) : un lien parent → enfant n'est gardé que si le parent
  est plus ancien que l'enfant.
- Un vrai cas positif d'`app_track_silent` (piste muette alors que l'appli joue) n'a pas pu
  être provoqué : la logique est couverte par les tests unitaires.
- Après un rattachement, ~100 corrections de dérive d'un échantillon (≈ 2 ms), inaudibles
  (RESEARCH).
- Pas de `sysinfo` : Toolhelp suffit (ARCHITECTURE mise à jour).
