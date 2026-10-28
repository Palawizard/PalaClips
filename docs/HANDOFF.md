# HANDOFF — où on en est

> Mis à jour à la fin de **chaque** session (`/handoff`). Une nouvelle session doit pouvoir
> reprendre en lisant uniquement `CLAUDE.md` + ce fichier.

## État actuel

- **V1 publiée :** v1.0.0 le 2026-10-21 (https://github.com/Palawizard/PalaClips/releases/tag/v1.0.0),
  `dev` mergée dans `main` (#88), tag `v1.0.0` sur `main`. Phase 8 terminée ; revue finale
  `docs/reports/release-v1.md` : chaque exigence de la SPEC a sa preuve. Avant la release, Palawi a demandé (2026-10-21) un nouveau logo (#83, triangle de
  lecture en 3 bandes), une feuille de route publique (#84, `ROADMAP.md` / `ROADMAP.fr.md`) et un
  premier lancement en questions (#85) : tout est mergé, captures du README refaites.
  Branche : `dev`.
- **Piste 1 (ADR-029, ADR-030)** : Mix complet = somme des pistes choisies (micro si « Aussi
  dans Mix complet », Jeu, applis) ; « Audio du bureau » (désactivé par défaut) = toute la
  sortie + micro. La CLI garde la sortie entière (`--tracks-mix` pour le nouveau mode).
- **Socle i18n (APP-08, ADR-028)** mergé (#37) avant la phase 6 : clés
  `app/src/locales/fr.json` / `en.json`, `t()` / `useT()` de `app/src/lib/i18n.ts`, alertes et
  erreurs Rust en `Localized { code, params }`, langue dans Réglages > Général, `npm test`
  refuse les chaînes en dur. Anglais vu par Palawi dans l'app isolée.
- **v1.1.0 (2026-10-22)** : encodage par le processeur (ENC-05 révisé, ENC-06, ADR-035,
  rapport `docs/reports/cpu-encoder.md`). PR #92 (recherche, choix D de Palawi : `h264_mf` +
  SVT-AV1), #93 (moteur), #94 (app, docs), #95 (version 1.1.0, CHANGELOG, notes), #96 (essai
  de mise à jour), #97 (`dev` → `main`, accord de Palawi), tag `v1.1.0` sur `main`,
  **publiée le 2026-10-22** (https://github.com/Palawizard/PalaClips/releases/tag/v1.1.0) avec
  l'accord de Palawi. Vérifié sans connexion : `latest.json` 1.1.0, installeur, `.sig`,
  CHANGELOG et page en 200 ; la 1.0.0 installée de Palawi propose la 1.1.0.
- **v1.1.1 publiée le 2026-10-22** à la demande de Palawi, **sans le test VM** : #99 (mémoire de
  l'encodeur H.264 de Windows ~170 Mo en 1080p au lieu de ~720, bascule AV1 sans Media
  Foundation), #101 (version), #102 (`dev` → `main`), tag `v1.1.1`. Vérifié sans connexion après
  1 min : `latest.json` 1.1.1, installeur, `.sig`, CHANGELOG et page en 200. Reste : test VM T11.
  Branche : `dev` (PR de handoff `docs/handoff-v1.1`).
- **v1.2.0 en préparation (2026-10-23/24)** : recherche et décisions de Palawi (#105,
  `docs/reports/v1.2-research.md`), pages Capture et Audio + alerte de sortie (#106, ADR-036),
  enregistrer après la sauvegarde (#107, ADR-037), compression à une taille cible (#108,
  ADR-038), retours de la session T14 (#109 : Compression avant Réglages, tailles en unités
  binaires, copie sans réencodage, « Choisir des clips »). Session T14 de Palawi validée le
  2026-10-24. Préparation (#110), `dev` → `main` (#111, accord de Palawi), tag `v1.2.0`,
  **publiée le 2026-10-24** (https://github.com/Palawizard/PalaClips/releases/tag/v1.2.0) avec
  l'accord de Palawi. Brouillon relu connecté (version, signature = `.sig`, liens publics) ;
  vérifié sans connexion 1 min après : `latest.json` 1.2.0, installeur, `.sig`, page et
  CHANGELOG en 200. Branche : `dev`.
- **v1.3.0 (2026-10-24/26)** : bug « réglages pris seulement après une relance » (zip de Palawi :
  copies figées côté front, #113, ADR-039) ; recherche et mesures (`docs/reports/v1.3-research.md`),
  décisions de Palawi : 2e encodage pour l'enregistrement, quatre réglages séparés, REC-04 en
  clip à part, pistes sans maximum avec note dès la 13e (#114, ADR-040) ; estimation de la taille
  (RPL-11) ; accès direct (APP-15) ; son d'enregistrement ; session T15 validée ; préparation
  (#115), `dev` → `main` (#116), tag `v1.3.0`, **publiée le 2026-10-26**
  (https://github.com/Palawizard/PalaClips/releases/tag/v1.3.0) avec l'accord de Palawi. Brouillon
  relu connecté (version, signature = `.sig`, liens publics) ; vérifié sans connexion après
  publication : `latest.json` 1.3.0, installeur, `.sig`, page et CHANGELOG en 200. Branche : `dev`.
- **v1.3.1 (2026-10-26)** : sons de PalaClips dans la piste 1 en mode « Audio du bureau »
  (vérifiés par la CLI ; exclusion de notre PID mesurée : sons retirés, mais toutes les sorties
  captées et 61 ms d'avance sur le loopback Voicemeeter ; choix de Palawi : source au choix,
  « Tous les sons sauf PalaClips » par défaut, #118, ADR-041) ; listes de clips à jour partout
  (#119) ; déplacement dans le lecteur depuis les pistes et calage (#120) ; session de Palawi
  validée ; préparation (#121), `dev` → `main` (#122, accord de Palawi), tag `v1.3.1`,
  **publiée le 2026-10-26** (https://github.com/Palawizard/PalaClips/releases/tag/v1.3.1) avec
  l'accord de Palawi. Brouillon relu connecté (version, signature = `.sig`, liens publics) ;
  vérifié sans connexion après publication : `latest.json` 1.3.1, installeur, `.sig`, page et
  CHANGELOG en 200. Branche : `dev`.
- **v1.4.0 en préparation (2026-10-27/28)** : six retours de Palawi, une PR chacun : défilement
  d'État et 12 clips (#125), cellule Source = vraie source (#126, `SourceMode`, `source_changed`),
  glisser fluide dans le lecteur (#124, `ScrubSession`), son « clip terminé » (#127,
  `clip_saved.origin`), Replay & qualité en onglets (#128), Mix complet coupé à la lecture et
  état par clip (#129, `player.mute_full_mix`, colonne `mix`). Décisions et mesures :
  `docs/reports/v1.4.md`. Session manuelle de Palawi validée le 2026-10-28. Préparation de la
  version : branche `chore/release-v1.4.0`. Reste : PR `dev` → `main`, tag et brouillon **avec
  l'accord de Palawi**, puis publication avec son accord.
- **Dernière mise à jour :** 2026-10-28

## Ce qui existe

- Workspace Cargo (`crates/engine`, `crates/cli`, `app/src-tauri`), edition 2024.
- Moteur : `clock` (`Hns`), `config` (`EngineConfig`), `events` (`EngineEvent`), `error`,
  `replay` (`PacketRing`, estimation RAM, `plan_clip`, `ReplayPacket`/`Spool`), `engine`,
  `paths`, et `encode` (Windows) :
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
- **Avant phase 3 (PR #9)** : la piste comble le silence toutes les 50 ms quand le
  périphérique n'envoie rien ; `capture-av --mix-device/--mic-device`, `probe-av-emitter
  --device` ; `measure-av-sync.py` donne la pente (ms/h) par régression ; test sans
  surveillance `scripts/av-drift.ps1` ; règle « tests longs sans surveillance » (`CLAUDE.md`).
- **Phase 3 (`feature/per-app-audio`, PR #10)** :
  - `sources` : `processes()` (Toolhelp + dates de création, liens parent réattribués
    retirés), `root_pid`, `process_tree`, `foreground_pid`/`foreground_app`,
    `audio_sessions` (toutes les sorties), `session_peak(root)` (sans les sessions de
    PalaClips).
  - `AudioSource::Application { exe_name }` (process loopback de l'arbre, rattachement par
    nom d'exe, fin suivie par handle de processus) et `AudioSource::Foreground` (piste « Jeu ») ;
    `app_track_silent` (`SilenceWatch`) ; `AudioTrackStats::silent_alerts`.
  - CLI : `list-audio-sessions`, `capture-apps`, `probe-audio --app`, `play-tone` ; script
    `measure-tones.py`. Preuves `docs/reports/phase-3/`.

- **Piste Jeu redéfinie (PR #12, ADR-019)** : `sources::CurrentGame` (processus fixé >
  fenêtre capturée > dernière appli plein écran ou sans bordure), `ForegroundWatch`
  (`SetWinEventHook`), `AudioSource::Game(CurrentGame)`, `AudioSettings::game_exe`,
  `capture-apps --game-exe`, `play-tone --window`, `scripts/game-track.ps1` (40 s). Réutiliser
  `CurrentGame` pour le nom des clips (STO-03) en phase 4.
- Mesures : `probe-audio --device` (pente réception − `pu64QPCPosition`), `play-tone
  --amplitude`, `scripts/clock-drift.ps1` (PR #11).

- **Phase 4 (`feature/replay-engine`)** : `engine` (`Engine`, `EngineHandle` : `arm`,
  `disarm`, `save_replay`, `configure`), `replay::plan_clip`, `ReplayPacket` / `Spool` /
  `PacketReader` (RAM ou segments disque), `paths` (dossiers connus, nom des clips, mémoire
  privée), `StorageSettings::clips_dir`, `EngineError::{Stopped, NothingToSave}`, CLI
  `replay`, scripts `replay-save.ps1` (20 sauvegardes) et `replay-ram.ps1` (2 h).
  `VideoCapture::start` maintient le MTA COM (`keep_mta_alive`).
- **Phase 5, socle (`feature/app-shell`)** :
  - Tauri (`app/src-tauri/src/lib.rs`, `commands.rs`) : `EngineSlot` (config, `Engine`,
    `EngineHandle`, `AppStatus`) géré par l'app ; fil `palaclips-events` qui relaie chaque
    `EngineEvent` sur l'événement `engine-event` et tient `AppStatus` (état, secondes/octets du
    tampon, alertes actives une par code) ; commandes `get_state`, `replay_arm`,
    `replay_disarm`, `replay_save`, `list_recent_clips { limit }`, `storage_status` ; replay
    armé au démarrage si `arm_on_startup` (config par défaut pour l'instant, pas encore de
    persistance) ; `Alt+F10` global (`tauri-plugin-global-shortcut` 2.4) → `save_replay`,
    alerte `hotkey_unavailable` citant l'overlay NVIDIA ; zone de notification (Ouvrir,
    Sauvegarder le replay, Quitter ; clic gauche = ouvrir) ; instance unique
    (`tauri-plugin-single-instance` 2.5) ; fermer la fenêtre la détruit, seul « Quitter »
    sort (`prevent_exit` si `code: None`) ; niveaux audio pilotés par la fenêtre (focus 20 Hz,
    sans focus 10 Hz, réduite/détruite : coupés).
  - Moteur : `audio::PeakMeter` (crête sans verrou), `AudioTrack::meter()`,
    `engine::LevelRate` + `EngineHandle::set_level_rate`, échéances stats/niveaux vérifiées en
    tête de boucle ; `encode::write_thumbnail` (dernière image clé → JPEG 480 px dans
    `%LOCALAPPDATA%\com.palawi.palaclips\cache\thumbnails\<clip>.jpg`, après fermeture du MP4) ;
    `clip_saved.thumbnail` ; `library` : `probe_clip` (durée, titres des pistes audio lus dans le
    fichier), `recent_clips(root, limit)`, `disk_space` ; `paths::thumbnail_path` ;
    `EngineState: Default`.
  - CLI : `recent-clips [--clips-dir] [--limit]` (JSON), `replay --levels`.
  - `ipc.ts` et `ARCHITECTURE.md` § IPC synchronisés.
- **Réglages, noms de jeux, confirmation, clips sans doublon (2026-10-17)** :
  - `app/src-tauri/src/settings.rs` : `settings.json` (`%APPDATA%\com.palawi.palaclips\`) =
    `{ schema_version, engine, feedback }`, écriture atomique, illisible → `.bak` + défauts +
    alerte `settings_reset`, format plus récent → défauts sans écraser ; `EngineSlot` garde
    `Settings` entier (`update` = enregistrer puis garder) ; commandes `set_config`,
    `reset_config`, `get_feedback`, `set_feedback`.
  - `engine::games` (ADR-024) : nom du jeu = ProductName → FileDescription → nom d'exe nettoyé
    (suffixes Unreal, « Client ») ; génériques ignorés ; nettoyé pour le disque ; figé par exe
    dans `storage.game_names` (événement `game_named`) ; CLI `game-name`.
  - `feedback.rs` : carillon généré en mémoire, `PlaySoundW` async, indépendant du toast muet
    (`tauri-plugin-notification` 2.7) ; désactivables. Le carillon finit dans la piste Mix des
    clips suivants.
  - `replay::forget_saved` (RPL-09, ADR-025) : le clip suivant commence où le précédent s'est
    arrêté.
- **Raccourcis, démarrage avec Windows, écran Réglages (2026-10-17)** :
  - `hotkeys.rs` : actions `save_replay` (Alt+F10), `toggle_replay` (Ctrl+Alt+F10 ;
    Alt+Shift+F10 est pris par l'overlay NVIDIA chez Palawi) ; refus : sans modificateur,
    raccourcis système, touche Windows, Maj + lettre, doublons ; enregistrement avec retour
    arrière ; `get_hotkeys` (actions indisponibles), `set_hotkeys`, `suspend_hotkeys` /
    `resume_hotkeys` (pendant la saisie).
  - `tauri-plugin-autostart` 2.7 : `--autostart` = zone de notification, aucune fenêtre ni
    WebView2 ; `get_autostart` / `set_autostart`.
  - `settings.json` : `{ schema_version, engine, feedback, hotkeys, theme }` ;
    `reset_settings` (tout, autostart désactivé) ; `get_theme` / `set_theme`, `data-theme` posé
    par `lib/theme.ts`.
  - Front : `components/settings/` (`Settings` : sous-menu + `SettingRow`, `Switch`,
    `Segmented` ; `HotkeysSection` : saisie par combinaison, lettres via `event.key`
    (AZERTY), bord ambre si pris ; `GeneralSection`) ; brief
    `.impeccable/surfaces/app-src-components-settings-tsx.md` ; revue impeccable : 2 tours,
    restes : confirmation de réinitialisation et Raccourcis à 900×600 vérifiés en code seulement.
  - Moteur : moins de 3 s à sauvegarder → pas de clip, alerte d'info `clip_too_short`.
- **Écran État + shell (front, `feature/app-shell`)** : `app/src/App.tsx` (rail État / Galerie
  / Réglages ; Galerie et Réglages = textes d'attente), `components/` : `StatusBanner` (replay :
  état, chrono « en mémoire », RAM, Sauvegarder + Armer/Désarmer ; enregistrement désactivé
  jusqu'à la phase 6 ; source ; disque), `Alerts` (alertes actives, masquables), `Roster`
  (plaque numérotée à la couleur de la piste, vumètre, crête, état), `RecentClips` (5 derniers,
  miniature par `convertFileSrc`), `Rail` (logo en encre) ; `lib/useEngine.ts` (état +
  événements + actions), `lib/levels.ts` (vumètres : une boucle rAF partagée), `lib/tracks.ts`
  (couleur fixe par piste), `lib/format.ts`. Polices Barlow / Barlow Condensed (`@fontsource`,
  hors ligne), icônes `lucide-react` 1.52.0. Système documenté : `DESIGN.md` +
  `.impeccable/design.json`. Thème clair sous `prefers-color-scheme` (réglage Général à venir).
  Moteur : événements `capture_started` et `track_changed` (`TrackState`), `get_config` ;
  `AppStatus` porte la source et les pistes ; alertes d'échec du replay effacées au
  réarmement ; menu de la zone de notification avec Armer / Désarmer ; protocole `asset`
  limité à `cache\thumbnails`.
- **Mémoire (PR `fix/engine-memory-overhead`)** : NVENC `dpb_size=2`, `surfaces=2`/`delay=1`
  jusqu'à 60 i/s (ADR-022, −110 Mo) ; CLI `probe-memory` (mémoire étape par étape : tas,
  `VirtualAlloc`, image, blocs qui grandissent), `replay --no-arm` ; script
  `scripts/engine-memory.ps1` (étapes (a)–(d) de PERF-02, ~3 min, fenêtre ffplay et sons de
  test). PERF-02 mesuré : `docs/reports/phase-4.md` § PERF-02, `RESEARCH.md` § Mémoire du
  pipeline. PERF-01/02 en working set privé (ADR-023) ; qualité d'ADR-022 vérifiée sur
  Fortnite (`scripts/nvenc-quality.ps1`) ; estimation RAM (RPL-03) = fenêtre du tampon + 7 %
  (`replay::buffer_window`, `estimate_buffer_bytes`).
- **Encodage par le processeur (v1.1.0, 2026-10-22)** : `encode::video_encoders()` (matériel +
  `SOFTWARE_ENCODERS` = `h264_mf`, `libsvtav1`, `EncoderInfo.hardware`), `pick_encoder(…,
  EncoderChoice)`, `VideoSettings::encoder` (`auto` | `hardware` | `cpu`) ; `encode::readback`
  (`FrameConverter`, `SystemConverter` : Video Processor → NV12 → staging, ou BGRA + swscale) ;
  `encode::threads` (fils de l'encodeur en priorité basse) ; `capture::pacer::bounded_due` ;
  `SaturationWatch::cpu_encoder()` → `cpu_encoder_overloaded` ; `EngineError::NoVideoEncoder`.
  CLI : `--encoder auto|materiel|cpu`, `probe-encoder-threads`, `ffmpeg-info` (matériel/CPU).
  Scripts : `cpu-encoders.ps1`, `fps-impact.ps1 -Cpu`, `t2-av-sync.ps1 -VideoArgs`,
  `update-test.ps1 -Tag` (toute version). Front : ligne Encodeur (`ReplaySection.tsx`,
  `cpuVideo`, `cpuEncoderBytes`), premier lancement, `CPU_ENCODER_CODECS` / `encodesOnGpu`
  (`ipc.ts`).
- **Robustesse de la capture (phase 7, 2026-10-20, #64–#67)** : `capture::pacer::emit_due`
  (seule l'échéance la plus récente reçoit une image convertie, les autres répètent la dernière
  `AVFrame` encodée) ; `capture::set_gpu_priority(GpuPriority)` (`D3DKMTSetProcessScheduling
  PriorityClass` NORMAL ou REALTIME, appliquée à chaque démarrage de capture) et
  `VideoSettings::gpu_priority` (`game` par défaut | `capture`), ligne dans Réglages > Replay &
  qualité ; `capture/rate.rs` : hystérésis `Sustained` (5 s pour lever, 5 s pour retirer),
  `RateWatch` (G-Sync, retirée désormais), `SaturationWatch` (`capture_gpu_saturated`),
  `FreezeWatch` + `exclusive_fullscreen()` (`capture_exclusive_fullscreen`) ;
  `EngineEvent::AlertCleared { code }`, `events::CAPTURE_ALERTS` (retirées par le shell au
  démarrage et à l'arrêt de la capture) ; HDR de l'écran comparé chaque seconde, capture
  relancée s'il change ; `encode::compile_shader` / `blob_bytes` partagés. CLI :
  `probe-gpu-priority [--window]`, `gpu-load` (caché), `probe-fullscreen`, `--capture-priority`.
  Scripts : `app-crash.ps1`, `fps-impact.ps1 -CapturePriority`.

- **v1.2 (2026-10-23/24)** :
  - Front : barre État, Galerie, Capture, Audio, Compression, Réglages (`Rail.tsx`) ; garde de
    sortie `lib/leaveGuard.ts` (vitest) + `components/LeaveDialog.tsx`, `useConfigDraft(page)`
    déclare son brouillon ; `npm test` = test i18n + vitest. Page `components/compress/`
    (`CompressPage`, `ClipPicker`), store `lib/compressDrafts.ts` (révision par brouillon contre
    les réponses périmées) ; « Compresser » dans le menu de la galerie et la barre d'export du
    lecteur ; « Clip en cours » + « Prolonger » / « Terminer maintenant » sur État ; ligne
    « Après l'appui » dans Replay & qualité ; `format::bytes` en unités binaires.
  - Moteur : `replay.after_secs`, `PostRollCut` (`replay`), `Pipeline::start_post_roll` réutilise
    le `Recorder` (`RecordKind::Replay`) ; `EngineEvent::PostRoll`, `EngineHandle::finish_clip`,
    alerte `clip_cut_short`. `library::compress` (sonde, plan, graphe de filtres, essais, copie
    sans réencodage), `AacEncoder::open_with_bitrate`, erreurs `not_a_video`,
    `target_too_small`, `over_target`, `compress_cancelled`.
  - Shell : `compress.rs` (file d'attente sur un fil, `compress_*`), `replay_finish_clip`,
    `AppStatus.clip_ends_at_ms`, entrée « Terminer le clip maintenant » dans la zone de
    notification, permission `core:window:allow-destroy`.
  - CLI : `replay --after N` (touche `f`), `compress`. Scripts : `post-roll.ps1`,
    `post-roll-memory.ps1`, `compress-check.ps1`, `clip-continuity.py`. Preuves :
    `docs/reports/v1.2/`.

- **v1.3 (2026-10-24/26)** :
  - Config : le shell émet `config-changed` / `hotkeys-changed` depuis `EngineSlot::update` ;
    front : un seul store (`lib/config.ts` `createConfigStore`, `lib/useConfig.ts`
    `useLiveConfig`, `saveConfig`), `useConfigDraft` garde un patch (`configPatch` / `withPatch`,
    objets à `kind` et listes remplacés entiers) ; `set_config { patch }` fusionné par
    `settings::patched` ; cas de redémarrage communs `app/src/lib/restart-cases.json` (vitest +
    Rust, `needs_restart` public) ; le moteur journalise « réglages reçus ».
  - Moteur : `VideoCapture::add_output` → `VideoOutput` (2e encodeur, horloge alignée sur la
    grille principale, `Role::Extra`) ; `RecordSettings::video: Option<RecordVideo>`,
    `EngineConfig::record_video()` / `records_separately()` ; `Pipeline::request_separate_recording`
    (repli processeur, alerte `record_encoder_cpu`), `store_record`, REC-04 en clip à part ;
    pipeline démarré à la qualité de l'enregistrement si le replay est désarmé ; l'enregistreur
    ignore l'audio antérieur au début (sans découpage) ; `sources::recent_processes` (500 ms) ;
    `max_app_tracks` retiré ; `replay::estimate_file_bytes`, `ReplayEstimate.file_bytes_per_minute` ;
    compression : débit audio abaissé avant de refuser.
  - App : `ReplaySection` en trois parties (Replay, Enregistrement « Comme le replay », Commun) +
    bandeau d'estimation (`lib/useEstimate.ts`), ancre `#replay-quality` et lavis d'arrivée ;
    État : résumé de qualité cliquable (`QualityLink`), Capture : `QualityButton` ; Audio : note dès
    la 13e piste ; sons `feedback::Sound` (sauvegarde, début et fin d'enregistrement).
  - CLI : `capture-video --also-*`, `replay --record-*`, `estimate` (octets par minute). Scripts :
    `track-cost.ps1`, `dual-encoder.ps1`, `fps-impact.ps1 -Dual`, `record-quality.ps1 [-AvSync]`,
    `size-estimate.ps1`, `many-tracks.ps1 [-Keep]`, `app-isolated.ps1 -RightScreen`. Preuves :
    `docs/reports/v1.3/`.

- **v1.4 (2026-10-27/28)** :
  - Moteur : `events::SourceMode` (`capture_started.source_mode`, `source_changed` relevé chaque
    seconde dans le fil d'encodage, `capture::display::refresh_hz`) ; `events::SaveOrigin`
    (`clip_saved.origin` : `replay`, `post_roll`, `recording`) ; `chime::Sound::ClipFinished` ;
    `Library::mix` / `set_mix` (colonne `mix`, ajoutée aux bases existantes).
  - Shell : `player.mute_full_mix` dans `settings.json` (`get_player` / `set_player`),
    `clip_mix` / `set_clip_mix` (`ClipMix { tracks, full_mix_chosen }`, volume borné) ; son joué
    selon `origin` (rien pour un enregistrement, qui a son propre son de fin).
  - Front : `lib/scrubSession.ts` (glisser : dessin immédiat, chargement après 120 ms ou au
    relâchement, dernière position gagnante, audio tenu par `PlayerAudio.hold/release`) ;
    `lib/qualityTabs.ts` (onglets, onglet retenu, `Destination.tab`) ; `lib/playerMix.ts`
    (coupure du Mix complet seulement avec une piste Jeu ou appli, export sur l'état choisi).
  - Scripts : `cdp-run.ps1` (app isolée + script CDP, sortie dans `target/journals/<script>`),
    `scrub-measure.mjs`, `quality-tabs-check.mjs`, `player-mix-check.mjs`, `manual-session.ps1`
    (session de Palawi, clips sur F:, `-Stop` range tout), `app-isolated.ps1 -ClipsDir`,
    `readme-screenshots.mjs` mode `quality`.

## Prochaines actions (dans l'ordre)


1. **Clore la v1.1** : quand Palawi rapporte le test VM T11 (`docs/TESTING.md`, installeur de
   la release 1.1.1 dans `target\vm\PalaClips_1.1.1_x64-setup.exe` ; s'il a testé la 1.1.0,
   noter laquelle) : remplir la ligne
   VM de `docs/reports/cpu-encoder.md`, cocher les cases restantes de PLAN v1.1. Un défaut trouvé
   → branche `fix/…` depuis `dev`, puis 1.1.1 avec son accord (même chemin de release).

2. **Prochaine version** (seulement avec l'accord explicite de Palawi). Pistes : `ROADMAP.md`.
   1. Notes : `pwsh scripts/release-notes.ps1 -Tag dev` liste les PR visibles par l'utilisateur
      depuis le dernier tag (`feat`/`fix`/`perf`, sans scripts, tests, outils, docs, maintenance).
      En tirer `docs/release-notes/X.Y.Z.md` : `## Français` puis `## English`, quelques puces
      sur ce qui change pour l'utilisateur, lien vers `CHANGELOG.md` au tag `vX.Y.Z` (modèle :
      `1.0.0.md`). Ce texte devient la page de release et les notes de l'updater ; l'app n'en
      montre que la partie de sa langue (titres = `settings.general.languages`, depuis 1.0.1).
      Le workflow échoue si le fichier manque. Faire valider le texte par Palawi.
   2. Version X.Y.Z dans `Cargo.toml` (workspace), `Cargo.lock`, `app/src-tauri/tauri.conf.json`,
      `app/package.json`, `app/package-lock.json` ; section `## [X.Y.Z] - date` du CHANGELOG ;
      PR vers `dev`, puis PR `dev` → `main`, tag `vX.Y.Z` sur `main`.
   3. **Toujours en brouillon d'abord** : le workflow `Release` crée un brouillon. Le relire
      connecté (`gh release download vX.Y.Z -p latest.json`) : version, signature, et URL déjà
      sous la forme publique `https://github.com/Palawizard/PalaClips/releases/download/vX.Y.Z/…`
      (réécriture automatique de #89, à vérifier pour la première fois à la 1.0.1).
   4. Publier (`gh release edit vX.Y.Z --draft=false --latest`), puis tout de suite, **sans être
      connecté** (`curl`) : `releases/latest/download/latest.json` donne X.Y.Z et son URL
      télécharge l'installeur. Un brouillon n'est pas lisible sans connexion : cette vérification
      ne peut se faire qu'après publication. En cas d'échec, remettre en brouillon
      (`gh release edit vX.Y.Z --draft=true`), corriger, recommencer.
2. Clé de l'updater : générée par Palawi le 2026-10-20, secrets `TAURI_SIGNING_PRIVATE_KEY` et
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` posés ; clé publique dans `tauri.conf.json`. Claude ne
   lit jamais le fichier privé ni le mot de passe. Un build local avec installeur exige la clé :
   ajouter `--config '{"bundle":{"createUpdaterArtifacts":false}}'` pour les mesures locales.
3. Essai visuel : `pwsh scripts/app-isolated.ps1 -MainScreen` (ou `-LeftScreen` quand Palawi
   joue) `-Settings '{"language":"en","engine":{"replay":{"arm_on_startup":false}}}'`.
- ⚠️ Toujours : demander avant de lancer l'app (elle arme le replay) ; jamais de touches
  synthétiques ; tout essai par `scripts/app-isolated.ps1` (jamais le `settings.json` de
  Palawi).

## Bloquants

Aucun. (PERF-02 levé le 2026-10-16 : RAM mesurée en working set privé, ADR-023.)

## Décisions en attente de Palawi

Aucune. (2026-10-22 : encodeur CPU → D, `h264_mf` pour H.264 + SVT-AV1 pour AV1, ADR-035.)

## Points d'attention

- `app-isolated.ps1` : un `settings.json` partiel sans `schema_version` est lu comme une
  version 1 et passe par les migrations ; le script ajoute désormais la version courante (lue
  dans `settings.rs`).
- Sons de PalaClips : `palaclips_engine::chime` ; vérification `scripts/feedback-sounds.ps1`
  (détection par corrélation, `scripts/find-chimes.py`), sonde `palaclips-cli probe-exclusion`.
- Disque C: de Palawi presque plein (≈ 3,5 Go libres le 2026-10-26) : fichiers de test
  fragmentés (`fsutil sparse`) pour les gros fichiers.

- Le MCP DaVinci Resolve ne pilote pas l'édition gratuite de Palawi (pas de scripting externe) :
  un `launch` le laisse tourner en `-nogui` ; l'arrêter ensuite (Stop-Process, avec l'accord de
  Palawi). Tests Resolve = manuels.
- CDP : `Input.dispatchKeyEvent` Entrée n'active un bouton qu'avec `text` (`\r`).
- `clip-continuity.py` et le « premier pas » : un premier pas de 3–4 images révèle de l'audio
  écrit avant le début du fichier (corrigé dans l'enregistreur pour la 2e sortie).
- Bash : de gros heredocs Python contenant des triples apostrophes échouent (« unexpected EOF ») ; écrire le
  script dans le scratchpad puis l'exécuter.

- Glisser-déposer : il ne marche pas dans l'app de test placée par `PALACLIPS_TEST_WINDOW`
  (fenêtre créée cachée puis montrée sans focus, `show_without_focus`) ; il marche dans une
  fenêtre ouverte normalement (vérifié par Palawi le 2026-10-24). Pour un essai de dépôt, lancer
  l'exe avec `PALACLIPS_CONFIG_DIR` mais sans `PALACLIPS_TEST_WINDOW`.
- npm 10.9.4 plante en ajoutant vitest (`edgesOut`) : ajouter une dépendance avec
  `npx -y npm@11.6.2 install` ; `npm ci` (npm 10, CI) lit le verrou sans souci.
- MP4 hybride : le premier échantillon de chaque piste est ramené à 0 (première image ~13 ms de
  plus) ; `clip-continuity.py` ignore ce premier pas (RESEARCH § Enregistrer après la sauvegarde).
- Compression : ~0,8× le temps réel pour du 1440p60 (décodage H.264 sur le processeur) ; un
  replay sauvegardé pendant une compression reste continu.

- Encodage CPU (v1.1) : la mémoire de `h264_mf` dépend de son nombre de fils (16 par défaut →
  ~720 Mo en 1080p). Limité à 2/4 fils par `encode::mf::limit_mf_workers` (ICodecAPI lu dans
  `MFContext` juste avant `opt_enc_rc` : à revérifier si FFmpeg change `mfenc.c` ; en cas
  d'échec, simple avertissement dans le journal et défaut gardé). L'estimation de Réglages
  ajoute la mémoire de l'encodeur (`cpuEncoderBytes`, constantes mesurées). Sans Media
  Foundation (éditions N/KN), `pick_encoder` bascule le CPU sur SVT-AV1. Le VMAF standard est trompé par l'accentuation de `h264_mf` : utiliser
  `vmaf_v0.6.1neg`. SVT-AV1 écrit ses `Svt[info]` sur stderr (bruit dans la CLI, invisible
  dans l'app). Les fils de l'encodeur sont repérés par leur module de départ
  (`encode::threads`) ; `mfh264enc.dll` les crée à la 1re image, pas à l'ouverture.
- `fps-impact.ps1 -Cpu` : Fortnite plafonné à 225 i/s masque la perte moyenne (1 % bas −3 à
  −5 %) ; pour une perte de jeu limité par le CPU, limite d'images « Illimité » (Palawi a gardé
  le passage plafonné le 2026-10-22).
- Juste après une publication, `releases/latest` peut encore pointer sur l'ancienne version
  pendant ~1 min : la 1.0.0 de Palawi démarrée 36 s après la publication a dit « à jour », puis
  a proposé la 1.1.0 au clic suivant sur « Rechercher » (2026-10-22).
- `update-test.ps1 -Tag test-vX.Y.Z` prend la version dans le tag et met les vraies notes de
  `docs/release-notes/X.Y.Z.md` dans le flux local.

- Capture et fps du jeu (2026-10-17, `scripts/fps-impact.ps1`, journaux
  `docs/reports/phase-7/`) : aucune perte mesurable dans Fortnite plafonné à ~225 i/s, avec et
  sans G-Sync ; le jeu reste en Independent Flip (WGC ne force pas la composition). Rien à
  corriger côté capture. La perte de 50–100 i/s du 2026-10-16 venait **probablement** des
  réencodages et du VMAF lancés en parallèle (hypothèse, non vérifiée). PERF-03 GPU-bound
  validé le 2026-10-20 (Kombustor fenêtré, 0,7 % en priorité « jeu »).

- Horloge audio : `pu64QPCPosition` suit QPC à 0,00 ms/h (Apogee) et +0,32 ms/h
  (Voicemeeter) sur 20 min (`clock-drift.ps1`). Les pentes d'`av-drift.ps1` (−19,4 / −2,3 /
  −8,4 ms/h) viennent probablement du banc (non prouvé) : ne plus s'en servir pour une pente.
- Chaîne micro de Palawi : retard de +410 à +473 ms qui change à chaque relance de
  Voicemeeter/LightHost ; pour tout test micro, calibrer puis ne plus relancer la chaîne.

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
- HDR basculé pendant une capture : détecté chaque seconde, capture relancée au bon format
  (#67) ; le tampon de replay repart de zéro à chaque bascule (ADR-031).
- Sous GPU saturé en priorité « jeu », le rappel WGC attend le contexte D3D11 tenu par
  l'encodeur : ~15 images source/s (RESEARCH). Si un jour il faut mieux sans temps réel :
  piste = copier l'image WGC sans le verrou partagé (second contexte différé).
- Redimensionnements d'une fenêtre capturée : pas de fuite (`scripts/resize-leak.ps1`,
  plateau à ~41 Mo après 6 changements, #69). Une fenêtre immobile ne renvoie la nouvelle
  taille qu'au prochain changement de contenu. La CLI écrit ses journaux `tracing` sur la
  sortie **standard**.
- Plein écran exclusif : « presque figé » (< 5 images en 5 s) + `D3DKMTCheckExclusiveOwnership`
  ou `QUNS_RUNNING_D3D_FULL_SCREEN` (#69). Kombustor exclusif coupe le HDR de l'écran : la
  capture redémarre à sa fermeture (#67).
- `gpu-load --window` (CLI cachée) : charge GPU au premier plan (fenêtre 640×360) pour
  reproduire un jeu GPU-bound ; une charge sans fenêtre ne reproduit pas la famine (Windows
  favorise le processus au premier plan).
- Replay : mémoire privée +~1 Mo par sauvegarde pendant 20 sauvegardes (612 → 633 Mo) ; à
  surveiller (test de 2 h, ou une boucle de sauvegardes) avant de conclure à une fuite.
- Plusieurs sessions WGC dans un processus : `keep_mta_alive` (`CoIncrementMTAUsage`) est
  indispensable ; sans lui, crash à la 2e session après des pistes audio.
- TranslucentFlyouts et un mod Windhawk sont injectés dans nos processus chez Palawi : un
  « blocage » peut être un crash intercepté (boîte de TF + minidump dans
  `E:\Autre\Downloads\TranslucentFlyouts.Win32.V3.x64\dumps`). Analyse :
  `cdb -z <dump> -c ".ecxr; kn; lmu; q"` (cdb installé dans Windows Kits\10\Debuggers\x64).
- Fenêtres de test : `ffplay` de `third_party\ffmpeg\bin` (bruit `geq=random(1)` ou mire
  `testsrc2`) dans une fenêtre 1280×720 en bas à droite, capturée en `window:ffplay.exe`.
  `Start-Process -ArgumentList` ne met pas d'arguments entre guillemets : pas d'espaces.
- Suivi de la CI : `gh pr checks --watch` peut rendre la main avant que la CI du dernier push
  existe ; attendre le run du commit (`gh run list --commit <sha>`) puis `gh run watch`.
- Process loopback testé sur build 26200 : pas de piste muette. Le flux d'une appli fermée
  continue d'envoyer des zéros (fin suivie par handle de processus).
- Une capture loopback crée une session « active » dont le vumètre reproduit la sortie : ne
  jamais conclure qu'une appli joue d'après sa seule crête de session.
- Tests audio sans déranger Palawi : copies de la CLI (`target\tone-a.exe`, `tone-b.exe`)
  qui jouent `play-tone` vers `--device Realtek` (sortie sans haut-parleurs).
- Discord a été lancé pendant le test de rattachement (il ne tournait pas avant).
- L'app (`tauri dev`) arme le replay dès le démarrage avec la config par défaut : écran
  principal, micro par défaut, mix et piste Jeu. Toujours prévenir Palawi avant de la lancer.
- Tauri 3 est en alpha sur crates.io : ne pas suivre les suggestions de mise à jour vers 3.x.
- Fin de tâche : merger soi-même la PR vers `dev` quand la CI est verte, puis supprimer la
  branche (`CLAUDE.md` § 4 Git, « Merge et ménage »).
- Session Windows propre (compte local temporaire) abandonnée par Palawi le 2026-10-20 : elle
  partage WebView2, pilotes et bibliothèques avec sa session. Remplacée par l'app isolée avec un
  profil vierge, l'essai de mise à jour automatisé et la VM (T10).
- Essai de mise à jour (`scripts/update-test.ps1`, T8) : il installe vraiment dans
  `%LOCALAPPDATA%\PalaClips` puis désinstalle ; il refuse de tourner si une installation existe.
  Ce dossier existait **vide** chez Palawi (reste de la phase 7) : l'essai le recrée vide. Une
  app isolée ne demande pas de relance à l'installeur (`restart_after_install(false)`) : la
  relance de NSIS passe par l'Explorateur et perdrait l'environnement isolé, donc les vrais
  réglages. Le plugin updater refuse le `http` en release sauf `dangerousInsecureTransportProtocol`,
  activé seulement dans la build 1.0.0 de l'essai (fichier `--config`).
- La ligne « démarrage de PalaClips » du journal porte la version du crate moteur (0.1.0 tant
  que la release n'a pas monté la version du workspace), pas celle de Tauri.
- Bancs ajoutés en phase 8 : `multi-instance.ps1` (deux instances d'un exe sur une piste),
  `shared-encoder.ps1` (sessions NVENC replay + enregistrement), `readme-screenshots.ps1`
  (captures du README par CDP dans l'app isolée), `update-test.ps1`.
- Installeur pour un essai hors de la machine de dev sans release publique : tag `test-v1.0.0`
  sur `dev` (brouillon « TEST »), `gh release download`, puis `gh release delete test-v1.0.0
  --cleanup-tag -y`. Un brouillon n'est téléchargeable qu'avec le compte de Palawi.
- Écran mémorisé par `monitor_path` (#79) : un réglage enregistré avant garde le nom GDI ; il
  passe au chemin stable dès que l'écran est rechoisi dans Réglages > Capture.

- Push forcé refusé par le mode auto (rebase d'une branche déjà poussée) : merger `origin/dev`
  dans la branche à la place, le push reste en avance rapide.
- Clips de test faits à la main : écrire `handler_name` en plus de `title` (l'app lit
  `handler_name`, sinon toutes les pistes s'appellent « SoundHandler »). ffprobe montre le titre
  MP4 sous `tag:name`, pas `tag:title`.
- La build FFmpeg LGPL n'a pas libx264 : clips de test en `h264_nvenc`.
- Palawi garde souvent son PalaClips installé ouvert : `app-isolated.ps1` refuse alors de lancer
  l'app de test ; lui demander de le quitter, et lui dire à la fin de le relancer.

## Journal des sessions

| Date | Phase | Fait | Suite |
|---|---|---|---|
| 2026-10-15 | — | Création du repo, docs, squelette, CI | Phase 0 |
| 2026-10-15 | 0 | Règles Git `main`/`dev` (PR #1) ; env Windows vert, rsmpeg 0.18 ↔ FFmpeg 8.1 (bindgen 0.72.1), `ffmpeg-info`, `PRODUCT.md`, direction visuelle ; phase 0 validée (PR #2) | Phase 1 |
| 2026-10-15 | 1 | Pipeline vidéo GPU (WGC → Video Processor NV12 → NVENC/AMF/QSV), pacer CFR QPC, repli, préréglages de débit, CLI `capture-video` ; PR #5. Tests 👤 avec Palawi : Fortnite ✅, HDR délavé → correction FP16 + shader (`fix/hdr-capture`) ✅ ; phase 1 validée | Phase 2 |
| 2026-10-15 | 1→2 | Suites phase 1 (PR #7 mergée : HDR par `activeColorMode`, atténuation des hautes lumières, horloge après le 1er encodage, 240 i/s, limite G-Sync). Phase 2 : `probe-audio`, `AudioTrack`/`GapFiller`/AAC/mux multi-pistes, `capture-av`, débranchement ✅, Voicemeeter +100 ms → AUD-11 (décalage par piste), T2 automatisé ; PR #8 ouverte | T2 1 h par Palawi, puis merge et phase 3 |
| 2026-10-16 | 2 | CAP-11 + ADR-018 (alerte G-Sync, code en phase 7) ; chaîne micro Voicemeeter/LightHost instable d'un lancement à l'autre (hors moteur, assistant de calibrage en un clic en phase 5) ; T2 d'une heure 6 × `OK` ; phase 2 validée, PR #8 mergée | Phase 3 |
| 2026-10-16 | 2→3 | PR #9 : silence comblé quand le périphérique n'envoie rien, pente A/V par régression, `av-drift.ps1`, règle des tests longs. Phase 3 : `sources`, pistes par appli, rattachement, piste Jeu, `app_track_silent`, CLI ; T3 (Zen + VLC), Discord, piste Jeu avec Palawi ; 2 fausses alertes corrigées ; phase 3 validée (PR #10) | Test de dérive Apogee, puis phase 4 |
| 2026-10-16 | 2→4 | PR #11 : horloge audio contre QPC mesurée (0,00 ms/h Apogee), hypothèse de dérive d'appareil infirmée, AUD-08 maintenu ; règles « test en arrière-plan » et « ≤ 5 min sans accord ». PR #12 : piste Jeu = jeu en cours (ADR-019, `SetWinEventHook`), preuve Alt+Tab. PR #13 : phase 4 (moteur, replay RAM/disque, nom par jeu, redémarrage, CLI `replay`), 20 sauvegardes 146–200 ms, crash COM à la 2e capture corrigé ; Resolve n'affiche pas les noms de pistes → bloquant | Décisions de Palawi : test 2 h, noms Resolve |
| 2026-10-16 | 4→5 | RPL-08 en test accéléré (ADR-020, corrigé : mise en régime de 10 min exclue) : 30 min, 360 sauvegardes, −1,8 Mo/h ✅ ; noms de pistes dans le fichier seulement (ADR-021) ; phase 4 validée ; `/impeccable shape` de l'écran État + shell, brief confirmé avec 7 corrections | Phase 5 : écran État + shell |
| 2026-10-16/17 | 4→5 | PERF-02 mesuré (a–d) : NVENC défauts ~175 Mo → ADR-022 (`dpb_size=2`, pipeline court ≤ 60 i/s, −110 Mo, qualité identique sur Fortnite) ; PERF-01/02 en working set privé (ADR-023, 61 Mo hors tampon) ; RPL-03 = fenêtre + 7 % ; PR #16, #17. Perte de 50–100 i/s dans Fortnite pendant une capture → `fps-impact.ps1` (PR #18), test en attente. Phase 5 : socle Tauri (événements, commandes replay, `Alt+F10`, zone de notification, instance unique), niveaux audio, miniatures, derniers clips, espace disque | Test fps avec Palawi ; écran État (`/design`) |
| 2026-10-17 | 5 | `fps-impact.ps1` lancé par Palawi (avec et sans G-Sync) : aucune perte de fps, Independent Flip conservé ; script corrigé (mise en route non comptée, GPU relevé) ; PERF-03 GPU-bound ajouté au PLAN | Écran État (`/design`) |
| 2026-10-17 | 5 | `track_changed` / `capture_started`, `get_config` ; écran État + shell (Barlow, roster à plaques de couleur, vumètres 20 Hz interpolés, derniers clips avec miniatures) ; revue impeccable : recapture, 8 correctifs + logo, verdict « ship » ; `DESIGN.md` par le documenteur ; vérifié en vrai : 1180×760, 900×600, thème clair, sauvegarde, 0 WebView2 fenêtre fermée, instance unique | Montrer l'écran à Palawi ; persistance des réglages, toasts, écrans Réglages |
| 2026-10-17 | 5 | Vumètres indexés par piste, animation plafonnée à 60 i/s et sans boucle hors focus ; PERF-03 mesuré en release (fenêtre ouverte sans focus 0,57 % CPU, fermée 0,27 %) ; test de Palawi : écran OK, `Alt+F10` dans Fortnite sans perte de focus, sauvegarde fenêtre ouverte OK ; consigne de copie dans PRODUCT.md ; PR #19 mergée | Réglages persistants, noms de jeux, toast + son |
| 2026-10-17 | 5 | PR #21 réglages persistants ; PR #22 noms de jeux (Fortnite sans métadonnées → nom d'exe nettoyé, décision de Palawi, ADR-024) ; PR #23 carillon + toast, test Fortnite ✅ (carillon en jeu, toast bloqué en jeu puis dans le centre de notifications, toast + carillon au bureau, dossier « Fortnite ») ; PR #24 clips sans doublon (RPL-09, ADR-025, 20,2 s puis 10,2 s) | Raccourcis, démarrage avec Windows, écrans Réglages |
| 2026-10-17 | 5 | PR #25 clip < 3 s ignoré ; PR #26 démarrage avec Windows (0 WebView2) ; raccourcis personnalisables (conflit réel détecté : Alt+Shift+F10 pris par NVIDIA → défaut Ctrl+Alt+F10) ; écran Réglages (Raccourcis, Général, thème) testé par Palawi au clavier ; revue impeccable 2 tours | Sections Réglages restantes |
| 2026-10-17 | 5 | Socle i18n avant la phase 6 (ADR-028) : fr/en complets, langue de Windows par défaut, codes + paramètres côté Rust, menu de la zone de notification et toasts traduits, test anti-chaînes en dur | Revue de Palawi (anglais + sélecteurs), phase 6 |
| 2026-10-18 | 6 | Noms de pistes par défaut hors du fichier de réglages (#39) ; enregistrement manuel sur la capture du replay en MP4 hybride (#40), récupération au démarrage et arrêt propre disque plein (#41) ; bibliothèque SQLite, surveillance du dossier, quota (#42) ; protocole `palaclips://` par plages et pistes extraites (#43) ; découpe rapide ou précise, export pistes choisies ou mixées (#44) ; galerie et lecteur multipiste (#45) | Mesures de la phase 6 |
| 2026-10-19 | 6→7 | Mesures de la phase 6 et fuite du lecteur corrigée (#46) ; micro dans la piste 1 (#47, ADR-029), Mix complet = somme des pistes (#48, ADR-030), pistes en retard attendues (#50) ; phase 6 validée (#49) ; alerte de cadence DWM (#51), reprise après panne du pipeline (#52, ADR-031), GOP fixe des vidéos de test (#53), secondes sans image ignorées (#54) ; panneau de performances (#55) ; WebView et données isolées (#56, #58), données sous l'identifiant de l'app (#57, ADR-032), FFmpeg et licences dans l'installeur (#59), preuves de non-migration et désinstallation (#60–#62) ; séance T7 (#63) | Défauts de la séance T7 |
| 2026-10-20 | 7 | Défaut 1 : famine GPU reproduite seulement avec une charge au premier plan, seule la classe REALTIME aide (sans admin avec HAGS), file bornée (11 s → 57 ms), alerte GPU saturé (#64) ; défaut 3 : `alert_cleared` (#64) ; défaut 2 : alerte plein écran exclusif, Kombustor (#65) ; PERF-03 : 7,5 % en temps réel contre 0,7 % en normal → réglage, « jeu » par défaut (#66, ADR-033) ; T7 appli qui crashe (script), résolution, HDR basculé (#67) ; phase 7 validée | Minecraft quand Palawi le dit, phase 8 |
| 2026-10-20/21 | 8 | Journaux + diagnostic anonymisé (#71) ; premier lancement en 6 étapes, 27,5 s jusqu'au clip (#72) ; relecture de l'anglais (#73) ; README EN/FR + captures (#74) ; bug Vesktop trouvé par Palawi : applis en plusieurs instances additionnées (#76) ; Windows 11 requis (#77) ; `NotImplemented` supprimé (#78) ; updater + workflow de release, essai 1.0.0 → 1.0.1 et fausse signature automatisés (#75) ; écran par chemin stable (#79) ; REC-02 et `dumpbin` prouvés ; session Windows propre abandonnée ; revue finale | T10 (VM), Minecraft, release avec l'accord de Palawi |
| 2026-10-21 | 8 | Release v1.0.0 : notes de release automatiques (PR par type), README EN/FR à la structure classique et SEO (description, 20 topics, page d'accueil du dépôt), version 1.0.0 (#87), `dev` → `main` (#88), tag, installeur signé + `latest.json` vérifiés, release publiée ; URL de l'updater corrigée à la main puis automatisée | Suivre les issues ; prochaine version avec l'accord de Palawi |
| 2026-10-21 | 8 | Logo « Pistes » choisi par Palawi parmi 3 propositions (#83) ; feuille de route (#84) ; premier lancement en 8 questions, une à la fois, cartes « Conseillé », récapitulatif modifiable, vérifications seulement en cas de problème (#85), vu deux fois par Palawi ; RAM gardée par défaut jusqu'à 2 min (décision de Palawi, RPL-04 inchangé) ; captures FR/EN refaites | Release avec l'accord de Palawi |
| 2026-10-22 | v1.1 | Encodage par le processeur : recherche (encodeurs du FFmpeg BtbN, licences, lecture, banc VMAF NEG), choix D de Palawi ; moteur (`h264_mf` + SVT-AV1, Video Processor → NV12 → staging ou swscale, fils en priorité basse, file bornée, découpe précise CPU) ; app (réglage Encodeur, premier lancement, panneau), critique/audit impeccable ; validé : clip CPU, A/V 3,7 ms d'écart avec NVENC, Fortnite 0 % (1 % bas −3 à −5 %), mise à jour 1.0.0 → 1.1.0 ; mémoire de `h264_mf` (~720 Mo en 1080p) gardée par Palawi ; release v1.1.0 publiée (accord de Palawi), vérifiée sans connexion, sa 1.0.0 propose la 1.1.0 | Test VM T11 par Palawi |
| 2026-10-22 | v1.1.1 | Mémoire de `h264_mf` : nombre de fils (16 images en vol), pas une file ; faible latence et B-frames sans effet ; limité à 2/4 fils par ICodecAPI (+244 Mo au lieu de +773 en 1080p30, moins de CPU, même VMAF) ; bascule AV1 sans Media Foundation (N/KN), FAQ ; v1.1.1 publiée sans le test VM (demande de Palawi) | Test VM T11 |
| 2026-10-27/28 | v1.4 | Six retours de Palawi, questions en une fois (Source = écran seul, état du lecteur par clip, export sans coupure auto, 12 clips) ; #125 défilement d'État ; #124 glisser (122 → 2 chargements, curseur toutes les 17 ms au lieu de figé) ; #126 cellule Source ; #127 son « clip terminé » (candidat A choisi à l'écoute) ; #128 onglets ; #129 Mix complet coupé à la lecture ; session manuelle validée ; version 1.4.0 préparée | Accord de Palawi pour `dev` → `main`, tag, brouillon, publication |
| 2026-10-26 | v1.3.1 | Sons de PalaClips dans la piste 1 confirmés en mode bureau seulement ; exclusion du PID mesurée (sons retirés, toutes les sorties, retard de la piste Jeu) ; soustraction écartée (−3 dB) ; choix de Palawi → source au choix (#118, ADR-041) ; listes de clips suivant `library-changed { removed }` (#119) ; lecteur : pistes cliquables, calage (#120) ; session de Palawi validée ; version 1.3.1 | Release v1.3.1 publiée (accord de Palawi), vérifiée sans connexion |
| 2026-10-24/26 | v1.3 | Zip de Palawi : moteur correct, copies figées côté front → source unique de config (#113) ; recherche et mesures (2e NVENC : invisible si plus léger, ~5 % pour deux 1440p60 ; piste ≈ 5 % d'un cœur après partage du relevé des processus) ; décisions de Palawi ; 2e encodage pour l'enregistrement, horloge alignée, A/V 8 ms ; estimation −1,4 à −5,4 % ; 26 pistes de bout en bout (compression corrigée) ; Replay & qualité en trois parties, accès direct, sons d'enregistrement (#114) ; T15 validée ; captures du README ; version 1.3.0 (#115, #116), tag, publication | Release v1.3.0 publiée (accord de Palawi), vérifiée sans connexion |
| 2026-10-23/24 | v1.2 | Recherche (Discord 20 Mo, codecs, NVENC un passage, glisser-déposer Tauri) et décisions de Palawi (#105) ; pages Capture et Audio + alerte de sortie, vitest (#106) ; enregistrer après la sauvegarde, jonction continue, synchro identique, mémoire sans croissance sur 10 min (#107) ; compression à une taille cible, 11 cas sous la cible, HDR → SDR (#108) ; session T14 de Palawi en jeu validée, retours appliqués (#109) ; release 1.2.0 préparée | Release v1.2.0 publiée (accord de Palawi), vérifiée sans connexion |
