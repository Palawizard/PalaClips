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
