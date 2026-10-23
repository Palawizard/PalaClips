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
| `replay` | `PacketRing` purgé par âge, estimation RAM, `plan_clip`, paquets en RAM ou sur disque (`Spool`) | 0 (file) ✅ / 4 ✅ |
| `capture` | WGC, énumération écrans/fenêtres, pacer CFR, repli (miniatures : phase 5) | 1 ✅ |
| `encode` | FFmpeg (rsmpeg) : NVENC/AMF/QSV, encodeurs CPU (`h264_mf`, SVT-AV1), conversion GPU, détection par test réel ; AAC | 1 ✅ / 2 / v1.1 |
| `audio` | Loopback mix, micro, process loopback, combleur de silences, détection muette | 2–3 ✅ |
| `sources` | Sessions audio, processus, jeu en cours (`CurrentGame`), périphériques | 3 ✅ |
| `mux` | `Mp4Writer` multi-pistes titrées (vidéo, replay) ; MP4 hybride | 1–4 ✅ / 6 |
| `paths` | Dossiers connus (Vidéos, LocalAppData), nom des clips, mémoire du processus | 4 ✅ |
| `record` | Enregistrement manuel, découpage, récupération après crash | 6 |
| `library` | Index des clips (SQLite), miniatures, surveillance du dossier, quota | 6 |
| `engine` | `Engine` / `EngineHandle` : orchestration, threads, cycle de vie | 4 ✅ / 5 |

Les modules sans ✅ n'existent pas encore : ils sont créés dans leur phase. Leur cahier des
charges :

