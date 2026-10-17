# HANDOFF — où on en est

> Mis à jour à la fin de **chaque** session (`/handoff`). Une nouvelle session doit pouvoir
> reprendre en lisant uniquement `CLAUDE.md` + ce fichier.

## État actuel

- **Phase en cours :** 5 — Application et sélecteurs. Phase 4 **validée** le 2026-10-16
  (`docs/reports/phase-4.md`, ADR-020 et ADR-021). Brief du premier écran (État + shell)
  confirmé par Palawi : `.impeccable/surfaces/app-src-app-tsx.md` § « Brief confirmé ».
- **Branche :** `feature/hotkeys` (PR à merger) : raccourcis personnalisables + écran Réglages
  (Raccourcis, Général) ; `dev` contient déjà les PR #21 à #26.
- **Dernière mise à jour :** 2026-10-17

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
    `%LOCALAPPDATA%\PalaClips\cache\thumbnails\<clip>.jpg`, après fermeture du MP4) ;
    `clip_saved.thumbnail` ; `library` : `probe_clip` (durée, titres des pistes audio lus dans le
    fichier), `recent_clips(root, limit)`, `disk_space` ; `paths::thumbnail_path` ;
    `EngineState: Default`.
  - CLI : `recent-clips [--clips-dir] [--limit]` (JSON), `replay --levels`.
  - `ipc.ts` et `ARCHITECTURE.md` § IPC synchronisés.
- **Réglages, noms de jeux, confirmation, clips sans doublon (2026-10-17)** :
  - `app/src-tauri/src/settings.rs` : `settings.json` (`%APPDATA%\PalaClips\`) =
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

## Prochaines actions (dans l'ordre)

1. Phase 5, suite (une branche par tâche, depuis `dev`) :
   - merger la PR de `feature/hotkeys` si ce n'est pas fait ;
   - sections Réglages restantes via `/design` (brief confirmé :
     `.impeccable/surfaces/app-src-components-settings-tsx.md`, application immédiate sauf
     redémarrage du tampon → confirmation en ligne RPL-07) : Capture (miniatures en direct),
     Audio (vumètres, recherche, appli non lancée, renommage, assistant AUD-11), Replay &
     qualité (estimation RAM RPL-03), Stockage (dossier, quota, noms de jeux modifiables) ;
   - ⚠️ L'app arme le replay au démarrage : demander à Palawi avant tout lancement ; tests de
     saisie de raccourcis : jamais de touches synthétiques (elles partent vers la fenêtre au
     premier plan de Palawi), le faire tester par lui ; une sauvegarde de test écrit un vrai
     clip : le supprimer après (demander).
2. Phase 6 (note de Palawi) : la galerie affiche la liste des pistes de chaque clip, noms lus
   dans le fichier (Resolve ne les montre pas, ADR-021) ; ajouté au PLAN.
3. Puis les autres écrans de la phase 5 dans l'ordre du PLAN (Réglages : Capture, Audio,
   Replay & qualité, Raccourcis, Stockage, Général ; assistant de calibrage AUD-11).

## Bloquants

Aucun. (PERF-02 levé le 2026-10-16 : RAM mesurée en working set privé, ADR-023.)

## Décisions en attente de Palawi

- Alerte G-Sync validée le 2026-10-16 : CAP-11, ADR-018, code en phase 7.

## Points d'attention

- Capture et fps du jeu (2026-10-17, `scripts/fps-impact.ps1`, journaux
  `docs/reports/phase-7/`) : aucune perte mesurable dans Fortnite plafonné à ~225 i/s, avec et
  sans G-Sync ; le jeu reste en Independent Flip (WGC ne force pas la composition). Rien à
  corriger côté capture. La perte de 50–100 i/s du 2026-10-16 venait **probablement** des
  réencodages et du VMAF lancés en parallèle (hypothèse, non vérifiée). PERF-03 reste à
  valider GPU-bound en phase 7 (script : mise en route non comptée, GPU relevé par
  `nvidia-smi`).

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
- HDR basculé pendant une capture : pris en compte seulement au démarrage suivant (à traiter
  en phase 7, robustesse).
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
