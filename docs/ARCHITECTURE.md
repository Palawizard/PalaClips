# Architecture — PalaClips

> Référence technique. Les choix et leurs raisons sont dans `docs/DECISIONS.md`, les faits
> vérifiés et leurs sources dans `docs/RESEARCH.md`.

## 1. Vue d'ensemble

```
┌──────────────────────────── processus palaclips.exe ─────────────────────────────┐
│                                                                                  │
│  Zone de notification · raccourcis globaux · instance unique · mises à jour      │
│                       (app/src-tauri, Tauri 2)                                    │
│        │ commandes ▲ événements                         ┌──────────────────────┐ │
│        ▼           │                                    │ WebView2 (React UI)  │ │
│  ┌───────────── palaclips-engine ──────────────┐        │ créé à la demande,   │ │
│  │ EngineHandle (canal de commandes)           │◄──IPC─►│ détruit à la         │ │
│  │                                             │        │ fermeture            │ │
│  │  capture ─► pacer CFR ─► encode vidéo ─┐    │        └──────────────────────┘ │
│  │                                        ├─► fan-out ─┬─► replay (PacketRing)  │ │
│  │  audio x N ─► combleur ─► encode AAC ──┘            └─► record (mux hybride) │ │
│  │                                                                             │ │
│  │  sources (sessions audio, processus, écrans)   library (index des clips)    │ │
│  └─────────────────────────────────────────────┘                                │
└──────────────────────────────────────────────────────────────────────────────────┘
```

Un seul processus. Le WebView2 (et ses processus enfants) n'existe que quand la fenêtre est
ouverte (APP-02, PERF-01).

## 2. Crates et modules

| Chemin | Rôle | Dépend de Tauri ? |
|---|---|---|
| `crates/engine` | Capture, audio, encodage, replay, mux, sources, bibliothèque de clips | **Non, jamais** |
| `crates/cli` | Banc de test du moteur, une sous-commande par phase | Non |
| `app/src-tauri` | Shell : commandes, événements, zone de notification, raccourcis, plugins | Oui |
| `app/src` | UI React 19 + TypeScript + Tailwind 4 | — |

Modules du moteur (`crates/engine/src/`) :

| Module | Contenu | Phase |
|---|---|---|
| `clock` | `Hns` (QPC en 100 ns), conversions vers bases de temps | 0 ✅ |
| `config` | `EngineConfig` sérialisable + validation | 0 ✅ |
| `events` | `EngineEvent` (contrat moteur → UI) | 0 ✅ |
| `error` | `EngineError` (messages FR affichables) | 0 ✅ |
| `replay` | `PacketRing` purgé par âge, estimation RAM, stockage disque | 0 (file) ✅ / 4 |
| `capture` | WGC, énumération écrans/fenêtres, pacer CFR, repli (miniatures : phase 5) | 1 ✅ |
| `encode` | FFmpeg (rsmpeg) : NVENC/AMF/QSV, conversion GPU, détection par test réel ; AAC | 1 ✅ / 2 |
| `audio` | Loopback mix, micro, process loopback, combleur de silences, détection muette | 2–3 |
| `sources` | Sessions audio, processus, appli au premier plan, périphériques | 3 |
| `mux` | MP4 classique vidéo seule (`Mp4Writer`) ; MP4 replay, hybride, métadonnées, noms | 1 ✅ / 4–6 |
| `record` | Enregistrement manuel, découpage, récupération après crash | 6 |
| `library` | Index des clips (SQLite), miniatures, surveillance du dossier, quota | 6 |
| `engine` | `Engine` / `EngineHandle` : orchestration, threads, cycle de vie | 4–5 |

Les modules sans ✅ n'existent pas encore : ils sont créés dans leur phase. Leur cahier des
charges :

**`capture`**
- Énumérer écrans et fenêtres capturables (`Window::enumerate` + `is_valid()`), avec miniatures.
- Session WGC sur la `CaptureTarget` choisie : bordure (`DrawBorderSettings`) et curseur selon
  les réglages.
- Le rappel WGC ne fait qu'un `CopyResource` GPU de `Frame::as_raw_texture` vers la « dernière
  image » (texture BGRA recréée si la taille change). Aucune copie CPU.
- Pacer CFR (`CfrClock`) : tick `n` à `t0 + n / fps` sur l'horloge QPC ; à chaque tick, la
  dernière image reçue est convertie et encodée (doublon si Windows n'a rien envoyé, surplus
  ignorés). Tick en retard → tous les ticks manqués sont émis. `MinUpdateInterval` = moitié de
  la période.
- Repli sur l'écran de la fenêtre si la capture de fenêtre échoue → alerte
  `capture_fallback_monitor`.
