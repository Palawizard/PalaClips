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
- **Coût de la capture sur les fps du jeu** (`scripts/fps-impact.ps1`, PresentMon 2.4.1,
  Fortnite en Créatif immobile, plafonné à ~225 i/s, deux passes avec et sans G-Sync,
  2026-10-17) : aucune perte mesurable. Fenêtre 60 i/s, écran 60 i/s, fenêtre 30 i/s et WGC
  seul donnent 224,7–225,0 i/s (1 % bas 178–181 i/s) contre 224,6–224,7 i/s pour l'essai sans
  capture de fin. **Fortnite reste en Independent Flip** (« Hardware: » ou « Hardware
  Composed: Independent Flip ») pendant la capture : WGC ne force pas la composition DWM.
  Retard max d'encodage 0,7–7,7 ms. Le premier essai sans capture est toujours plus bas (212,7
  et 220,1 i/s, mise en route du jeu) et faussait la référence → essai de mise en route non
  compté ajouté au script. Limite : jeu sous son plafond, GPU non saturé ; la marge peut
  masquer le coût de la capture (PERF-03 à valider GPU-bound, PLAN phase 7). Journaux :
  `docs/reports/phase-7/fps-impact-2026-10-17_10-54.txt` et `_10-58.txt`.
- Métadonnées des exe de jeux (2026-10-17, `palaclips-cli game-name`) :
  `FortniteClient-Win64-Shipping.exe` (et `_EAC_EOS`) n'ont **ni ProductName ni
  FileDescription** ; Terraria : « Terraria » ; Rocket League : `Launcher.exe` « Launcher ».
  `QueryFullProcessImageNameW` (PROCESS_QUERY_LIMITED_INFORMATION) lit le chemin de Fortnite
  sous EAC alors que `Get-Process .Path` le refuse. La `ProductName` d'`explorer.exe` contient
  littéralement `&apos;` (« Système d&apos;exploitation ») : entités XML à décoder.
- Hypothèse non vérifiée : la perte de 50–100 i/s vue le 2026-10-16 pendant
  `nvenc-quality.ps1` venait d'autre chose que la capture (réencodages et VMAF lancés en
  parallèle du jeu, CPU et GPU chargés).
