# Recherche — faits vérifiés et pièges connus

> Faits vérifiés le 2026-10-15 avant la création du repo. Chaque point a sa source.
> Ajouter ici tout nouveau fait vérifié pendant le dev (avec la source et la date).

## Horloges

- `Direct3D11CaptureFrame.SystemRelativeTime` = heure QPC à laquelle le compositeur a rendu
  l'image. — [Microsoft](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.direct3d11captureframe.systemrelativetime)
- `IAudioCaptureClient::GetBuffer` renvoie `pu64QPCPosition`, converti en unités de 100 ns
  (`10 000 000 × t / f`). — [Microsoft](https://learn.microsoft.com/en-us/windows/desktop/api/Audioclient/nf-audioclient-iaudiocaptureclient-getbuffer)
- En process loopback, la position device peut rester à 0 ; un projet a dû placer les paquets
  par leur QPC. — [MeetCap #38](https://github.com/lybym/MeetCap/issues/38)

## Capture vidéo (`windows-capture` 2.0.1)

- `Frame` expose `as_raw_texture()` (texture D3D11), `timestamp()`, `device()`, `dirty_regions()`.
  — [docs.rs Frame](https://docs.rs/windows-capture/latest/windows_capture/frame/struct.Frame.html)
- `Settings::new(item, cursor, draw_border, secondary_window, minimum_update_interval,
  dirty_region, color_format, flags)`. — [docs.rs Settings](https://docs.rs/windows-capture/latest/windows_capture/settings/struct.Settings.html)
- `Window::enumerate()`, `is_valid()`, `process_id()`, `process_name()`, `monitor()`.
  — [docs.rs Window](https://docs.rs/windows-capture/latest/windows_capture/window/struct.Window.html)
- WGC ne livre une image que quand l'écran change (README historique : « Only Updates The Frame
  When Required ») → pacer CFR nécessaire.
- Plein écran exclusif « legacy » : la capture de fenêtre peut échouer, un autre projet se
  rabat sur l'écran. — [WKI-Clipper](https://github.com/WKImods/WKI-Clipper)
- Vérifié le 2026-10-15 sur la machine de Palawi (`palaclips-cli probe-wgc`) : le périphérique
  D3D11 créé par `windows-capture` (sans `D3D11_CREATE_DEVICE_VIDEO_SUPPORT`) expose
  `ID3D11VideoDevice` et `ID3D11VideoContext1` → conversion NV12 sur le même périphérique.
- Même sonde : `SystemRelativeTime` est bien sur l'horloge QPC mais peut **devancer** le QPC
  lu à la réception de l'image (jusqu'à ~7 ms à 240 Hz) → ne pas choisir les images en
  comparant leur horodatage à l'heure du tick.
- Même sonde : `MinUpdateInterval` par défaut ≈ 12,5 ms (~80 Hz max) ; une valeur
  personnalisée (1–4 ms) laisse passer 240 Hz. On règle la moitié de la période cible.
- `windows-capture` : pool d'images à **1** tampon recréé quand `ContentSize` change ; le seul
  `Map` de staging est dans `Frame::buffer()` / `buffer_crop()` (jamais appelés).
- `Window::process_name()` de `windows-capture` ouvre le processus avec `PROCESS_VM_READ`
  (refusé sur les processus protégés) → on lit le nom d'exe par
  `QueryFullProcessImageNameW` avec `PROCESS_QUERY_LIMITED_INFORMATION`.
- Fenêtre réduite : WGC n'envoie aucune image → repli sur l'écran après 3 s sans image
  (constaté le 2026-10-15).
- HDR (vérifié le 2026-10-15, `palaclips-cli probe-hdr`, écran principal en HDR, blanc SDR
  280 nits, « HDR automatique » actif) : en `B8G8R8A8`, Windows applique sa propre courbe qui
  éclaircit les tons moyens (scRGB 0,75 → 0,57 au lieu de 0,50 en sRGB fidèle) et compresse
  les blancs → vidéo délavée. En `R16G16B16A16_FLOAT`, on reçoit le scRGB linéaire (1,0 =
  80 nits ; blanc SDR = `SDRWhiteLevel / 1000` ; jusqu'à 16,9 avec HDR automatique).
- `DisplayConfigGetDeviceInfo` : `GET_SDR_WHITE_LEVEL` (`SDRWhiteLevel` en millièmes de
  80 nits), en passant par `GET_SOURCE_NAME` pour relier le nom GDI `\.\DISPLAYn` au chemin
  d'affichage.
  — [Microsoft](https://learn.microsoft.com/windows/win32/api/wingdi/ns-wingdi-displayconfig_sdr_white_level)
- Détection du HDR : `advancedColorEnabled` (`GET_ADVANCED_COLOR_INFO`, bit 1) vaut aussi 1
  sur un écran SDR avec la gestion automatique des couleurs (ACM)
  ([xbmc/xbmc#26140](https://github.com/xbmc/xbmc/issues/26140)). On lit
  `GET_ADVANCED_COLOR_INFO_2` (type 15, absent de `windows` 0.62 : structure déclarée
  d'après `wingdi.h` du SDK 10.0.26100) et HDR ⇔ `activeColorMode == HDR (2)` ; si l'appel
  échoue, repli sur `advancedColorEnabled && !wideColorEnforced`. Mesuré le 2026-10-15
  (`probe-hdr`, écran 2 GO27Q24, Windows 11 build 26200) :

  | État | `INFO` (bits) | `INFO_2` (bits) | `activeColorMode` | `SDRWhiteLevel` |
  |---|---|---|---|---|
  | HDR on, ACM on | `0011` | `11110011` | 2 (HDR) | 3500 (280 nits) |
  | HDR on, ACM off | `0011` | `01110011` | 2 (HDR) | 3500 (280 nits) |
  | HDR off, ACM on | `0111` | `11010111` | 1 (WCG) | 1000 (80 nits) |
  | HDR off, ACM off | `0001` | `01010001` | 0 (SDR) | 1000 (80 nits) |

  Bits de `INFO` (poids faible à droite) : supported, enabled, wideColorEnforced,
  forceDisabled. Bits de `INFO_2` : supported, active, —, limitedByPolicy, hdrSupported,
  hdrUserEnabled, wcgSupported, wcgUserEnabled. HDR off + ACM on est bien le cas piégé
  (`enabled = 1`), et `wideColorEnforced = 1` y suffit au repli. Le réglage ACM n'est pas
  affiché quand le HDR est actif (il se règle HDR désactivé, dans « Profil de couleurs ») ; il
  reste mémorisé dessous (`wcgUserEnabled`).
- Mise à l'échelle Windows à 150 % : WGC capture l'écran en pixels physiques (1920×1080),
  vérifié le 2026-10-15.
- Fortnite (EAC) en fenêtré et en plein écran : la capture de fenêtre WGC fonctionne sans
  repli (2560×1440 en plein écran), fluide à 60 i/s. — 2026-10-15
- Débit de WGC à 240 Hz (2026-10-15, `probe-wgc`, écran 2 à 240 Hz, écran 1 à 165 Hz,
  écran 3 à 60 Hz, RTX 4070 Super) : vidéo de test à 240 i/s dans VLC → 232 i/s reçus
  (écran) et 227 i/s (fenêtre). Fortnite à 225 i/s (plafond Reflex) **avec G-Sync** → 165 à
  175 i/s reçus, que la cible soit la fenêtre ou l'écran, en plein écran comme en plein écran
  fenêtré, et sans encodage ; l'horloge de composition DWM (`DwmGetCompositionTimingInfo`)
  annonce alors 164,9 Hz, celle de l'écran 1. **Sans G-Sync** → 219 i/s reçus, horloge DWM
  240 Hz. Avec la VRR, l'écran du jeu n'a plus de cadence fixe et DWM se cale sur un autre
  écran ; WGC ne livre qu'une image par composition. Écartés par mesure : nombre de tampons
  du frame pool (`windows-capture` en crée 1, essai à 3 identique) et fréquence de l'écran 1
  (à 60 Hz, 175 i/s reçus). Aucun contournement sans injection.
- La bordure jaune (`IsBorderRequired`) est dessinée par DWM à l'écran, **pas** dans l'image
  capturée (constaté le 2026-10-15).
- Git Bash réécrit `\\.\DISPLAY1` (perte d'une barre oblique) : lancer la CLI depuis
  PowerShell pour les cibles `monitor:` (constaté le 2026-10-15).

## Audio (`wasapi` 0.26)

- `AudioClient::new_application_loopback_client(process_id, include_tree)` ; l'exemple cible
  le **parent** pour capturer tout l'arbre. — [record_application.rs](https://github.com/HEnquist/wasapi-rs/blob/master/examples/record_application.rs)
- ⚠️ Cet exemple arrête la capture après 3 s sans événement : à ne pas reproduire.
- Énumération des sessions + crête : `get_iaudiosessionmanager()` →
  `get_audiosessionenumerator()` → `get_process_id()`, `get_audiometerinformation().get_peak_value()`.
  — [processes.rs](https://github.com/HEnquist/wasapi-rs/blob/master/examples/processes.rs)
- `PROCESS_LOOPBACK_MODE` : inclure **ou** exclure **un** arbre de processus ; Windows 10
  build 20348 minimum. — [Microsoft](https://learn.microsoft.com/windows/win32/api/audioclientactivationparams/ne-audioclientactivationparams-process_loopback_mode)
- Loopback système : aucune donnée quand rien ne joue (comportement normal de WASAPI) → combler
  par du silence. — [forum Audacity](https://forum.audacityteam.org/t/audacity-stops-recording-during-silence-solved/53092)
- Navigateurs multi-processus : cibler le bon arbre. — [LiveTranscriber #2](https://github.com/avysotsky/LiveTranscriber/issues/2)
- Piste process loopback **entièrement à zéro** sur Windows 11 build 26200 avec NAudio 3.1.0 ;
  cause non établie (wrapper ou Windows), une seule machine. — [MeetCap #38](https://github.com/lybym/MeetCap/issues/38)
- Mesuré le 2026-10-15 avec `palaclips-cli probe-audio` (`wasapi` 0.26, flux partagé piloté
  par événement, 48 kHz stéréo f32 `autoconvert`, tampon 200 ms ; journaux bruts dans
  `docs/reports/phase-2/`), machine de Palawi (sortie par défaut « Voicemeeter Input »,
  interface USB Apogee BOOM, micro par défaut « CABLE Output ») :
  - Un paquet de 480 images toutes les 10 ms ; `pu64QPCPosition` tremble de ±1,5 à 2 ms
    d'un paquet à l'autre (Voicemeeter), ±0,55 ms (Apogee), sans dérive cumulée : sur 176 s,
    nombre d'images = durée QPC × 48 000 à 0 ppm. Ne jamais recaler paquet par paquet.
  - Latence réception − QPC : ~17 ms (Voicemeeter), **−10 à 0 ms** sur l'Apogee (horodatage
    en avance sur la réception), ~20 ms (micro virtuel).
  - Musique en pause : le flux continue avec des paquets de **zéros sans
    `AUDCLNT_BUFFERFLAGS_SILENT`** (Voicemeeter comme Apogee) ; aucun réveil manqué. Le cas
    « aucun paquet quand rien ne joue » ne s'est pas produit ici (les applis gardent leur flux
    ouvert), mais reste possible : le combleur ne doit pas en dépendre.
  - Zéros sans drapeau aussi sur un micro virtuel muet : la détection de piste muette ne peut
    pas se fier au drapeau SILENT.
  - Changement du périphérique par défaut : `OnDefaultDeviceChanged` (Console puis
    Multimedia) ; l'ancien flux continue de recevoir des données, c'est à nous de rouvrir
    (~210 ms sans audio). Débranchement USB du périphérique ouvert : `OnDeviceStateChanged
    → NotPresent`, puis `GetBuffer` → `AUDCLNT_E_DEVICE_INVALIDATED` (`0x88890004`) 12 ms
    plus tard, puis Windows bascule seul le défaut. Les premiers paquets d'un flux rouvert
    peuvent **chevaucher** la fin du précédent (jusqu'à 20 ms) : à couper.
  - Une seule `DATA_DISCONTINUITY`, sur le premier paquet de chaque flux ; jamais de
    `TIMESTAMP_ERROR`.
  - `read_from_device_to_deque` fait un `unwrap()` sur `ReleaseBuffer` (panique si le
    périphérique disparaît) : utiliser `read_from_device`.
- Jeux qui ouvrent le périphérique en mode exclusif : la capture loopback échoue
  (`AUDCLNT_E_DEVICE_IN_USE`), il faut réessayer et le signaler. — [ClipFlow CHANGELOG](https://github.com/jaimitus/ClipFlow/blob/main/CHANGELOG.md)

## Encodage et fichiers

- `ffmpeg-next` est en mode maintenance seulement ; `rsmpeg` suit FFmpeg 6/7/8 mais à rythme
  lent. — [état des crates média Rust 2026](https://dev.to/yeauty/the-state-of-media-processing-in-rust-2026-what-each-crate-actually-covers-2k2c)
- `rsmpeg 0.18.0+ffmpeg.8.0` (août 2025) dépend de `rusty_ffmpeg ^0.16.7`, feature par défaut
  `ffmpeg8`. `rusty_ffmpeg 0.17.0+ffmpeg.8.1` existe (avril 2026, feature `ffmpeg8_1`,
  `use_prebuilt_binding`). — crates.io
- `rusty_ffmpeg` sous Windows : `FFMPEG_INCLUDE_DIR` (bindgen, libclang), `FFMPEG_DLL_PATH`
  (liaison dynamique, `.lib` à côté des `.dll`) ou `FFMPEG_LIBS_DIR` (statique), ou vcpkg.
  — [rusty_ffmpeg](https://github.com/CCExtractor/rusty_ffmpeg)
- Build LGPL partagée : `ffmpeg-n8.1-latest-win64-lgpl-shared-8.1.zip`. — [BtbN releases](https://github.com/BtbN/FFmpeg-Builds/releases/latest)
- Vérifié le 2026-10-15 sur la machine de Palawi : `rsmpeg 0.18` compile avec les en-têtes
  FFmpeg **n8.1.3** (BtbN) à condition que bindgen soit ≥ 0.72.1 ; `h264_nvenc`, `hevc_nvenc`,
  `av1_nvenc` s'ouvrent (RTX 4070 Super, pilote 617.42). — `palaclips-cli ffmpeg-info`
- bindgen < 0.72.1 + libclang ≥ 22 : structs déclarées en avance puis définies générées
  opaques (`_address: u8`) ; `rusty_ffmpeg 0.16.x` épingle bindgen 0.71, `0.17` passe à 0.72.
  — [rust-bindgen #3264](https://github.com/rust-lang/rust-bindgen/issues/3264)
- `rsmpeg` : aucun commit depuis le 2025-08-24 (0.18.0, toujours `rusty_ffmpeg 0.16.7`).
  — [larksuite/rsmpeg](https://github.com/larksuite/rsmpeg)
- clang autodétecte la dernière installation Visual Studio même sans outils C++ et ne trouve
  alors ni MSVC ni le Windows SDK (`errno.h` introuvable) ; `VCToolsInstallDir` ou `INCLUDE`
  le contournent. — constaté le 2026-10-15
- Liaison dynamique : les exécutables (tests compris) exigent les DLL FFmpeg dans le PATH
  (`STATUS_DLL_NOT_FOUND` sinon) ; runner Cargo `scripts/with-ffmpeg.cmd`. — constaté le 2026-10-15
- FFmpeg 8.1 : `h264_qsv`/`hevc_qsv`/`av1_qsv` n'acceptent que des frames `qsv` → dériver un
  périphérique QSV du D3D11VA, un contexte de frames QSV du pool D3D11
  (`av_hwframe_ctx_create_derived`, pool dynamique géré par `qsv_dynamic_frames_derive_to`),
  puis `av_hwframe_map` par image. Non testable sur la machine de Palawi (13600KF sans iGPU).
  — [hwcontext_qsv.c n8.1](https://github.com/FFmpeg/FFmpeg/blob/n8.1/libavutil/hwcontext_qsv.c)
- Sans pilote AMD, `*_amf` échoue proprement (« DLL amfrt64.dll failed to open »).
- `rsmpeg 0.18` : `AVCodecContext::open(Some(dict))` libère deux fois le dictionnaire quand
  `avcodec_open2` échoue (FFmpeg a déjà remplacé `*options`) → plantage. On règle les options
  privées par `av_opt_set` sur `priv_data` et on ouvre sans dictionnaire. — constaté le 2026-10-15
- NVENC accepte les frames `d3d11` d'un pool dynamique (`initial_pool_size = 0`) avec
  `BindFlags = D3D11_BIND_RENDER_TARGET` (requis par la sortie du Video Processor).
- Capture 1440p60 → `h264_nvenc` : 0,4 % du CPU de la machine (7,8 % d'un cœur, build debug),
  125 Mo de mémoire de travail. Premier `send_frame` NVENC ≈ 15–50 ms, rattrapé par des
  doublons ; ensuite retard des ticks < 1 ms avec `std::thread::sleep`. — mesuré le 2026-10-15
- MP4 fragmenté brut : mal supporté par éditeurs/lecteurs ; OBS a créé le « Hybrid MP4 »,
  compatible DaVinci Resolve. — [OBS](https://obsproject.com/blog/obs-studio-hybrid-mp4)
- FFmpeg : `movflags hybrid_fragmented` (fragmenté pendant l'écriture, normal à la fin).
  — [FFmpeg](https://ffmpeg.org/pipermail/ffmpeg-cvslog/2024-June/144076.html)
- AAC a des échantillons d'amorçage (priming) : mal gérés, ils décalent l'audio de quelques ms.
  — [OBS, même article](https://obsproject.com/blog/obs-studio-hybrid-mp4)
- DaVinci Resolve gère mal le framerate variable : sortir en CFR. — [forum Blackmagic](https://forum.blackmagicdesign.com/viewtopic.php?p=474782)

## UI (Tauri 2)

- Versions stables au 2026-10-15 : `tauri` 2.12.4, `tauri-build` 2.7.3, CLI npm 2.12.3 ;
  plugins `global-shortcut` 2.4.1, `single-instance` 2.5.4, `autostart` 2.7.0,
  `notification` 2.7.0, `opener` 2.7.2, `dialog` 2.8.3, `log` 2.10.1, `updater` 2.14.0,
  `process` 2.4.0. ⚠️ Des 3.0.0-alpha existent : ne pas les prendre.
- `RunEvent::ExitRequested { api, .. } => api.prevent_exit()` garde l'app vivante sans fenêtre.
  — [Tauri](https://github.com/tauri-apps/tauri/discussions/2684)
- Tauri 2.12 : écouteurs d'événements nettoyés à la destruction d'une fenêtre (fuites corrigées).
  — [release 2.12.0](https://github.com/tauri-apps/tauri/releases/tag/tauri-v2.12.0)
- Une app Tauri pour gamers détruit le WebView en zone de notification : un seul processus natif.
  — [WindowAutoLayout](https://github.com/Riqqqque/WindowAutoLayout/wiki/Performance-And-Gaming)
- WebView2 fenêtre ouverte : mémoire comparable à Electron. — [Tauri #5889](https://github.com/tauri-apps/tauri/issues/5889)
- Asset protocol Tauri v1 : plantage sur vidéos de plusieurs Go. — [Tauri #6375](https://github.com/tauri-apps/tauri/issues/6375)
- Sélection de piste audio dans une `<video>` Chromium : seulement avec « Experimental Web
  Platform features ». — [asbplayer](https://docs.asbplayer.dev/docs/guides/audio-track-selection)
- `tauri icon` accepte un SVG source (vérifié localement le 2026-10-15).

## Skills de design

- impeccable : `/plugin marketplace add pbakaus/impeccable` puis installer depuis `/plugin`,
  ou `npx impeccable install` à la racine du projet ; puis `/impeccable init`.
  — [pbakaus/impeccable](https://github.com/pbakaus/impeccable)
- emil-design-eng : `npx -y skills add emilkowalski/skill --skill emil-design-eng --agent claude-code`.
  — [claudemarketplaces](https://claudemarketplaces.com/skills/emilkowalski/skill/emil-design-eng)

## Projets voisins utiles à lire

| Projet | Intérêt |
|---|---|
| [GeniusClip](https://github.com/GandalfDark/geniusclip) (GPL-3, Rust + Tauri) | DXGI → NV12 GPU → FFmpeg → ring buffer ; option CFR pour éditeurs |
| [ClipFlow](https://github.com/jaimitus/ClipFlow) (MIT, Rust + Tauri) | pièges : mode exclusif, focus, privacy mode |
| [WKI-Clipper](https://github.com/WKImods/WKI-Clipper) (MIT, .NET) | process loopback « game-only », repli plein écran exclusif |
| [ClipLite](https://github.com/ConnorSawaya/ClipLite) (MIT, C++) | pistes par processus + éditeur |

⚠️ Ne pas copier de code GPL-3 (GeniusClip) dans PalaClips (MIT) : s'en inspirer seulement.