**`capture`**
- Énumérer écrans et fenêtres capturables (`Window::enumerate` + `is_valid()`), avec miniatures.
- Écran choisi (CAP-03) : `CaptureTarget::Monitor { device_name, monitor_path }`. Les noms GDI
  `\\.\DISPLAYn` se renumérotent pendant une session ; `monitor_path` (`monitorDevicePath` de
  `DISPLAYCONFIG_TARGET_DEVICE_NAME`, modèle de l'écran + port) ne change pas. Au démarrage, le
  chemin redonne le nom GDI du moment (`display::device_name_of`) ; un écran débranché donne
  `monitor_missing` au lieu d'un autre écran. Réglage ancien sans chemin : nom GDI, comme avant.
- Session WGC sur la `CaptureTarget` choisie : bordure (`DrawBorderSettings`) et curseur selon
  les réglages.
- Le rappel WGC ne fait qu'un `CopyResource` GPU de `Frame::as_raw_texture` vers la « dernière
  image » (texture BGRA recréée si la taille change). Aucune copie CPU.
- Pacer CFR (`CfrClock`) : tick `n` à `t0 + n / fps` sur l'horloge QPC ; à chaque tick, la
  dernière image reçue est convertie et encodée (doublon si Windows n'a rien envoyé, surplus
  ignorés). Tick en retard → tous les ticks manqués sont émis. `MinUpdateInterval` = moitié de
  la période. Encodeur CPU : au plus une demi-seconde de ticks en retard est émise, les plus
  anciens sont sautés (trou dans les PTS, la synchro A/V tient) ; comptés en retard, ils lèvent
  `cpu_encoder_overloaded` (ADR-035).
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
  Application { exe_name }` cherche les PID racines (chaque seconde tant que l'appli n'est pas
  lancée, en comblant du silence) et ouvre un process loopback par racine : une appli lancée en
  plusieurs instances indépendantes (deux `Vesktop.exe` sans parent commun vivant) a un arbre
  par instance. `StreamSum` donne à chaque flux son `GapFiller` (horloge QPC, silences) démarré
  à la position de la piste et additionne les blocs communs, déjà alignés ; le remplisseur de la
  piste les reçoit tels quels (un seul flux = résultat identique à avant). La piste attend aussi
  le handle de chaque processus racine (`SYNCHRONIZE`) et revérifie les racines chaque seconde :
  une instance qui apparaît ou disparaît rouvre les flux. Sans racine, la piste attend le retour
  de l'appli.
- Piste « Jeu » (`AudioSource::Game(CurrentGame)`, ADR-019) : même mécanique, l'exe est le
  « jeu en cours » de `sources::CurrentGame` : processus fixé par l'utilisateur
  (`AudioSettings::game_exe`), sinon l'appli de la fenêtre capturée, sinon la dernière appli
  passée au premier plan en plein écran ou sans bordure (fenêtre qui couvre son écran, sans
  barre de titre). `ForegroundWatch` (thread dédié, `SetWinEventHook`
  `EVENT_SYSTEM_FOREGROUND` + `EVENT_OBJECT_LOCATIONCHANGE` de la fenêtre au premier plan)
  réveille la piste ; elle ne se rouvre que si le jeu change.
- Piste à zéro numérique alors que la session de l'appli a du signal → `app_track_silent` :
  piste sans échantillon non nul depuis 3 s, puis crête de session des arbres > 0,001 à deux
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
- Encodeurs `h264/hevc/av1` × `nvenc/amf/qsv`, et sur le processeur `h264_mf` (MFT logiciel de
  Windows, `hw_encoding=0`, CBR) et `libsvtav1` (`rtc=1:rc=2:lp=2`, M10 jusqu'à 720p30, M12
  au-delà), tous détectés par un encodage de test réel (`video_encoders`, `EncoderInfo.hardware`).
  Choix `video.encoder` = `auto` (matériel si un encodeur matériel marche, sinon CPU),
  `hardware` ou `cpu` (`pick_encoder`) ; HEVC n'existe qu'en matériel (ADR-035).
- Encodeur CPU : BGRA → NV12 par le Video Processor dans une texture NV12, une seule relecture
  par texture de staging, puis swscale vers le format de l'encodeur (NV12, ou yuv420p pour
  SVT-AV1). Sans Video Processor (création ou `VideoProcessorBlt` en échec, VM, WARP) :
  relecture BGRA et conversion + mise à l'échelle + bandes noires par swscale (BT.709 limité).
  Les fils créés par l'encodeur (départ dans `mfh264enc.dll` ou `avcodec-*.dll`) passent en
  priorité « en dessous de la normale » à l'ouverture et après la première image
  (`encode::threads`). ENC-04 reste vrai pour le chemin matériel.
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
  PID racines d'une appli (`root_pids`) = processus de cet exe dont le parent n'est pas le même
  exe, triés ; `root_pid` (liste des sessions, jeux) garde le plus petit. L'arbre = la racine et
  tous ses descendants.
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
| `rusqlite` (feature `bundled`) | 0.40 | library | 6 ✅ |
| `notify` | 8.2 (9 encore en RC) | library (surveillance du dossier, GAL-08) | 6 ✅ |
| `tauri-plugin-*` | voir `docs/RESEARCH.md` § UI | app | 5–8 |

## 3. Threads et flux de données

Tous les échanges passent par des canaux **bornés** (`crossbeam-channel`). Aucun verrou n'est
tenu pendant un appel système bloquant.

| Thread | Entrée | Sortie | Notes |
|---|---|---|---|
| Capture (callback WGC) | images WGC | « dernière image » (texture GPU) | un `CopyResource`, jamais de travail lourd |
| Vidéo (`palaclips-video`) | dernière image | `AVPacket` vidéo (canal borné) | pacer CFR sur QPC + conversion GPU + encodeur : une seule session matérielle |
| Audio × N (`palaclips-audio-<piste>`, un par source) | WASAPI | `AVPacket` AAC (canal borné) | 48 kHz stéréo f32, `WaitForMultipleObjects` (arrêt, données, défaut changé), réveil après 50 ms sans paquet pour combler ; gain, décalage de synchro, `GapFiller`, AAC dans le même fil |
| Moteur (`palaclips-engine`) | commandes (`EngineHandle`) + paquets de tous les flux + alertes des pistes | un `PacketRing` par flux, événements | un seul fil : armer, désarmer, sauvegarder, reconfigurer ; `select` sur les canaux, réveil chaque seconde pour `replay_stats` ; paquets partagés par référence FFmpeg (`av_packet_clone`), sans copie, pour le replay et (phase 6) l'enregistreur |
| Écriture replay (`palaclips-replay-save`) | instantané du `PacketRing` | MP4 | lancée à la sauvegarde, n'arrête pas la capture ; un fil par sauvegarde |
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
  Fenêtre = durée + intervalle d'images clés + 2 s : l'audio arrive ~150 ms après la vidéo
  (comblement, AAC, décalage de synchro) et doit encore couvrir l'image clé de départ.
- Sauvegarde (`plan_clip`, dans le fil moteur, sans verrou) : fin = la plus petite des fins
  disponibles (dernière image + 1 image, dernier paquet AAC + 1024 échantillons de chaque
  piste) ; début = image clé ≤ fin − durée ; chaque piste audio part du paquet qui couvre
  cette image clé. Paquets partagés, triés par temps, PTS/DTS décalés du début dans la base
  de chaque flux (premier paquet audio légèrement négatif, compensé par la liste d'édition :
  `start_time = 0`), puis MP4 écrit par `palaclips-replay-save`, qui émet `clip_saved`
  (chemin, durée, latence depuis la commande).
- Stockage disque (`Spool`) : données des paquets dans des segments de 4 s
  (`%LOCALAPPDATA%\com.palawi.palaclips\replay\NNNNNNNN.seg`), le ring ne garde que les positions ;
  un segment se supprime dès que plus aucun paquet ne le référence ; les restes d'une session
  précédente sont effacés au démarrage.
- Nom (STO-03) : `<dossier>/<Jeu>/<Jeu>_<AAAA-MM-JJ_HH-MM-SS>_replay.mp4`, `<Jeu>` = exe du
  jeu en cours sans `.exe`, « Bureau » s'il n'y en a pas ; suffixe `_2`, `_3`… si deux
  sauvegardes tombent dans la même seconde. Dossier par défaut : `Vidéos\PalaClips`
  (`StorageSettings::clips_dir`).
- Redémarrage (RPL-07) : `configure` redémarre la capture et vide le tampon si la zone, la
  vidéo, l'audio ou le stockage changent ; une nouvelle durée ne fait que changer la fenêtre.

## 4. Pistes et métadonnées

| Ordre | Piste | Titre écrit |
|---|---|---|
| 1 | Mix complet : somme des pistes choisies, ou sortie audio entière + micro | `Mix complet` |
| 2 | Micro | `Micro` |
| 3… | Appli (choisie ou « Jeu » automatique) | libellé choisi par l'utilisateur |

Les lecteurs et Discord ne lisent que la piste 1 : c'est pour ça qu'elle contient tout ce qui
est enregistré, micro compris (ADR-029, ADR-030, comme la piste 1 d'OBS envoyée au stream).

- Par défaut (`desktop_in_full_mix: false`), la piste 1 ne capture rien elle-même
  (`AudioSource::TracksMix`) : elle additionne le micro (si `microphone_in_full_mix`), la piste
  Jeu et les pistes d'applis.
- Avec « Audio du bureau » (`desktop_in_full_mix: true`), elle capture toute la sortie
  (`SystemMix`) et n'ajoute que le micro : la sortie contient déjà le jeu et les applis.
- Mixage (`engine::mixdowns`) : chaque piste ajoutée publie ses échantillons déjà recalés
  (même origine QPC, même index d'échantillon) sur son `MixBus` ; la piste 1 les additionne
  avant l'encodage AAC, puis par un limiteur doux (anticipation 3 ms, plafond −1 dBFS, retour
  80 ms ; vumètre sur le résultat). Elle attend la piste la plus lente au plus
  `max(200 ms, plus grand décalage négatif des pistes ajoutées + 150 ms)`, borné à 1 s, puis
  continue sans elle ; à l'arrêt, elle attend leur fin au plus 2 s. Une piste Jeu qui suit le
  même exe qu'une piste d'appli ajoutée envoie du silence au mixage (pas de doublon). Le
  calibrage force « Audio du bureau » et coupe le micro de la piste 1 pour mesurer chaque
  source seule.

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

Textes (ADR-028) : aucune phrase affichée ne traverse l'IPC. Une erreur de commande est un
`Localized { code, params }` ; le front l'affiche par `messages.<code>` de
`app/src/locales/fr.json` / `en.json` (`errorText`, `message` de `lib/i18n.ts`). Un paramètre
peut être un autre `Localized` (`reason`), traduit à son tour ; `detail` est le texte brut
d'une erreur système.

### Commandes (front → app)

| Commande | Phase | Rôle |
|---|---|---|
| `engine_version` | 0 ✅ | version du moteur |
| `default_config` | 0 ✅ | configuration par défaut |
| `get_state` | 5 ✅ | état du moteur (`AppStatus` : état, secondes et octets du tampon, source capturée, pistes dans l'ordre avec leur état, alertes actives une par code) |
| `get_config` | 5 ✅ | configuration en cours |
| `set_config { config }` | 5 ✅ | si la vidéo ou la capture change : limite du codec puis essai réel d'ouverture de l'encodeur à la taille de sortie (refus = `output_too_large` / `output_unsupported` avec codec et taille, rien n'est appliqué) ; valider, enregistrer `settings.json` (écriture atomique), appliquer au moteur (redémarre le replay si la capture ou les sources changent) ; refusé si le fichier vient d'une version plus récente |
| `get_autostart` / `set_autostart { enabled }` | 5 ✅ | démarrage avec Windows (APP-04, `tauri-plugin-autostart` 2.7, clé `Run` de l'utilisateur) ; lancé avec `--autostart` : zone de notification seulement, aucune fenêtre ni WebView2, puis « armer au démarrage » comme d'habitude |
| `get_feedback` / `set_feedback { feedback }` | 5 ✅ | confirmation de sauvegarde : `{ sound, toast }` (RPL-06), enregistrée dans `settings.json` |
| `reset_settings` | 5 ✅ | tous les réglages par défaut (APP-07) : moteur, confirmation, raccourcis, thème, démarrage avec Windows désactivé |
| `get_theme` / `set_theme { theme }` | 5 ✅ | `system` / `dark` / `light` ; le front pose `data-theme` sur `<html>` (système suivi par `matchMedia`) |
| `get_language` / `set_language { language }` | 5 ✅ | APP-08, ADR-028 : `system` / `fr` / `en` ; `get_language` renvoie `{ setting, resolved }` (`system` = langue d'affichage de Windows, français si elle est française, anglais sinon), `set_language` renvoie la langue retenue et retraduit le menu de la zone de notification |
| `suspend_hotkeys` / `resume_hotkeys` | 5 ✅ | pendant la saisie d'un raccourci : sinon `RegisterHotKey` intercepte la combinaison avant la fenêtre |
| `list_capture_targets` | 5 | écrans + fenêtres + miniatures |
| `calibrate_audio` | 5 ✅ | assistant de calibrage (AUD-11, ADR-026) : capture temporaire de l'écran principal, flash + bip 10 s, mesure par piste, replay réarmé ; renvoie mesures et décalages proposés (rien n'est appliqué) |
| `list_audio_devices { input }` | 5 ✅ | sorties ou entrées actives (id, nom, par défaut) pour le Mix et le Micro |
| `list_app_sessions` | 5 ✅ | applis qui ont une session audio, regroupées par exe (PID racine, nom d'appli, crête) ; interrogé ~10 fois/s seulement pendant que la section Audio est visible |
| `pick_app_exe` | 5 ✅ | sélecteur de fichier .exe pour une appli non lancée ; renvoie le nom d'exe et le nom d'appli lu dans ses métadonnées |
| `replay_arm` / `replay_disarm` / `replay_save` | 5 ✅ | contrôle du replay (erreur = `Localized`) ; avec `replay.after_secs` > 0 (RPL-10, v1.2), `replay_save` écrit le tampon dans un MP4 hybride et y ajoute le direct jusqu'à la fin de la durée après l'appui ; pendant ce temps, un nouvel appui prolonge le même clip |
| `replay_finish_clip` | v1.2 ✅ | termine tout de suite le clip en cours (aussi dans le menu de la zone de notification, entrée visible seulement pendant le « après ») |
| `record_start` / `record_stop` | 6 ✅ | enregistrement manuel (REC-01/02) : même capture et mêmes encodeurs que le replay (arme le pipeline si besoin, désarmé à la fin s'il ne l'était pas) ; état `recording` ; MP4 hybride (`hybrid_fragmented`, fragment d'1 s, `flush_packets`) ; option `record.include_replay` (REC-04) et découpage `record.split` par durée ou taille sur image clé (REC-05) ; un changement de sources pendant l'enregistrement redémarre le replay à la fin ; calibrage refusé (`recording_running`) |
| `list_recent_clips { limit }` | 5 ✅ | derniers clips du dossier (`ClipSummary` : chemin, jeu, date, taille, durée, noms des pistes lus dans le fichier, miniature) |
| `storage_status` | 5 ✅ | dossier des clips, espace libre et total du disque |
| `pick_clips_dir` | 5 ✅ | sélecteur de dossier Windows (`tauri-plugin-dialog`, appelé côté Rust, aucune permission JS) ; renvoie le chemin ou rien si annulé ; l'app l'enregistre ensuite par `set_config` |
| `list_capture_targets` | 5 ✅ | écrans (nom GDI, `monitor_path` stable de DisplayConfig, nom, définition, Hz, principal) et fenêtres (exe, titre, PID, taille, réduite) ; PalaClips, fenêtres invisibles et fenêtres de moins de 160×90 exclues |
| `app_icon { processId }` | 5 ✅ | icône 48 px de l'exe en PNG (octets bruts, `SHDefExtractIconW` + encodeur PNG de FFmpeg) |
| `set_performance_panel { open }` | 7 ✅ | panneau de performances (APP-12) ouvert ou fermé : le moteur n'envoie `performance_stats` que si le panneau est ouvert **et** la fenêtre visible (coupé à la réduction et à la destruction de la fenêtre) |
| `start_preview { target, channel }` / `stop_preview { id }` / `stop_previews` | 5 ✅ | aperçu en direct d'une cible : session WGC à part, sans curseur ni bordure, ~1 image/s, réduite sur le GPU (≤ 384×216) puis JPEG envoyé en octets bruts sur un `Channel` ; 12 au plus ; toutes arrêtées à la destruction de la fenêtre. Jamais utilisé pour la capture réelle |
| `get_update` / `check_update` / `install_update` | 8 ✅ | mises à jour (ADR-034) : `UpdateInfo { current, available: { version, notes } | null }` ; `get_update` lit le résultat de la dernière recherche (démarrage puis toutes les 6 h), `check_update` relance, `install_update` télécharge, arrête le moteur, installe en mode `passive` et redémarre ; erreurs `update_failed { detail }`, `update_while_recording`, `update_none` ; une version nouvelle lève l'alerte `update_available { version }` (info) |
| `onboarding_needed` / `onboarding_checks` / `finish_onboarding` | 8 ✅ | premier lancement guidé (APP-06) : `onboarding_needed` vaut vrai seulement si `settings.json` n'existait pas (`onboarded` absent d'un fichier existant = déjà fait) ; `onboarding_checks` renvoie `{ windows_build, windows_supported (≥ 20348), webview, clips_dir_writable }` (le dossier des clips est créé et testé par un fichier vide aussitôt supprimé) ; `finish_onboarding` écrit `onboarded: true` (« Passer » et « Terminer ») ; une réinitialisation des réglages garde `onboarded` |
| `export_diagnostic` | 8 ✅ | boîte « Enregistrer sous » (zip daté), puis `diagnostic::entries` + `write_archive` : `system.txt` (version, Windows, processeur, mémoire, cartes graphiques et pilotes, HAGS, écrans et HDR, encodeurs, périphériques audio, WebView2), `settings.json` et `logs/*.log` ; chemin du profil remplacé par `%USERPROFILE%` et nom du PC par `[pc]` partout ; nom d'utilisateur et mots du nom complet du compte (`NetUserGetInfo` niveau 10) remplacés par `[user]` dans les noms de périphériques, les réglages et les journaux (pas dans le reste de `system.txt`, où « RTX 4070 SUPER » se ferait masquer) ; titre de fenêtre capturée masqué ; `system.txt` donne aussi le fuseau horaire (les journaux sont en UTC) ; renvoie le nom du zip ou `null` si annulé ; erreur `diagnostic_failed { detail }`. Même contenu par `palaclips-cli diagnostic --out` |
| `open_diagnostic` | 8 ✅ | ouvre dans l'Explorateur le dernier zip exporté pendant la session (chemin gardé côté Rust, jamais reçu du front) |
| `open_clips_dir` | 5 ✅ | crée le dossier des clips au besoin et l'ouvre dans l'Explorateur |
| `list_encoders` | 5 ✅ | encodeurs matériels et CPU (`{ name, description, codec, hardware, failure }`) détectés par encodage de test (ENC-01), mis en cache pour la session ; un codec sans encodeur pour le choix courant est grisé dans Réglages |
| `memory_status` | 5 ✅ | RAM physique totale et disponible (`GlobalMemoryStatusEx`) |
| `estimate_replay { config }` | 5 ✅ | estimation du tampon pour une config non appliquée (RPL-03) : taille de la source, taille de sortie (jamais plus haute que la source), débit vidéo, nombre de pistes, octets (fenêtre du tampon + 7 %). Réglages alerte au-delà de 40 % de la RAM disponible et propose le tampon sur disque |
| `list_clips { query }` | 6 ✅ | clips de la bibliothèque (index SQLite) : filtres `game`, `kind` (`replay` / `recording`, d'après le nom), `favorites`, `since_ms`, `search` (nom ou jeu), tri `newest` / `oldest` / `largest` / `longest` / `name`, `limit` ; seuls les clips du dossier courant (racine + un niveau). Chaque clip : `path`, `name`, `game`, `kind`, `modified_ms`, `size_bytes`, `duration_secs`, `fps` (cadence vidéo moyenne, pas d'image du lecteur), `tracks` (titres), `thumbnail`, `favorite` |
| `clip_games` | 6 ✅ | jeux (sous-dossiers) présents dans la bibliothèque |
| `set_clip_favorite { path, favorite }` / `rename_clip { path, name }` / `trash_clip { path }` / `reveal_clip { path }` | 6 ✅ | GAL-05 : favori ; renommer le fichier (mêmes règles de nom que les dossiers de jeux, miniature renommée, favori gardé) ; Corbeille (`SHFileOperationW` + `FOF_ALLOWUNDO`) ; Explorateur sur le fichier. Un chemin absent de la bibliothèque est refusé (`clip_unknown`) |
| `quota_preview { quotaGb }` | 6 ✅ | STO-04 : taille totale et clips qui seraient supprimés (plus anciens, jamais un favori ni un enregistrement en cours) |
| `prepare_player { path }` | 6 ✅ | ADR-008 : pistes audio du clip copiées (sans réencodage) en `.m4a`, une par piste, en une seule lecture du fichier ; cache `%LOCALAPPDATA%\com.palawi.palaclips\cache\tracks\` limité à 2 Go (les moins récemment utilisées sont supprimées, les `.part` nettoyés au démarrage) ; renvoie `[{ title, path }]` |
| `track_peaks { path }` | 6 ✅ | forme d'onde d'une piste du cache : 1 600 pics (0–255, octets bruts), calculés en décodant le `.m4a` une fois puis gardés à côté (`.peaks`) ; chemin hors du cache refusé |
| `copy_clip { path }` | 6 ✅ | GAL-05 : copie le fichier dans le presse-papiers (`CF_HDROP`) pour le coller dans Discord ou l'Explorateur |
| `start_export { export }` / `cancel_export { id }` | 6 ✅ | GAL-06/07 : `{ path, name, start_secs, end_secs, precise, tracks, gains, mix, mix_title }` → id. `gains` : un volume (0–1) par piste choisie, appliqué au mixage (le lecteur envoie ses volumes ; absent = 1). Rapide : copie depuis l'image clé au début ou juste avant ; précis : décodage puis réencodage matériel, ou sur le processeur sans encodeur matériel du codec (même codec, débit de la source) à partir de l'image exacte. Pistes audio choisies (positions parmi les pistes audio), copiées, ou mixées en une piste AAC titrée `mix_title`. Fichier `<name>.mp4` dans le dossier du clip (suffixe `_2`… si pris), écrit en `.part.mp4` puis renommé (la bibliothèque ignore les `.part`). Événements Tauri `export-progress { id, progress }` (pas de 1 %) et `export-finished { id, path, error }` |
| `get_hotkeys` / `set_hotkeys { hotkeys }` | 5 ✅ | raccourcis globaux (APP-05) : `save_replay`, `toggle_replay`, `toggle_recording` (défaut `Ctrl+Alt+F9`) ; refus des touches sans modificateur, des raccourcis système (Alt+F4, Alt+Tab…), de la touche Windows, de Maj + lettre et des doublons ; conflit = `RegisterHotKey` refusé → retour aux raccourcis précédents. `get_hotkeys` renvoie aussi les actions dont le raccourci est indisponible. Limite : les applis qui lisent le clavier sans `RegisterHotKey` (Discord, overlay NVIDIA) ne sont pas détectées |
| `performance_snapshot` | 7 | panneau de performances |
| `export_diagnostics` | 8 | archive zip |

### Événements (app → front) : `EngineEvent`

Émis tels quels sur l'événement Tauri `engine-event` par le fil `palaclips-events`, qui tient
aussi l'`AppStatus` renvoyé par `get_state`.

`state_changed`, `capture_started { source, width, height, fps, tracks: [{ title, kind }] }` (à l'armement ; `source` = `Localized` : `source_primary_screen`, `source_screen { name }`, `source_window { exe }`, `source_window_fallback { exe }` ; `kind` = `mix`, `microphone`, `application` ou `game`, car les pistes sont renommables),
`tracks_renamed { tracks: [{ title, kind }] }` (nom de piste ou langue changés pendant le
replay : titres mis à jour sans redémarrage, appliqués au prochain clip), `game_named { exe_name, name }` (nom de jeu choisi et figé, ADR-024 ; l'app l'ajoute à
`settings.json`), `track_changed { track, title, state: waiting | capturing | silent | unavailable, source }` (appli ou
périphérique rattaché ; émis à chaque changement), `clip_saved`, `audio_levels { levels }` (crête linéaire 0–1 de chaque piste depuis
le relevé précédent, un nombre par piste dans l'ordre des pistes : le front indexe par numéro
de piste, jamais par titre ; 20 Hz fenêtre focalisée, 10 Hz sans focus, coupé
fenêtre réduite ou fermée : `EngineHandle::set_level_rate`, piloté par les événements de la
fenêtre),
Protocole `palaclips` (ADR-007, `register_asynchronous_uri_scheme_protocol`) :
`http://palaclips.localhost/<chemin encodé>` (`convertFileSrc(path, "palaclips")`), sert un clip
de la bibliothèque ou une piste du cache, et rien d'autre (403 ; `..` refusé) ; réponses `206`
par plages, 4 Mo au plus par réponse ; lecture sur un fil bloquant de Tauri.

Événement Tauri `library-changed` (sans contenu) : la bibliothèque a changé (synchro au
démarrage, surveillance du dossier, renommage, Corbeille, quota) ; la galerie relit `list_clips`.

`performance_stats { cpu_percent, cpu_max_percent, engine_bytes, buffer_bytes, buffered_secs,
capture: { encoder, width, height, fps, repeated_percent, late_total, late_last_minute,
lag_max_ms } | null }` (chaque seconde, seulement sur demande, `EngineHandle::set_performance`) :
CPU du processus en % de la machine entière (temps CPU ÷ cœurs logiques, comme le Gestionnaire
des tâches), maximum depuis l'armement ; `engine_bytes` = working set privé − tampon × 1,07
(surcharge mesurée, même calcul que PERF-02 dans `engine-memory.ps1`) ; images répétées en % des
10 dernières secondes ; images en retard (encodées plus d'une période après leur instant) depuis
l'armement et sur la dernière minute ; replay désarmé : CPU et RAM seulement, `capture: null`.
`replay_stats`, `recording_stats { elapsed_secs, bytes }` (chaque seconde pendant un
enregistrement ; chaque fichier terminé arrive en `clip_saved`), `post_roll { ends_in_secs }`
(v1.2 : nombre = clip en cours ou prolongé, secondes restantes ; `null` = clip en cours terminé,
le fichier arrive en `clip_saved` ; le shell joue le carillon et le toast « Clip en cours » et
tient `AppStatus.clip_ends_at_ms`, heure de fin en ms Unix), `alert { severity, code, params, message }` (`code` + `params` pour le texte
affiché, `message` en français pour les journaux et la CLI), `alert_cleared { code }` (la
condition d'une alerte a disparu : l'app la retire de `AppStatus.alerts`). Alertes de capture
(`events::CAPTURE_ALERTS` : `capture_source_rate_limited`, `capture_gpu_saturated`,
`cpu_encoder_overloaded`, `capture_exclusive_fullscreen`) : retirées par `alert_cleared` après 5 s de retour à la normale,
et à chaque démarrage ou arrêt de capture.

Codes d'alerte stables : `capture_fallback_monitor`, `capture_source_lost`, `capture_source_rate_limited`, `capture_gpu_saturated`, `cpu_encoder_overloaded`, `capture_exclusive_fullscreen`, `app_track_silent`,
`audio_device_changed`, `audio_device_unavailable`, `disk_almost_full`, `disk_full`, `encoder_reset`, `no_hardware_encoder`, `no_video_encoder`,
`hotkey_unavailable` (raccourci global déjà pris ; cite l'overlay NVIDIA), `recording_failed { reason }`
(écriture impossible ou disque plein : l'enregistrement s'arrête, les fichiers déjà écrits
restent lisibles), `disk_almost_full { free_mb }` (moins de 2 Go libres pendant un
enregistrement, une fois), `recording_recovered { name }` (enregistrement interrompu remuxé au
démarrage, REC-06), `clip_too_short` (info :
moins de 3 s à sauvegarder, aucun clip ; pas de notification), `calibration_running` (info :
sauvegarde demandée pendant un calibrage, rien n'est sauvegardé), `settings_unreadable` /
`settings_unreadable_kept` (réglages illisibles, mis de côté en `settings.json.bak`),
`settings_newer` (version plus récente, fichier laissé intact), `settings_dir_missing`,
`quota_deleted { count, size_mb }` (clips supprimés définitivement par le quota).
Paramètres : `replay_stopped`, `replay_arm_failed`, `clip_save_failed { reason }`,
`clip_cut_short { reason }` (v1.2 : écriture d'un clip en cours interrompue, disque plein par
exemple ; le début déjà écrit reste lisible) ;
`clip_too_short { seconds }` ; `capture_fallback_monitor { title }` ;
`encoder_reset { reason }` (avertissement : capture, encodage ou piste en panne, par exemple
après un redémarrage du pilote graphique ; le moteur désarme puis réarme seul après 1, 2 puis
5 s, tampon de replay vide, et un enregistrement en cours reprend dans un nouveau fichier ; après
trois échecs, `replay_stopped`, ADR-031) ;
`capture_source_rate_limited { rate, fps }` (cadence de composition DWM sous le fps demandé
pendant 5 s, vérifiée chaque seconde par le fil d'encodage vidéo ; seules comptent les secondes
où Windows livre de nouvelles images, un écran immobile ne déclenche jamais l'alerte, ADR-018) ;
`capture_gpu_saturated` (au moins 10 % des images de chaque seconde encodées plus d'une période
en retard pendant 5 s : le jeu prend tout le GPU ; conseille de limiter ses fps) ;
`cpu_encoder_overloaded` (même mesure avec un encodeur CPU, ticks sautés compris : le processeur
ne suit pas ; conseille de baisser résolution ou fps) ;
`capture_exclusive_fullscreen` (aucune image nouvelle de Windows pendant 5 s alors qu'une appli
a la propriété exclusive de l'écran, `D3DKMTCheckExclusiveOwnership` ; paramètre `advice` :
`exclusive_fullscreen_advice` (plein écran fenêtré ou sans bordure) ou, si l'appli au premier
plan est `javaw.exe`/`java.exe`, `exclusive_fullscreen_advice_minecraft` (fenêtre agrandie ou
mod sans bordure)) ; `audio_device_changed
{ track, device }` ; `app_track_silent { track, app }` ; `audio_device_unavailable { track,
reason }` ; `hotkey_unavailable { keys }` ; `unsupported_windows_build` (au démarrage, si le build
de Windows est connu et inférieur à 20348).

Raccourci global : `tauri-plugin-global-shortcut` 2.4, côté Rust seulement ; `Alt+F10` →
`save_replay` (personnalisable avec la persistance des réglages).

## 7. Données sur le disque

| Quoi | Où |
|---|---|
| Réglages (`settings.json` : `{ schema_version, engine, feedback, hotkeys, theme, language, onboarded }`, format 2 : noms de pistes par défaut à `null`, versionné ; illisible → `.bak` + défauts ; version plus récente → défauts sans écraser) | `%APPDATA%\com.palawi.palaclips\` (ADR-032) |
| Journaux (`tracing` sans couleurs, filtre `warn,palaclips_engine=info,palaclips_app_lib=info` ou `RUST_LOG` ; un fichier `palaclips.AAAA-MM-JJ.log` par jour, 7 gardés ; les paniques y sont écrites) | `%LOCALAPPDATA%\com.palawi.palaclips\logs\` (ou `<dossier>\local\logs\`) |
| Index de la bibliothèque (SQLite, table `clips` : chemin, nom, jeu, type, date, taille, durée, pistes, miniature, favori ; WAL) | `library.db` à côté de `settings.json` (`%APPDATA%\com.palawi.palaclips\`, ou `PALACLIPS_CONFIG_DIR`) ; le dossier des clips reste la source de vérité |
| Miniatures, pistes extraites, proxys | `%LOCALAPPDATA%\com.palawi.palaclips\cache\` (miniatures : `thumbnails\<nom du clip>.jpg`, 480 px, dernière image clé, écrites à la sauvegarde) ; avec `PALACLIPS_CONFIG_DIR`, `<dossier>\local\cache\`, pour qu'aucun fichier de test ne sorte du dossier isolé ; le dossier des miniatures est autorisé au protocole `asset` au démarrage |
| Buffer de replay sur disque | `%LOCALAPPDATA%\com.palawi.palaclips\replay\` (ou `<dossier>\local\replay\`) |
| Migration (premier lancement après ADR-032, à chaque démarrage tant qu'il reste quelque chose à déplacer) | `settings.json`, `.bak`, `library.db` (+ `-wal`, `-shm`), `recordings.txt` de `%APPDATA%\PalaClips\`, puis `cache\` et `replay\` de `%LOCALAPPDATA%\PalaClips\`, déplacés vers les dossiers ci-dessus (`migrate::migrate_data`) ; un fichier déjà présent à l'arrivée n'est jamais écrasé ; chemins des miniatures réécrits dans `library.db` ; seuls ces fichiers et sous-dossiers bougent : un fichier inconnu reste en place, `%APPDATA%\PalaClips` est supprimé seulement s'il est vide, et `%LOCALAPPDATA%\PalaClips` (dossier d'installation) n'est jamais supprimé ni renommé. Jamais en mode isolé |
| Données du WebView2 (localStorage : état du panneau de performances) | `%LOCALAPPDATA%\com.palawi.palaclips\EBWebView\` ; avec `PALACLIPS_CONFIG_DIR`, `<dossier>\webview\` (`data_directory` de la fenêtre) : une app de test ne lit ni n'écrit jamais le stockage réel (`scripts/webview-isolation.ps1` le vérifie par empreinte) |
| Clips | `Vidéos\PalaClips\<Jeu>\` (réglable) |
| Enregistrements en cours (`recordings.txt` : `PID<tab>chemin`, une ligne par fichier ouvert ; au démarrage, ceux d'un processus mort sont remuxés en MP4 classique) | à côté de `settings.json` |
| Programme (installateur NSIS par utilisateur) | `%LOCALAPPDATA%\PalaClips\` : `palaclips-app.exe` (même nom que la build de dev : même instance unique, même entrée « Démarrer avec Windows »), les 7 DLL FFmpeg (LGPL) à côté, `licenses\` (MIT de PalaClips, LGPL v3 et GPL v3 de FFmpeg, `THIRD-PARTY.txt` : version, compilation BtbN, liens vers le code source, DLL remplaçables). Page de licence = MIT + notice tiers. Désinstallation : retire le programme et l'entrée `Run` ; la case « Supprimer aussi les réglages, l'historique et les favoris de la galerie (jamais les clips) » efface `%APPDATA%\com.palawi.palaclips` et `%LOCALAPPDATA%\com.palawi.palaclips` (textes dans `app/src-tauri/nsis/*.nsh`, sans BOM : Tauri en ajoute un) ; le dossier des clips n'est jamais touché |

## 8. Légèreté

- WebView détruit à la fermeture ; `RunEvent::ExitRequested` → `prevent_exit()` ; sortie via
  « Quitter » uniquement.
- Sondes (sessions audio, crêtes) arrêtées quand l'UI est fermée, sauf rattachement en attente.
- Pas de polling : notifications Windows (périphériques audio, fichiers). Exceptions bornées :
  une piste d'appli qui attend son appli cherche le processus chaque seconde (Toolhelp) ; une
  appli rattachée est suivie par son handle de processus (attente, pas de polling) ; la piste
  « Jeu » est réveillée par les événements de premier plan (`SetWinEventHook`).