- **Capture sous GPU saturé et priorité GPU** (2026-10-20, `palaclips-cli probe-gpu-priority`,
  Windows 11 26200, RTX 4070 Super, HAGS activé, **sans droits admin**, journaux
  `target/journals/gpu-priority-*.txt`). La sonde lance un processus de charge (pixel shader
  de 20 ms par appel de dessin, 2 appels en vol) et mesure la latence d'une conversion
  Video Processor 1440p → NV12 toutes les 16,7 ms, sous chaque réglage :
  - charge **en arrière-plan** (sans fenêtre) : la conversion n'attend jamais plus de 12 ms,
    quel que soit le réglage, et `capture-video` reste à l'heure (2,8 ms de retard max). Le
    blocage vu sous Superposition ne se reproduit pas ;
  - charge **au premier plan** (fenêtre 640×360 qui présente, `--window`) : p50 32–45 ms,
    100 % des conversions au-delà d'une image, et `capture-video` prend **11,2 s** de retard
    (308 images source reçues en 20 s). Windows favorise le GPU du processus au premier plan :
    c'est le cas d'un jeu ;
  - `IDXGIDevice::SetGPUThreadPriority(7)` : accepté et relu à 7, **aucun effet**.
    `D3DKMTSetProcessSchedulingPriorityClass` `ABOVE_NORMAL` et `HIGH` : acceptés, aucun
    effet ; thread 7 + `HIGH` (réglage d'OBS) : aucun effet ;
  - classe **`REALTIME`** : acceptée **sans admin** (relue `REALTIME`) et seule efficace :
    p50 5 ms, 0 % au-delà d'une image. OBS ne la demande qu'en administrateur ; avec HAGS
    activé, Windows 11 l'accorde à un processus normal. Sans HAGS : à mesurer.
  - Correctif (moteur) : `capture::set_gpu_priority()` (classe `REALTIME` si le réglage vaut
    `capture`, `NORMAL` sinon, ADR-033 révisé ; refus journalisé) **et** file vidéo bornée : à chaque réveil, seule
    l'échéance la plus récente reçoit une image convertie, les échéances manquées répètent la
    dernière image encodée (même surface NVENC, aucun travail GPU). Sous la même charge au
    premier plan : retard max 57 ms sans la priorité (aujourd'hui le défaut, 530 images
    source) et 63 ms avec (1 333 images source), 1 200 images CFR sur 20 s, 0 erreur de
    décodage.
  - Alerte `capture_gpu_saturated` sous la même charge, 30 s : sans la priorité, levée au bout
    de 5 s (1 097 images en retard sur 1 800) ; avec, jamais (3 images en retard).
  - Coût sur le jeu (PERF-03, `fps-impact.ps1`, Kombustor fenêtré « Composed: Copy with GPU
    GDI », GPU 100 %, ~84 i/s, journaux `docs/reports/phase-7/fps-impact-2026-10-20_17-*.txt`) :
    temps réel → perte 9,1–9,2 % (60 i/s), **7,5 %** (1440p60 + 4 pistes), 5,2 % (30 i/s),
    0,3 % (WGC seul) ; priorité normale → 0,1 %, 2,7 %, **0,7 %**, −0,3 %, 2,7 % (dérive entre
    essais sans capture jusqu'à 3,5 %), mais ~15 images source par seconde seulement : le
    rappel de WGC attend le contexte D3D11 que tient l'encodeur bloqué sur le GPU.

- **Plein écran exclusif** (2026-10-20, MSI Kombustor Vulkan en plein écran exclusif sur l'écran
  principal, `palaclips-cli probe-fullscreen` pendant `capture-video --target primary`) : la
  capture ne reçoit que **2 images en 60 s** ; `D3DKMTCheckExclusiveOwnership()` = vrai et
  `SHQueryUserNotificationState` = `QUNS_RUNNING_D3D_FULL_SCREEN` les 60 secondes, faux et
  `QUNS_ACCEPTS_NOTIFICATIONS` sur le bureau, avec un navigateur ou un terminal au premier
  plan. Alerte `capture_exclusive_fullscreen` levée 5 s après le gel (aucune image nouvelle
  **et** propriété exclusive). Fermeture de Kombustor pendant une capture : 457 images répétées
  (≈ 15 s) sans alerte, quelques images arrivant encore pendant la sortie du mode exclusif ;
  retrait de l'alerte vérifié par les tests unitaires seulement.
  Explication (2026-10-20) : la cible était l'écran principal, pas la fenêtre de Kombustor ;
  aucun repli ni fermeture de fenêtre ne pouvait jouer. La règle exigeait 5 s consécutives
  sans image, et l'image du démarrage puis quelques images éparses pendant la sortie du mode
  exclusif remettaient le compteur à zéro. Règle révisée : moins de 5 images en 5 s **et**
  exclusif (`D3DKMTCheckExclusiveOwnership` ou `QUNS_RUNNING_D3D_FULL_SCREEN`). Nouvel essai
  Kombustor : alerte levée pendant le gel ; les deux signaux vrais les 49 s de Kombustor au
  premier plan ; Kombustor exclusif repasse l'écran HDR en SDR, et sa fermeture rend le HDR,
  ce qui relance la capture (#67) et retire l'alerte (`phase-7/exclusive-v2-2026-10-20_18-51.txt`).
- **Redimensionnement d'une fenêtre capturée** (2026-10-20, `scripts/resize-leak.ps1`, mire
  ffplay animée, 10 fois 1280×720 ↔ 800×450) : 10 changements de taille vus par le moteur,
  working set privé 36,4 → 41,3 Mo avec un plateau dès le 6e (40,7 / 40,9 / 41,0 / 41,0 /
  41,3) : pas de fuite par changement. Une fenêtre **immobile** ne renvoie plus d'image après
  un redimensionnement : `windows-capture` recrée son frame pool à l'image suivante seulement,
  la nouvelle taille arrive donc avec le premier changement de contenu.

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
- Mesuré le 2026-10-16 sur la machine de Palawi (Windows 11 build 26200, `wasapi` 0.26,
  `probe-audio --app` et `capture-apps`, sources `play-tone` 440/880 Hz lancées depuis des
  copies de la CLI vers « Realtek Digital Output », sortie qui n'est pas celle par défaut) :
  - Process loopback **non muet** : crête de la piste = crête de la session (0,250) ; le
    problème de MeetCap #38 ne se reproduit pas ici avec `wasapi`.
  - Capte l'appli quelle que soit la sortie qu'elle utilise (pas seulement la sortie par
    défaut). Isolation : l'autre fréquence est sous −70 dB, et seulement aux transitions.
  - Paquets de 480 images toutes les 10 ms, `pu64QPCPosition` valide, latence
    réception − QPC 10 à 15 ms ; `GetBufferSize` vaut 480 images ; index de position non
    contigu (un saut par paquet) : ne pas s'en servir.
  - **L'appli fermée, le flux continue** et envoie des zéros sans drapeau : la fin de l'appli
    se détecte par son handle de processus, pas par le flux.
  - Appli fermée puis relancée pendant la capture : nouveau PID retrouvé en ≤ 1 s, piste
    continue (30,000 s) avec le silence entre-temps.
  - Après un rattachement, ~100 à 150 corrections de dérive d'un échantillon sur la piste
    (≈ 2 à 3 ms rattrapés en quelques secondes) ; aucune sur une piste ouverte une seule fois.
    Inaudible ; à surveiller si un rattachement produit un décalage durable.
  - Un client de capture **loopback** crée une session « active » sur la sortie capturée, et
    son `IAudioMeterInformation` reproduit la crête de la sortie (constaté sur
    `palaclips-cli.exe`, Wallpaper Engine et FluentFlyout : 0,200 sans rien jouer). Les
    crêtes de session ne prouvent donc pas qu'une appli joue : PalaClips ignore ses propres
    sessions pour `app_track_silent`.
  - Toolhelp ne donne que le PID du parent, qui peut avoir été réattribué : un lien parent →
    enfant n'est gardé que si le parent a été créé avant l'enfant (`GetProcessTimes`).
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
    ouvert), mais reste possible : le combleur ne doit pas en dépendre. Constaté le
    2026-10-16 sur « Realtek Digital Output » (rien ne joue) : **zéro paquet** en 20 s ; la
    piste se comble désormais toutes les 50 ms (`docs/reports/phase-2/silent-device-fill.txt`).
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
- Synchro A/V mesurée le 2026-10-15 (`capture-av` 60 i/s, écran principal ; détection des
  flashs dans la vidéo et des bips dans la piste mix, script `av_sync.py` de la session) :
  - Vidéo de test lue par VLC (qui compense la latence annoncée du périphérique), sortie
    Voicemeeter, 160 s dont 60 s sans son : +103,7 ms constant (±0,2 ms), aucune dérive,
    synchro intacte après le silence. Même vidéo, sortie Apogee : +1,2 ms en moyenne.
  - Source « comme un jeu » (`probe-av-emitter`, flash et bip émis ensemble, tampon de rendu
    1056 images) : Apogee +15,7 ms constant ; Voicemeeter +103 à +136 ms (moy. +117) ;
    Voicemeeter avec `--mix-offset-ms -100` : +10 à +27 ms (moy. +16,9). Le loopback de
    « Voicemeeter Input » reçoit le son ~100 ms plus tard ; `GetStreamLatency` vaut 0 sur un
    client loopback et ne permet pas de le deviner (ADR-017).
- Retard de la chaîne micro de Palawi d'un lancement à l'autre (2026-10-16,
  `t2-av-sync.ps1 -CalibrateOnly`, `probe-av-emitter` 20 s, écouteur collé au micro physique ;
  chaîne micro physique → Voicemeeter → LightHost (VST) → VB-Cable → « CABLE Output ») :
  - lancement 1 : micro +473,3 ms (+457,8 à +477,0 sur 19 bips), mix +146,2 ms ;
  - après fermeture et relance de LightHost et Voicemeeter : micro +409,9 ms (+396,7 à
    +415,1 sur 19 bips), mix +148,3 ms.
  - **Pas stable** : −63 ms d'un lancement à l'autre, alors qu'il est stable à ±10 ms pendant
    un lancement et que le mix ne bouge que de 2 ms. La valeur mesurée contient aussi la
    sortie de Voicemeeter vers le casque (le bip passe par l'air) : la part qui vient de
    l'entrée micro seule n'est pas isolée.
  - Le retard dépasse ±400 ms : `measure-av-sync.py` apparie désormais chaque flash au
    premier bip entre −200 et +800 ms.
- T2 d'une heure (2026-10-16, `t2-av-sync.ps1`, sortie « Voicemeeter Input », décalages
  calibrés, émetteur 20 s à 0, 30 et 60 min, journal `docs/reports/phase-2/t2-av-sync-1h.txt`) :
  pente par régression **mix −8,4 ms/h** (+4,6 ms à 0 s), **micro −8,8 ms/h**. Calculée sur les
  3 moyennes de fenêtres : `t2.mp4` avait déjà été supprimé (17,8 Go) avant que
  `measure-av-sync.py` fasse la régression sur tous les bips. Incertitude estimée ±1,5 ms/h :
  l'écart d'un bip est quantifié par l'image (±8,3 ms à 60 i/s, dispersion ≈ 4,8 ms), et
  l'erreur-type de la pente vaut dispersion / √Σ(t − t̄)².
- Horloge audio contre QPC (2026-10-16, `scripts/clock-drift.ps1 -Minutes 20`, silence
  numérique joué par `play-tone --amplitude 0` et `probe-audio --device` sur la même sortie ;
  pente de la régression heure de réception − `pu64QPCPosition` sur tous les paquets, journal
  `target/journals/clock-drift-2026-10-16_14-31.txt`) :
  - **Apogee BOOM en direct : −0,01 ± 0,00 ms/h** (120 001 paquets), nombre d'images =
    durée QPC × 48 000 à moins d'un paquet près ;
  - **Voicemeeter Input : +0,32 ± 0,01 ms/h** (120 000 paquets), 0 ppm.
  - `pu64QPCPosition` suit donc le temps réel, pas une horloge d'appareil qui dériverait :
    aucune correction d'horloge à ajouter. La vidéo est exacte contre QPC par construction
    (`CfrClock`, nombres d'images exacts). AUD-08 tient par cette composition et par le T2
    d'une heure vert.
- Pentes de `av-drift.ps1` (2026-10-16, 20 min, émetteur et loopback sur la même sortie) :
  Apogee directe −19,4 ± 0,7 ms/h, Voicemeeter Input −2,3 ± 2,0 ms/h, et −8,4 ms/h au T2.
  Différentes à chaque essai alors que l'audio et la vidéo suivent QPC : elles viennent
  probablement du banc (battement entre le rafraîchissement de l'écran et QPC, écart borné
  qui ressemble à une pente sur 20 min). **Hypothèse non prouvée.** `av-drift.ps1` ne sert
  plus à mesurer une pente ; utiliser `clock-drift.ps1`.
- Les noms GDI `\\.\DISPLAYn` changent d'un moment à l'autre de la session (constaté le
  2026-10-15 : l'écran principal est passé de `DISPLAY2` à `DISPLAY1`) : ils ne suffisent pas
  à mémoriser un écran (CAP-03).
- Jeux qui ouvrent le périphérique en mode exclusif : la capture loopback échoue
  (`AUDCLNT_E_DEVICE_IN_USE`), il faut réessayer et le signaler. — [ClipFlow CHANGELOG](https://github.com/jaimitus/ClipFlow/blob/main/CHANGELOG.md)

- Deuxième session WGC dans le même processus (2026-10-16, redémarrage du replay) : crash
  (violation d'accès) dans `start_free_threaded`, sur un appel de méthode dont la table est
  dans `WinTypes.dll` **déchargée**. windows-rs garde les fabriques WinRT en cache pour tout
  le processus ; quand la première session et les pistes audio s'arrêtent, plus aucun thread
  n'est dans le MTA, COM le démonte et décharge `WinTypes.dll`. Seulement après une piste
  audio (capture vidéo seule : 3 sessions d'affilée sans souci). Correctif :
  `CoIncrementMTAUsage` une fois pour toute la vie du processus (`keep_mta_alive`). —
  [Microsoft](https://learn.microsoft.com/windows/win32/api/combaseapi/nf-combaseapi-coincrementmtausage)
- Machine de Palawi : TranslucentFlyouts (`TFMain64.dll`, `dbghelp-for-tf.dll`) et un mod
  Windhawk (`translucent-windows`) sont injectés dans les processus PalaClips. Le gestionnaire
  de TF transforme un crash en boîte « this application has thrown an uncaught exception »
  et écrit un minidump dans son dossier `dumps` : un processus qui semble bloqué peut être un
  crash en attente de réponse. Lire ces dumps avec `cdb -z <dump> -c ".ecxr; kn; lmu; q"`.

- DaVinci Resolve (machine de Palawi, 2026-10-16) n'affiche pas les noms de pistes audio
  d'un MP4 ni d'un MOV, même présents dans `hdlr` et `udta/name` de chaque piste : « Audio 1 »
  à « Audio N » dans la timeline, les attributs du clip et les métadonnées. Pistes séparées et
  alignées. Un fil de 2018 (Resolve 15.3) les voyait dans la colonne de métadonnées ; plus le
  cas ici. — [forum Blackmagic](https://forum.blackmagicdesign.com/viewtopic.php?p=413159)

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

### Calibrage intégré (AUD-11, mesuré le 2026-10-17, machine de Palawi)

`palaclips-cli calibrate --settings settings.json` (même chemin que Réglages > Audio) : 12,5 s
au total, 9 paires flash/bip par piste. Mix « Voicemeeter Input » : +138,0 ms (dispersion
0 ms) ; piste Jeu (capture par processus de l'émetteur) : +33,4 ms ; micro « CABLE Output » :
non mesuré au casque.
- Lecture corrigée (ADR-027) : les +33,4 ms de la piste Jeu sont le tampon de rendu de
  l'émetteur lui-même (même nature que les +15,7 ms de l'Apogee plus haut), présent dans
  toutes les pistes et propre à chaque jeu : ce n'est pas à compenser. Seul l'excédent d'une
  piste sur cette référence vient de la chaîne : Mix +138,0 − 33,4 ≈ **+105 ms** propres à
  Voicemeeter (cohérent avec les ~100 ms du loopback virtuel mesurés plus haut) → proposé
  −105 ms ; Jeu et applis 0 ms.
- La première version proposait −138 et −33 ms (alignement sur zéro) : faux, rien n'a été
  appliqué.
- Depuis l'app (config isolée, même machine, quelques minutes plus tard) : Mix +148 ms, Jeu
  (référence) +45 ms → Mix −103 ms, Jeu et applis 0 ms ; 14 s de bout en bout. Le tampon de
  l'émetteur varie (33 → 45 ms) mais l'excédent de Voicemeeter reste ~103–105 ms : la
  référence absorbe bien ce qui n'est pas la chaîne.

### Aperçus en direct du sélecteur de capture (mesuré le 2026-10-17)

`palaclips-cli preview --target <cible> --seconds <n> --out <dossier>` : une session WGC par
cible avec `MinimumUpdateInterval` à 900 ms, copie de la trame, réduction NV12 par le
processeur vidéo D3D11 (le même que la capture), lecture d'une petite surface (≤ 384×216)
puis JPEG : ~1 image/s, 10–13 Ko par image, couleurs correctes. Dans l'app, 3 écrans +
6 fenêtres en aperçu coûtent ~7 % d'un cœur (0,35 % de la machine, i5 13e gen) et
retombent à 0 % en quittant la section. Les icônes viennent de `SHDefExtractIconW` (48 px),
encodées en PNG par FFmpeg (encodeur présent dans la build LGPL).

### Taille de sortie acceptée par NVENC (mesuré le 2026-10-17, RTX 4070 Super)

`palaclips-cli try-output --codec <c> --width <l> --height <h> [--fps <n>] [--skip-limit]`
ouvre réellement l'encodeur et encode 5 images :

| Codec | Accepté | Refusé (« Provided device doesn't support required NVENC features ») |
|---|---|---|
| H.264 | 4096×2304, 4096×4096 à 240 i/s, 2560×1440 à 240 i/s | 4098×1152, 5120×1440 |
| HEVC | 8192×4320 | 8194×2160 |
| AV1 | 7680×4320 | — |

La limite est donc un côté maximal (4096 en H.264, 8192 en HEVC et AV1), pas un nombre de
pixels ni un débit d'images. L'app la vérifie d'abord (message clair sans ouvrir
l'encodeur), puis fait l'essai réel avant d'appliquer un changement vidéo ou de capture. Un
écran super-ultrawide 5120×1440 en natif exige HEVC ou AV1.

### Mémoire du pipeline (PERF-02, mesuré le 2026-10-16)

Machine de Palawi, pilote NVIDIA 617.42, 1440p60 `h264_nvenc` 40 Mb/s, 4 pistes ; mémoire
privée (`PrivateUsage`, engagée) en Mo de 10⁶ octets. Outils : `palaclips-cli probe-memory`
(étape par étape, tas / `VirtualAlloc` / image) et `scripts/engine-memory.ps1` (étapes (a)–(d)).

- **Le pilote NVIDIA engage en mémoire système la taille de chaque texture D3D11**, sans la
  toucher : 10 textures NV12 2458×1440 = +55 Mo de mémoire privée, +1,2 Mo de working set.
  Toute texture (pool WGC, copie de l'image, pool NV12 de FFmpeg) se paie donc deux fois en
  mémoire engagée (VRAM + réserve système), mais pas en RAM occupée.
- Premier périphérique D3D11 du processus : +52 Mo (DLL du pilote copiées à l'écriture, tas et
  blocs du pilote). Le sondage des encodeurs laisse ~+40 Mo après libération de son
  périphérique ; le supprimer ne fait gagner que ~5 Mo au régime (183 au lieu de 188 Mo) : le
  pilote reste chargé de toute façon.
- **Session NVENC 1440p avec les réglages par défaut de FFmpeg : +155 Mo à l'ouverture**, dont
  21 blocs de 5,4 Mo (taille d'une image NV12) ; +19 Mo de plus en régime (textures en vol).
  La taille du DPB (images de référence) domine : `dpb_size=1` → +78 Mo, `dpb_size=2` → +85 Mo.
  `surfaces` (défaut 4) et `delay` (défaut 3, `async_depth`) ajoutent ~2 blocs par surface.
  `tune=ll`/`ull` ne changent rien. Le Video Processor (conversion + mise à l'échelle) et les
  encodeurs AAC (0,75 Mo chacun) sont négligeables.
- Qualité (PSNR moyen, zoom de Mandelbrot 2560×1440 à 60 i/s, 40 Mb/s CBR p4, 8 s) : DPB auto
  41,03 dB, `dpb_size=2` 40,72 dB (−0,3), `dpb_size=1` 40,24 dB (−0,8). SSIM 0,9909 / 0,9904 /
  0,9896.
  Sur une vraie partie de Fortnite (30 s 1440p60, référence 150 Mb/s, `nvenc-quality.ps1`) :
  défauts VMAF 99,37 / PSNR 40,44 dB / SSIM 0,97115, ADR-022 99,37 / 40,43 / 0,97112 : la
  source synthétique exagérait l'écart. Retard max d'encodage jeu en charge : 0,79 ms.
- Pipeline court (`surfaces=2`, `delay=1`) : retard max des ticks 3,7 ms à 60 i/s (budget
  16,7 ms) mais 7,6–15,8 ms à 240 i/s contre 1,3–1,4 ms avec les défauts (toutes les images
  produites dans les deux cas). `surfaces=3,delay=2` : 3,3–4,6 ms à 240 i/s.
  → ADR-022 : `dpb_size=2` partout, `surfaces=2,delay=1` jusqu'à 60 i/s, défauts au-delà.
- 4 pistes audio : +30 Mo, surtout côté WASAPI/AudioSes (nos tampons < 1 Mo par piste, AAC
  0,75 Mo).
- Tampon du replay : la mémoire réelle dépasse de ~7 % le total des paquets journalisé
  (297 Mo contre 276–279 Mo pour 55 s de plus) : en-têtes, arrondis et fragmentation du tas.
  Les paquets ne sont jamais copiés : la sauvegarde les partage par refcount
  (`av_packet_clone`).
- Résultat ((c) = replay 5 s, (d) = replay 60 s) : (c) 321 → 213 Mo, (d) 616 → 510 Mo
  (−110 Mo). Hors tampon : 172 Mo engagés, dont ~136 Mo pour le pilote (périphérique 52 +
  session NVENC 63 + réserve des textures ~22) ; **61 Mo de RAM réellement occupée** (working
  set privé de (c) moins le tampon). Avec 17 Mo pour le processus à vide, 150 Mo engagés sont
  hors d'atteinte quoi qu'on fasse de notre côté : PERF-01/02 se mesurent en working set
  privé (ADR-023).

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
- `tauri-plugin-global-shortcut` : `Shortcut::new` n'est pas `const` (fonction à la place) ;
  enregistrement et gestion côté Rust seulement, sans permission ni paquet npm. — constaté le
  2026-10-16
- Crêtes audio par piste relevées par le pipeline toutes les 50 ms : 19,8 événements/s
  mesurés (`replay --levels`), un son de test d'amplitude 0,25 lu à −12,0 dB. — mesuré le
  2026-10-16
- Relire un MP4 de PalaClips avec FFmpeg (`avformat_open_input`) journalise « UDTA parsing
  failed retrying raw » par piste audio, puis lit bien le titre (`title`/`handler_name`) :
  bénin. Miniature 480 px de la dernière image clé : ~15 Ko, écrite après la fermeture du MP4
  (latence de sauvegarde inchangée, 16 ms en 5 s 1080p). — constaté le 2026-10-16

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