- Suivre changement de résolution, fermeture de la fenêtre, perte du périphérique D3D.

**`audio`**
- Pistes dans l'ordre : mix complet (loopback de la sortie par défaut, suivie à chaud), micro
  (gain appliqué), une par appli (`new_application_loopback_client(pid_parent, true)`).
- Horodatage uniquement par `pu64QPCPosition`.
- Combler les trous par du silence d'après les écarts QPC ; `AUDCLNT_BUFFERFLAGS_SILENT` =
  silence ; jamais d'arrêt sur timeout.
- Mode exclusif d'un jeu (`AUDCLNT_E_DEVICE_IN_USE`) : réessayer chaque seconde et le signaler.
- Rattachement automatique d'une piste à son appli (suivi par nom d'exe) : `AudioSource::
  Application { exe_name }` cherche le PID racine (chaque seconde tant que l'appli n'est pas
  lancée, en comblant du silence), ouvre le process loopback sur l'arbre et attend aussi le
  handle du processus (`SYNCHRONIZE`). À sa fin (le flux, lui, continue d'envoyer des zéros),
  la piste se referme et attend le retour de l'appli.
- Piste « Jeu » (`AudioSource::Game(CurrentGame)`, ADR-019) : même mécanique, l'exe est le
  « jeu en cours » de `sources::CurrentGame` : processus fixé par l'utilisateur
  (`AudioSettings::game_exe`), sinon l'appli de la fenêtre capturée, sinon la dernière appli
  passée au premier plan en plein écran ou sans bordure (fenêtre qui couvre son écran, sans
  barre de titre). `ForegroundWatch` (thread dédié, `SetWinEventHook`
  `EVENT_SYSTEM_FOREGROUND` + `EVENT_OBJECT_LOCATIONCHANGE` de la fenêtre au premier plan)
  réveille la piste ; elle ne se rouvre que si le jeu change.
- Piste à zéro numérique alors que la session de l'appli a du signal → `app_track_silent` :
  piste sans échantillon non nul depuis 3 s, puis crête de session de l'arbre > 0,001 à deux
  vérifications (une par seconde) → une alerte, réarmée dès que la piste reçoit du signal.
  Les sessions de PalaClips sont ignorées : une capture loopback crée une session de rendu
  « active » dont le vumètre reproduit la sortie. Limite connue : une appli suivie qui
  enregistre elle-même la sortie en loopback (OBS, fond d'écran réactif au son) peut déclencher
  l'alerte alors qu'elle ne joue rien.
- Capture en 48 kHz, stéréo, f32, `autoconvert = true` (le rééchantillonnage et le mixage
  des canaux sont faits par Windows).
- `GapFiller` : position cible = `(qpc + décalage − t0)` en échantillons ; écart > 5 ms →
  silence inséré ou chevauchement coupé ; en dessous, la dérive lissée est corrigée d'une
  image à la fois (au-delà de 1 ms). Fin exacte : `finish_at(fin)`, la piste lit jusqu'à la
  fin (250 ms de grâce + décalage négatif) puis comble.
- Périphérique muet (Windows n'envoie aucun paquet quand rien ne joue) : après 50 ms sans
  paquet, la piste se réveille et comble jusqu'à `maintenant + décalage − 100 ms`, puis
  encode. Elle produit donc des paquets en continu ; aucun réveil quand les données arrivent.
  Même chose pendant l'attente d'un périphérique indisponible.
- Décalage de synchro par piste (AUD-11, ADR-017), positif = son retardé.

**`encode`**
- Encodeurs `h264/hevc/av1` × `nvenc/amf/qsv`, détectés par un encodage de test réel.
- Entrée en frames matérielles `d3d11va` (pool dynamique, `BIND_RENDER_TARGET`) sur le
  périphérique de WGC ; BGRA → NV12 + mise à l'échelle + bandes noires par le D3D11 Video
  Processor (BT.709 limité), écrit directement dans la frame de l'encodeur. QSV : contexte
  dérivé + `av_hwframe_map`.
- Écran HDR : capture FP16 scRGB → `HdrToSdr` (pixel shader : ÷ blanc SDR, sRGB) → BGRA8 →
  même chemin (ADR-015).
- CBR pour le replay, intervalle d'images clés configurable.
- AAC-LC 48 kHz stéréo, 192 kb/s, un encodeur par piste ; PTS = numéro d'échantillon, le
  premier paquet a un PTS négatif (amorçage de 1024 échantillons) que la liste d'édition MP4
  compense (`start_time = 0`).
- Une seule session matérielle partagée par le replay et l'enregistrement.

**`mux`**
- Replay : MP4 classique écrit dans un thread dédié depuis le `PacketRing`.
- Enregistrement : `movflags +hybrid_fragmented`.
- Titre de chaque piste audio dans les métadonnées.
- Nom : `<Jeu>_<AAAA-MM-JJ_HH-MM-SS>[_replay].mp4` dans `<dossier>/<Jeu>/`.
- Vérification d'espace disque → `disk_almost_full` / `disk_full`.

**`record`**
- Partage capture et encodeur avec le replay.
- Option « inclure le replay en cours au début ».
- Découpage par durée ou taille ; récupération au démarrage d'un fichier interrompu.

**`sources`**
- Sessions audio actives + crête (`get_iaudiosessionmanager` → `get_audiosessionenumerator` →
  `get_process_id`, `get_audiometerinformation`).
- Processus (appli non lancée), résolution exe → PID parent, démarrages/arrêts.
- Jeu en cours (`CurrentGame`) : piste « Jeu », nom des clips (STO-03), galerie (GAL-01/02).
- Périphériques audio et changements du périphérique par défaut.
- Persistance par nom d'exe / identifiant de périphérique, jamais par PID.
- Processus par `CreateToolhelp32Snapshot` (PID, PID parent, nom d'exe), sans dépendance.
  PID racine d'une appli = processus de cet exe dont le parent n'est pas le même exe (le plus
  petit PID s'il y en a plusieurs) ; l'arbre = la racine et tous ses descendants.
- Fenêtre au premier plan (`foreground_window`) : `GetForegroundWindow` → PID → nom d'exe,
  rectangle comparé à celui de son écran (thread « per-monitor DPI aware »), style
  `WS_CAPTION`. Jamais un jeu : PalaClips lui-même et le shell (`explorer.exe`, menu
  Démarrer, recherche, écran de verrouillage, `ApplicationFrameHost.exe`).
- Sessions audio de **toutes** les sorties actives (une appli peut jouer ailleurs que sur la
  sortie par défaut) : PID, exe, état, crête `IAudioMeterInformation`.

**`library`**
- Index SQLite, miniatures, surveillance du dossier, quota, pistes extraites, proxys H.264.

### Dépendances prévues (versions stables vérifiées le 2026-10-15)

| Crate | Version | Module | Phase |
|---|---|---|---|
| `windows-capture` | 2.0 | capture | 1 |
| `windows` | 0.62 | capture, audio, sources | 1 |
| `rsmpeg` | 0.18 (FFmpeg 8.x, ADR-004) | encode, mux | 0–1 |
| `wasapi` | 0.26 | audio, sources | 2 |
| `rusqlite` (feature `bundled`) | dernière stable | library | 6 |
| `tauri-plugin-*` | voir `docs/RESEARCH.md` § UI | app | 5–8 |

## 3. Threads et flux de données

Tous les échanges passent par des canaux **bornés** (`crossbeam-channel`). Aucun verrou n'est
tenu pendant un appel système bloquant.

| Thread | Entrée | Sortie | Notes |
|---|---|---|---|
| Capture (callback WGC) | images WGC | « dernière image » (texture GPU) | un `CopyResource`, jamais de travail lourd |
| Vidéo (`palaclips-video`) | dernière image | `AVPacket` vidéo (canal borné) | pacer CFR sur QPC + conversion GPU + encodeur : une seule session matérielle |
| Audio × N (`palaclips-audio-<piste>`, un par source) | WASAPI | `AVPacket` AAC (canal borné) | 48 kHz stéréo f32, `WaitForMultipleObjects` (arrêt, données, défaut changé), réveil après 50 ms sans paquet pour combler ; gain, décalage de synchro, `GapFiller`, AAC dans le même fil |
| Fan-out | paquets | replay + enregistreur | `Arc<[u8]>` : pas de copie entre les deux |
| Écriture replay | instantané du `PacketRing` | MP4 | lancée à la sauvegarde, n'arrête pas la capture |
| Enregistreur | paquets | MP4 hybride | écriture continue |
| Sources | sondes Windows | listes + crêtes | actif seulement si l'UI est ouverte ou rattachement en attente |
| Bibliothèque | dossier des clips | index SQLite + miniatures | notifications de fichiers, pas de polling |

### Horloge (règle 1)

```
vidéo : Frame::timestamp() = SystemRelativeTime (QPC, 100 ns)
audio : GetBuffer → pu64QPCPosition (QPC, 100 ns)
                     │
                     ▼
            t0 = QPC au démarrage de la session
     PTS(vidéo) = n, tick n à t0 + n/fps (QPC)          (t0 = VideoStream::origin)
     PTS(audio) = (qpc − t0).to_samples(48_000)        (après comblement)
```

### Replay

- `PacketRing` par flux, purge par âge, conserve toujours une image clé au début (testé).
- Sauvegarde : instantané des indices sous verrou court, `start_index(maintenant − durée)`,
  alignement de chaque piste audio sur le PTS de l'image clé, écriture dans un thread dédié.
- Stockage disque : même interface, segments de quelques secondes dans
  `%LOCALAPPDATA%\PalaClips\replay\`.

## 4. Pistes et métadonnées

| Ordre | Piste | Titre écrit |
|---|---|---|
| 1 | Mix complet | `Mix complet` |
| 2 | Micro | `Micro` |
| 3… | Appli (choisie ou « Jeu » automatique) | libellé choisi par l'utilisateur |

Les lecteurs et Discord ne lisent que la piste 1 : c'est pour ça qu'elle contient tout.

## 5. UI et lecture des clips

- Les fichiers sont servis au WebView par un **protocole personnalisé `palaclips://`** qui
  gère les requêtes `Range` (lecture et déplacement fluides sur des fichiers de plusieurs Go,
  ADR-007).
- Chromium ne permet pas de choisir la piste audio d'une `<video>` sans option expérimentale :
  le lecteur lit la vidéo **sans son** et joue les pistes via Web Audio à partir de fichiers
  audio extraits à la demande et mis en cache (ADR-008). Mute/solo/volume par piste.
- HEVC/AV1 : si WebView2 ne sait pas décoder le codec sur la machine, le lecteur utilise un
  proxy H.264 généré à la demande (vérifié en phase 6).

## 6. IPC (contrat)

Source de vérité : `app/src-tauri/src/commands.rs`, `crates/engine/src/events.rs`,
miroir TypeScript `app/src/lib/ipc.ts`.

### Commandes (front → app)

| Commande | Phase | Rôle |
|---|---|---|
| `engine_version` | 0 ✅ | version du moteur |
| `default_config` | 0 ✅ | configuration par défaut |
| `get_config` / `set_config` | 5 | lire / valider + appliquer + persister |
| `list_capture_targets` | 5 | écrans + fenêtres + miniatures |
| `list_audio_sources` | 5 | sessions audio, processus, périphériques |
| `replay_arm` / `replay_disarm` / `replay_save` | 5 | contrôle du replay |
| `record_start` / `record_stop` | 6 | enregistrement |
| `list_clips` / `clip_*` | 6 | galerie : renommer, favori, supprimer, copier, exporter |
| `get_hotkeys` / `set_hotkeys` | 5 | raccourcis, avec conflits |
| `performance_snapshot` | 7 | panneau de performances |
| `export_diagnostics` | 8 | archive zip |

### Événements (app → front) : `EngineEvent`

`state_changed`, `clip_saved`, `audio_levels` (~20 Hz, seulement fenêtre ouverte),
`replay_stats`, `alert { severity, code, message }`.

Codes d'alerte stables : `capture_fallback_monitor`, `capture_source_lost`, `capture_source_rate_limited`, `app_track_silent`,
`audio_device_changed`, `audio_device_unavailable`, `disk_almost_full`, `disk_full`, `encoder_reset`, `no_hardware_encoder`.

## 7. Données sur le disque

| Quoi | Où |
|---|---|
| Réglages (`settings.json`, versionné) | `%APPDATA%\PalaClips\` |
| Journaux (rotation) | `%APPDATA%\PalaClips\logs\` |
| Index de la bibliothèque (SQLite) | `%APPDATA%\PalaClips\library.db` |
| Miniatures, pistes extraites, proxys | `%LOCALAPPDATA%\PalaClips\cache\` |
| Buffer de replay sur disque | `%LOCALAPPDATA%\PalaClips\replay\` |
| Clips | `Vidéos\PalaClips\<Jeu>\` (réglable) |
| DLL FFmpeg (LGPL) | à côté de `palaclips.exe` (ressources du bundle) |

## 8. Légèreté

- WebView détruit à la fermeture ; `RunEvent::ExitRequested` → `prevent_exit()` ; sortie via
  « Quitter » uniquement.
- Sondes (sessions audio, crêtes) arrêtées quand l'UI est fermée, sauf rattachement en attente.
- Pas de polling : notifications Windows (périphériques audio, fichiers). Exceptions bornées :
  une piste d'appli qui attend son appli cherche le processus chaque seconde (Toolhelp) ; une
  appli rattachée est suivie par son handle de processus (attente, pas de polling) ; la piste
  « Jeu » est réveillée par les événements de premier plan (`SetWinEventHook`).
