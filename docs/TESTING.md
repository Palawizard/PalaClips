# Protocoles de test

> Tests automatiques : `cargo test --workspace` (CI). Ce fichier décrit les **tests manuels**
> (👤 dans `docs/PLAN.md`). Chaque résultat va dans `docs/reports/phase-N.md` avec la date, la
> version, la machine et la preuve (capture, sortie de commande, fichier de mesure).

## Outils

- `ffprobe` (dans `third_party/ffmpeg/bin`) : fps, durées, pistes.
- `ffmpeg -i clip.mp4 -map 0:a:N -af astats -f null -` : niveau d'une piste (zéro numérique ?).
- MediaInfo : « Frame rate mode : Constant ».
- DaVinci Resolve (version gratuite) pour les imports.

## T1 — Vidéo (phase 1)
Depuis **PowerShell** à la racine du repo (Git Bash abîme `\\.\DISPLAYn`) :

```powershell
cargo build -p palaclips-cli
$env:PATH = "$PWD\third_party\ffmpeg\bin;$env:PATH"
$cli = ".\target\debug\palaclips-cli.exe"
New-Item -ItemType Directory -Force target\clips | Out-Null
```

1. `& $cli capture-video --target primary --fps 60 --seconds 10 --out target\clips\t1.mp4`
2. `ffprobe -v error -select_streams v:0 -show_entries stream=r_frame_rate,avg_frame_rate,nb_frames,duration -of compact target\clips\t1.mp4`
3. Attendu : `r_frame_rate = avg_frame_rate = 60/1`, `nb_frames = 600 ± 1`.
4. Fortnite **fenêtré** : lancer Fortnite en mode fenêtré, `& $cli list-targets` pour noter
   la ligne `window:<exe>`, puis
   `Start-Sleep 10; & $cli capture-video --target window:FortniteClient-Win64-Shipping.exe --fps 60 --seconds 30 --out target\clips\fn_fenetre.mp4`
   (10 s pour revenir dans le jeu et bouger la caméra). Rapporter : la dernière ligne de la
   CLI, la sortie `ffprobe` du point 2 sur ce fichier, et à la lecture : image correcte ?
   saccades ?
5. Fortnite **plein écran** : même commande avec `--out target\clips\fn_plein.mp4`. Rapporter en
   plus si la CLI affiche « … ne peut pas être capturée : PalaClips capture son écran à la
   place ».
6. Haute cadence pendant une partie :
   `Start-Sleep 10; & $cli capture-video --target primary --fps 144 --seconds 20 --out target\clips\fn_144.mp4`
7. **HDR** (`& $cli probe-hdr` montre l'état HDR et le blanc SDR de chaque écran) : activer le HDR de l'écran principal (Paramètres > Affichage > HDR), afficher une
   image très colorée, `& $cli capture-video --target primary --fps 60 --seconds 5 --out target\clips\hdr.mp4`,
   puis comparer la vidéo à l'écran : couleurs normales, délavées ou sombres ?
8. **240 i/s** (build release, `cargo build --release -p palaclips-cli` et
   `$cli = ".\target\release\palaclips-cli.exe"`) : Fortnite plein écran 1440p sur l'écran
   240 Hz, en partie, noter le FPS du jeu. `Start-Sleep 10; & $cli probe-wgc --target window:FortniteClient-Win64-Shipping.exe --seconds 15 --min-interval-ms 2`
   (images reçues sans encodage, horloge DWM), puis
   `Start-Sleep 10; & $cli capture-video --target window:FortniteClient-Win64-Shipping.exe --fps 240 --seconds 30 --out target\clips\fn_240.mp4`
   et `ffprobe` du point 2. Attendu : `nb_frames=7200`, retard max < 4,17 ms. Avec G-Sync et
   des écrans à fréquences différentes, WGC plafonne à la fréquence de l'autre écran
   (`RESEARCH.md`) : refaire l'essai G-Sync désactivé.
8. **Bordure** : pendant `& $cli capture-video --fps 30 --seconds 10 --border --out target\clips\border.mp4`,
   une bordure jaune entoure l'écran ; sans `--border`, aucune bordure.
9. **Curseur** : avec `--no-cursor`, aucun curseur dans la vidéo ; sans l'option, curseur
   visible.
10. Facultatif (CAP-08) : mettre un écran à 150 % de mise à l'échelle et le capturer avec
    `--target "monitor:\\.\DISPLAYn"` : la vidéo est dans la définition native de l'écran.

## T2 — Synchro audio/vidéo sur 1 h (phase 2)

Source de référence : `palaclips-cli probe-av-emitter` (fenêtre plein écran sur l'écran
principal qui flashe en blanc au moment exact où un bip de 50 ms est envoyé à WASAPI, chaque
seconde, sans compensation de latence, comme un jeu). Mesure : `scripts/measure-av-sync.py`
(Python 3, appelle `ffmpeg`) apparie chaque flash de la vidéo au bip de la piste.

Préparation : sortie par défaut habituelle (Voicemeeter chez Palawi), le **micro doit entendre
les bips** (écouteur du casque collé au micro, ou enceintes), écran principal libre aux trois
instants de mesure. Depuis **PowerShell 7** à la racine du repo :

```powershell
pwsh scripts\t2-av-sync.ps1
```

Le script, sans autre intervention :
1. compile la CLI en release ;
2. **calibre** (35 s : `capture-av` + émetteur 20 s) et calcule `--mix-offset-ms` /
   `--mic-offset-ms` (AUD-11) qui ramènent les deux pistes à 0, puis attend Entrée ;
3. lance `capture-av --target primary --seconds 3640` avec ces décalages, et l'émetteur à
   0:10, 30:00 et 60:00 (20 s chacun) ;
4. à 28:00, bip de rappel : **couper tout son pendant 2 min** (jusqu'au flash de 30:00) ;
5. affiche `ffprobe` du fichier `target\clips\t2.mp4` et, pour chaque piste (mix, micro) et
   chaque instant (fenêtres de 60 s), le décalage moyen son − image avec `OK` si |moyenne| < 16,7 ms.

Rapporter toute la sortie du script. Attendu : 3 × `OK` sur le mix et 3 × `OK` sur le micro,
fichier de 3640 s, pistes de même durée. Variante courte pour vérifier l'installation :
`pwsh scripts\t2-av-sync.ps1 -Minutes 2`. Calibrage seul (35 s, décalages mesurés sans
capture longue) : `pwsh scripts\t2-av-sync.ps1 -CalibrateOnly`.


## Dérive A/V sans surveillance (`av-drift.ps1`)

Mesure la pente de la synchro (ms/h) par régression sur un bip par seconde pendant toute la
durée. Palawi lance la commande et part ; rien d'autre à faire, aucun réglage Windows changé :
l'émetteur et la capture visent la sortie donnée explicitement (`-Device`, partie du nom).

```powershell
pwsh scripts\av-drift.ps1 -Minutes 20 -Device Apogee
```

Le script empêche la mise en veille, compile la CLI, lance `capture-av --no-mic --mix-device`
(360p, 2 Mb/s) et `probe-av-emitter --device` sur l'écran principal, mesure avec
`measure-av-sync.py`, supprime la vidéo et écrit tout dans
`target\journals\av-drift-<date>.txt` (ligne `RÉSULTAT` : pente ± erreur-type, dispersion).
Le son des bips sort sur le périphérique choisi pendant toute la durée.
## Débranchement du périphérique audio (phase 2)

Sortie par défaut sur un périphérique USB débranchable (chez Palawi : « Haut-parleurs
(3- Apogee BOOM) »), musique en lecture, puis :

```powershell
$env:PATH = "$PWD\third_party\ffmpeg\bin;$env:PATH"
.\target\release\palaclips-cli.exe capture-av --target primary --seconds 60 --out target\clips\debranche.mp4
```

Vers 15 s débrancher l'USB, vers 35 s le rebrancher. Attendu : pas de plantage, lignes
`flux audio interrompu` puis `périphérique audio ouvert` vers le nouveau défaut, alertes
« la capture suit maintenant … », fichier de 60 s qui se décode sans erreur
(`ffmpeg -v error -i target\clips\debranche.mp4 -map 0 -f null -`), trois flux de même
durée.

## T3 — Isolation par logiciel (phase 3)
1. Navigateur : jouer une sinusoïde 440 Hz en boucle. Lecteur audio : 880 Hz en boucle.
2. `capture-apps --app chrome.exe --app <lecteur>.exe --seconds 30 --out t3.mp4`
3. Niveau de chaque fréquence, seconde par seconde, sur chaque piste :
   `python scripts\measure-tones.py t3.mp4 --hz 440 --hz 880` (ou le spectre :
   `ffmpeg -i t3.mp4 -map 0:a:2 -lavfi showspectrumpic=s=800x300 a2.png`).
4. Attendu : 440 Hz seul sur la piste du navigateur, 880 Hz seul sur l'autre, les deux sur la piste 1.
5. Rattachement : fermer puis rouvrir le lecteur pendant la capture → silence puis reprise.

Variante sans personne (sources de test qui ne s'entendent pas) : deux copies de la CLI,
`target\tone-a.exe` et `target\tone-b.exe`, jouent `play-tone --hz 440` / `--hz 880` sur
`--device Realtek` (sortie sans haut-parleurs), et `capture-apps --mix-device Realtek --app
tone-a.exe --app tone-b.exe` les enregistre ; `tone-b` est fermé puis relancé pendant la
capture.

## T4 — Replay (phase 4)

Source vidéo des tests : `ffplay` (fourni avec FFmpeg dans `third_party`) affiche du bruit
dans une fenêtre de 1280×720 en bas à droite de l'écran principal ; le replay capture cette
fenêtre seule, mise à l'échelle en 1440p 60 i/s à 40 Mb/s. Pistes : mix, `tone-a.exe`
(440 Hz) et `tone-b.exe` (880 Hz) sur la sortie Realtek (inaudible), Jeu (= ffplay, muet).
Tout est arrêté et les clips supprimés à la fin.

- Latence et durée (🤖, ~2 min 15, accord de Palawi pour la fenêtre) :
  `pwsh scripts\replay-save.ps1` → 20 sauvegardes de 60 s toutes les 3 s ; journal
  `target\journals\replay-save-*.txt` : latence min/moy/max (max < 500 ms), et pour chaque
  clip durée vidéo (60 s ± 1 s), écart entre flux (≤ 50 ms), 5 flux, début à 0.
  `-Disk` : même test avec le tampon sur disque.
- Mémoire (🤖, test accéléré ADR-020, ~31 min, en arrière-plan, accord de Palawi pour la
  fenêtre) : `pwsh scripts\replay-ram.ps1` → replay de 60 s armé 30 min, une sauvegarde
  toutes les 5 s (~360 clips, effacés au fil de l'eau, ~100 Go écrits en tout), mémoire
  privée du processus relevée à chaque sauvegarde, toutes les mesures dans le journal. Les
  10 premières minutes sont la mise en régime, exclues ; régression sur 10–30 min : pente
  **< 20 Mo/h** et **< 0,1 Mo par sauvegarde**, aucun clip manquant. Journal
  `target\journals\replay-ram-*.txt`.
- Mémoire hors tampon, PERF-02 (🤖, ~3 min, fenêtre ffplay en bas à droite et sons de test sur
  `Realtek`) : `pwsh scripts\engine-memory.ps1` → working set privé (critère, ADR-023),
  mémoire engagée et working set (information), médianes pour (a) moteur sans capture
  (`replay --no-arm`), (b) capture sans replay
  (`capture-apps`), (c) replay 5 s, (d) replay 60 s ; tampon réel = (d) − (c), comparé au
  tampon journalisé. Détail par étape (D3D11, sondage, NVENC, pistes) :
  `palaclips-cli probe-memory --target window:ffplay.exe --height 1440 --app tone-a.exe`.
- Qualité des réglages NVENC (👤 Palawi en partie de Fortnite, ~3 min dont 30 s de jeu) :
  `pwsh scripts\nvenc-quality.ps1` → capture 30 s de la fenêtre du jeu à 150 Mb/s (référence,
  retard max d'encodage dans le journal), réencodage à 40 Mb/s avec les défauts de FFmpeg et
  avec ADR-022, VMAF / PSNR / SSIM contre la référence ; vidéos supprimées à la fin.
- Resolve (👤) : importer un replay produit par `palaclips-cli replay`, vérifier que chaque
  piste est séparée, nommée et alignée sur la vidéo.

## T5 — Application (phase 5)
- Tout essai de l'app : `pwsh scriptspp-isolated.ps1 [-Settings '{"engine":{...}}']` lance
  la build release avec `PALACLIPS_CONFIG_DIR=target	est-config` (réglages et clips du test
  dans ce dossier). Le refermer et supprimer ce dossier à la fin.
- Fortnite en plein écran, appuyer sur le raccourci de sauvegarde : le jeu ne perd pas le focus
  (pas de minimisation, pas de clignotement), toast visible.
- Fenêtre fermée : Gestionnaire des tâches → un seul `palaclips.exe`, aucun `msedgewebview2.exe`
  rattaché.

- Calibrage (AUD-11) : `palaclips-cli calibrate --settings %APPDATA%\com.palawi.palaclips\settings.json`
  (lecture seule) ou page Audio > Calibrer… ; écran principal libre, pas de jeu lancé ;
  attendu : Mix et Jeu mesurés (≥ 5 paires, dispersion ≤ 20 ms), Jeu = référence → 0 ms,
  Mix proposé = −(mesure Mix − mesure Jeu) (ADR-027), micro mesuré si un écouteur est collé
  contre lui, sinon « non mesuré » avec la consigne ; total < 30 s, replay réarmé s'il
  l'était.

## T6 — Enregistrement et galerie (phase 6)

Installateur sans l'installer : `npx tauri build` (dans `app`), puis extraire
`target\release\bundle\nsis\PalaClips_*-setup.exe` avec 7-Zip dans `target\`, lancer
`palaclips-app.exe` extrait avec `PALACLIPS_CONFIG_DIR` et un PATH sans FFmpeg : l'app arme le
replay et charge `avcodec`, `avformat`, `avutil` depuis son dossier. Toujours depuis une branche à
jour de `dev` (isolation du WebView, #56).

Migration des données (ADR-032) : `pwsh scripts/migration-check.ps1` copie les dossiers
`%APPDATA%\PalaClips` et `%LOCALAPPDATA%\PalaClips` réels dans `target\`, migre la copie et
vérifie chaque fichier en SHA-256 au nouvel emplacement ; les dossiers réels ne bougent pas.

Isolation : `pwsh scripts/webview-isolation.ps1` (~20 s) écrit dans le localStorage d'une app
isolée et vérifie que le stockage WebView2 réel (`%LOCALAPPDATA%\com.palawi.palaclips\EBWebView`)
reste identique, octet pour octet et date pour date.

Automatisés (🤖, après `npx tauri build --no-bundle` dans `app`) : l'app isolée est pilotée par
le protocole de débogage de WebView2 (`scripts/cdp.mjs`, port local, aucune touche simulée),
fenêtre sur l'écran principal sans prise de focus (`app-isolated.ps1 -MainScreen`, ou
`-LeftScreen` quand Palawi joue). Caches (miniatures, pistes extraites) dans le dossier isolé.

- `pwsh scripts/recording-crash.ps1` (~2 min) : capture d'une fenêtre ffplay (testsrc, aucun
  son produit), découpage 1 min. Démarrer, 75 s, Arrêter → 2 fichiers, ~75 s au total. Puis
  Démarrer, 12 s, `taskkill /F /T` : le fichier se lit (≥ 10 s) avant tout redémarrage ;
  relance → journal `recordings.txt` vidé, fichier finalisé, clip `recording` dans la
  galerie. Tout est supprimé à la fin. Journal : `target/journals/recording-crash-*.txt`.
- `pwsh scripts/gallery-bench.ps1` (~10 min, 12 Go libres exigés) : génère un enregistrement
  factice d'1 h à 6 pistes (testsrc2 720p30 NVENC CBR 9 Mb/s + 6 sinusoïdes AAC, MP4
  hybride, ~4,4 Go) et 5 000 clips factices de 2 s (copies, dates étalées), le tout sous
  `target\`. Mesures : indexation, `list_clips`, ouverture de la galerie (6 fois, < 500 ms),
  défilement par `requestAnimationFrame` à 24 et 120 px par image (≥ 60 i/s ou la cadence de
  l'écran), ouverture du lecteur, 12 déplacements à l'arrêt et 12 en lecture (image affichée
  et 6 pistes recalées, < 300 ms), RAM privée (app + WebView2) après 20 s de lecture à 0, 15,
  30, 45 et 59 min (croissance ≤ 50 Mo). Pistes lues en sourdine. Tout est supprimé à la fin,
  et le script vérifie que `%LOCALAPPDATA%\com.palawi.palaclips\cache` n'a pas bougé. Journal :
  `target/journals/gallery-bench-*.txt`.

Séance avec Palawi (👤, ~10 min, une seule fois) :

1. `pwsh scripts/app-isolated.ps1` (PalaClips fermé avant) ; en jeu, `Ctrl+Alt+F9`, jouer
   2 à 3 min, `Ctrl+Alt+F9` ; point rouge dans la zone de notification pendant
   l'enregistrement. Dans la galerie : le clip est là, se lit, pistes séparées, déplacements.
2. Discord : sur ce clip, `Ctrl+C` dans la galerie, coller dans un salon privé : envoi OK,
   le son entendu est le mix. Quitter PalaClips (zone de notification > Quitter).
3. Disque plein (STO-05), dans un terminal **administrateur** : `pwsh -File
   scripts\disk-full.ps1` (~3 min) : VHDX de 1 Go dans `target\`, monté sur une lettre
   libre, enregistrement à 80 Mb/s d'une fenêtre de bruit ffplay (aucun son) jusqu'à l'arrêt
   sous 300 Mo libres ; attendu : alertes `disk_almost_full` et `recording_failed`, sauvegarde
   du replay refusée, fichier lisible (ffprobe), journal des enregistrements vide. Le VHDX est
   démonté et supprimé à la fin, même en cas d'échec ; jamais d'écriture sur C: au-delà du
   VHDX (6 Go libres vérifiés avant). Journal : `target/journals/disk-full-*.txt`. Avant :
   tests unitaires par injection d'erreur
   (`a_full_disk_stops_the_recording_and_the_file_is_recovered_at_startup`).

## T7 — Robustesse (phase 7)

| Scénario | Comment le provoquer | Attendu |
|---|---|---|
| Changement de résolution | Paramètres d'affichage pendant la capture | capture continue, alerte si redémarrage du buffer |
| Plein écran exclusif | jeu ancien (ou MSI Kombustor) en plein écran exclusif, « Hardware: Legacy Flip » ; `palaclips-cli probe-fullscreen` pendant la capture | `capture_exclusive_fullscreen` après 5 s de vidéo figée |
| Casque débranché | débrancher le périphérique par défaut | `audio_device_changed`, piste continue |
| Appli capturée qui crashe | `pwsh scripts/app-crash.ps1` (~45 s, sans fenêtre : processus de test tué puis relancé pendant un replay) | silence puis rattachement, replay jamais arrêté |
| GPU saturé | `palaclips-cli probe-gpu-priority --window` puis `capture-video` sous `gpu-load --window` (fenêtre 640×360 au premier plan) | retard borné, `capture_gpu_saturated` seulement sans la priorité temps réel |
| Disque plein | dossier de clips sur une petite clé USB / VHD de 1 Go | `disk_full`, arrêt propre, aucun fichier corrompu |
| Redémarrage du pilote GPU | **D'abord fermer le navigateur, Discord et tout jeu** (un TDR fait planter tout ce qui utilise le GPU), puis `dxcap -forcetdr` dans un terminal administrateur (Outils graphiques Windows) | `encoder_reset`, reprise automatique (ADR-031), enregistrement repris dans un nouveau fichier |
| Veille / sortie de veille | mettre le PC en veille avec replay armé | reprise propre au réveil |
| Cadence limitée (G-Sync) | Fortnite à 225 i/s, G-Sync actif, autre écran à 165 Hz, capture à 240 i/s ; puis G-Sync coupé et jeu plafonné à 120 i/s | `capture_source_rate_limited` (165 Hz) dans le premier cas, aucune alerte dans le second |

## PERF — mesures (phase 7)

Build installée (accord de Palawi avant d'installer ; PalaClips de dev fermé, même identifiant) :

1. `target\release\bundle\nsis\PalaClips_*-setup.exe /S` : installation silencieuse par
   utilisateur dans `%LOCALAPPDATA%\PalaClips\`.
2. `pwsh scripts\perf-installed.ps1` (~5 min) : exe installé lancé avec des réglages isolés
   (`target\perf-config`), dans la zone de notification (`--autostart`) puis fenêtre ouverte sans
   focus sur l'écran principal. PERF-01 : working set privé et CPU, replay désarmé, sans fenêtre.
   PERF-04 : de la relance (instance unique) au premier rendu avec l'état, une fois à froid puis
   trois fois à chaud. PERF-02 : replay armé (écran principal 1440p60, pistes par défaut, 30 s),
   fenêtre fermée, working set privé − tampon × 1,07. PERF-03 (part CPU) : CPU replay armé.
   Journal `target\journals\perf-installed-*.txt`. `-Exe` pour viser une autre build (répétition
   sur `target\release`).
Avant l'étape 1 et après l'étape 3 : `pwsh scripts\real-data-fingerprint.ps1 -Out <fichier>`
   (puis `-Compare <fichier d'avant>`) relève en SHA-256 les vraies données de Palawi
   (`%APPDATA%\PalaClips`, `cache` et `replay` de `%LOCALAPPDATA%\PalaClips`, dossiers
   `com.palawi.palaclips`) : ni l'installation, ni la mesure isolée, ni la désinstallation ne
   doivent y toucher.

3. Désinstallation sans la case : `%LOCALAPPDATA%\PalaClips\uninstall.exe /S` ; attendu :
   programme et raccourcis retirés, `%APPDATA%\com.palawi.palaclips`,
   `%LOCALAPPDATA%\com.palawi.palaclips` et le dossier des clips inchangés.
4. Désinstallation avec la case (👤, accord de Palawi) : réinstaller (étape 1), PalaClips fermé,
   `pwsh scripts\uninstall-check.ps1 -Step Backup` (copie complète des deux dossiers de
   l'identifiant + empreinte SHA-256 du dossier des clips), Palawi désinstalle depuis Paramètres
   > Applications en cochant « Supprimer aussi les réglages… », puis
   `pwsh scripts\uninstall-check.ps1 -Step Verify` : dossiers de l'identifiant supprimés, clips
   intacts, données restaurées et comparées en SHA-256 (sauvegarde gardée en cas d'écart).
- Build release **installée** (pas `tauri dev`).
- RAM (PERF-01, PERF-02, ADR-023) : mémoire privée réellement occupée (*private working
  set*), colonne « Mémoire (jeu de travail privé) » du Gestionnaire des tâches ou
  `Win32_PerfFormattedData_PerfProc_Process.WorkingSetPrivate` ; machine hors pression
  mémoire (pas d'autre gros consommateur, RAM libre > 4 Go). Relever aussi la mémoire engagée
  (`PrivateUsage`) pour information dans le rapport.
- CPU : moyenne sur 5 min (Analyseur de performances ou `typeperf`).
- Charge de l'app (PERF-03, 🤖, ~2 min 30, fenêtre sur un écran secondaire, focus rendu à la
  fenêtre de Palawi) : après `npx tauri build --no-bundle` (dans `app`), `pwsh
  scripts\app-load.ps1` → CPU (app + WebView2) et GPU par moteur, fenêtre ouverte sans focus
  puis fenêtre fermée, replay armé ; journal `target\journals\app-load-*.txt`.
- Impact en jeu (PERF-03) : Fortnite en Créatif, scène fixe, **GPU-bound** (limite d'images
  levée ou scène lourde, GPU ~99 %), puis dans un PowerShell admin
  `pwsh -File scripts\fps-impact.ps1` : PresentMon 2.4.1 (CapFrameX), un essai de mise en
  route non compté, puis sans capture / fenêtre 60 / écran 60 / écran 1440p60 + 4 pistes (mix, micro,
  Jeu, `-App`, Discord par défaut : le critère PERF-03 de la SPEC) / fenêtre 30 / WGC seul / sans
  capture, 20 s chacun ; référence = moyenne des essais sans capture ; perte par essai, 1 %
  bas, GPU (`nvidia-smi`) et mode de présentation dans `target\journals\fps-impact-*.txt`.

## T8 — Mise à jour 1.0.0 → 1.0.1 (phase 8)

Automatisé, sans Palawi : `pwsh scripts/update-test.ps1` (environ 5 min ; il faut le brouillon
`test-v1.0.1`, créé en poussant ce tag sur le commit à tester). Jamais de release publique.

1. Garde-fous : PalaClips fermé, aucune installation dans `%LOCALAPPDATA%\PalaClips` ;
   empreinte (taille et date) de chaque fichier de données de Palawi, en lecture seule.
2. Télécharge l'installeur 1.0.1 signé et sa signature depuis le brouillon, écrit
   `latest.json` (bonne signature) et `latest-bad.json` (version 1.0.2, signature altérée), sert le
   dossier sur `127.0.0.1:8765`.
3. Compile l'app en version 1.0.0 avec `dangerousInsecureTransportProtocol` (fichier passé à
   `--config`, seulement pour cette build : la build publiée refuse le `http`), la lance isolée avec
   `PALACLIPS_UPDATE_URL` et le port CDP.
4. Fausse signature : Réglages → Général → Rechercher → « 1.0.2 disponible » → Installer →
   erreur `update_failed`, l'app reste en 1.0.0.
5. Bonne signature : Rechercher → « 1.0.1 disponible », notes → Installer → l'installeur NSIS
   (passif) installe dans `%LOCALAPPDATA%\PalaClips` ; en mode isolé, l'updater ne demande pas
   de relance (`restart_after_install(false)` : la relance de l'installeur passe par
   l'Explorateur et perdrait l'environnement isolé). Le script lance l'exe installé en mode
   isolé : CDP lit « Version 1.0.1 », le journal du dossier de test montre `version="1.0.1"`.
6. Ménage : app arrêtée, désinstallation silencieuse (`uninstall.exe /S`, données gardées),
   dossier, raccourci et clé de désinstallation vérifiés absents, serveur arrêté, brouillon et
   tag supprimés, empreintes des données de Palawi comparées (aucune différence attendue).

## T10 — Installateur dans une VM Windows 11 sans GPU (phase 8)

Ce qu'un PC neuf révèle et que la machine de dev cache : DLL manquantes, WebView2, droits,
SmartScreen, désinstallation. La VM n'a pas d'encodeur matériel : PalaClips doit le dire
clairement, sans planter. Installeur : celui du brouillon de release (`PalaClips_x.y.z_x64-setup.exe`),
téléchargé **dans la VM** par le navigateur, pour avoir la vraie marque « venu d'Internet ».

| # | À faire dans la VM | Attendu |
|---|---|---|
| 1 | Lancer l'installeur téléchargé | SmartScreen « Windows a protégé votre ordinateur » → *Informations complémentaires* → *Exécuter quand même* ; aucune demande de droits admin ; page de licence ; installation dans `%LOCALAPPDATA%\PalaClips` |
| 2 | Cocher « Lancer PalaClips » à la fin | La fenêtre s'ouvre (pas d'erreur de DLL manquante, pas de fenêtre blanche) sur le premier lancement |
| 3 | Lire l'étape Vérifications | Windows : build affiché, OK ; Encodeur vidéo : avertissement « Aucun encodeur matériel n'a fonctionné… » et « Exporter un diagnostic » ; WebView2 : version ; Dossier des clips : OK ; « Continuer » grisé |
| 4 | Exporter un diagnostic, puis « Ouvrir le zip » | Zip créé et ouvert ; `system.txt` liste l'adaptateur de la VM et les encodeurs refusés |
| 5 | « Passer la configuration » | Accueil : alerte « aucun encodeur matériel… » lisible, pas de plantage ; icône dans la zone de notification ; Réglages → Général → Mises à jour affiche la version |
| 6 | Fermer la fenêtre puis zone de notification → Quitter | Plus aucun processus `palaclips-app` dans le Gestionnaire des tâches |
| 7 | Paramètres → Applications → PalaClips → Désinstaller, case « supprimer aussi les réglages » **décochée** | `%LOCALAPPDATA%\PalaClips` et le raccourci du menu Démarrer disparaissent ; `%APPDATA%\com.palawi.palaclips` reste ; `Vidéos\PalaClips` reste |
| 8 | Réinstaller, puis désinstaller avec la case **cochée** | `%APPDATA%\com.palawi.palaclips` et `%LOCALAPPDATA%\com.palawi.palaclips` supprimés ; `Vidéos\PalaClips` reste |

À me rapporter : pour chaque ligne, OK ou ce qui diffère (texte exact des messages) ; une
capture de l'étape Vérifications et de l'accueil avec l'alerte ; le zip de diagnostic de
l'étape 4 ; la version de Windows de la VM (`winver`).

## T11 — Encodage par le processeur dans une VM sans GPU (v1.1)

La VM n'a pas d'encodeur matériel ni, souvent, de Video Processor D3D11 : PalaClips doit
proposer le processeur au premier lancement, capturer et sauvegarder un clip. Installeur : celui
du brouillon de test (`test-v1.1.0`), copié dans la VM.

| # | À faire dans la VM | Attendu |
|---|---|---|
| 1 | Installer, lancer | Premier lancement, étape Vérifications |
| 2 | Lire « Encodeur vidéo » | Ambre : « Aucun encodeur sur la carte graphique : le processeur encodera la vidéo (H.264 · AV1)… » ; « Continuer quand même » actif |
| 3 | Continuer, répondre aux questions, « C'est bon » | Test de sauvegarde : le replay s'arme, « En mémoire » monte |
| 4 | Bouger une fenêtre quelques secondes, puis « Sauvegarder maintenant » | Clip dans la galerie avec miniature ; il se lit dans le lecteur intégré, image nette, son présent |
| 5 | Accueil → panneau de performances ouvert 30 s | Encodeur « Processeur · H.264 » ; CPU de la machine et images en retard affichés ; pas d'alerte « le processeur n'arrive pas à encoder » à 1080p30 (sinon noter le nombre de cœurs de la VM) |
| 6 | Réglages → Replay & qualité | Encodeur « Automatique », aide en ambre ; « Carte graphique » grisé ; 30 i/s, 1080p (ou natif si l'écran de la VM est plus petit) ; Codec : HEVC grisé |
| 7 | Choisir le codec AV1, Appliquer, sauvegarder un clip | Clip AV1 lisible dans le lecteur intégré |
| 8 | Ouvrir le clip H.264 de l'étape 4 dans le lecteur Films et TV de la VM | Se lit |

À me rapporter : OK ou ce qui diffère pour chaque ligne (texte exact) ; une capture des étapes 2,
5 et 6 ; le nombre de cœurs de la VM (`Gestionnaire des tâches → Performance`) et sa version de
Windows (`winver`) ; le zip de diagnostic (Réglages → Général) **après** l'étape 7, qui dit si la
conversion d'image passe par le Video Processor ou par le processeur.

## T12 — Enregistrer après la sauvegarde (v1.2)

Automatique (`pwsh scripts/post-roll.ps1`, ~4 min, écran principal, flashs et bips la dernière
minute ; `-SkipSync` sans flashs, `-SyncOnly` pour la synchro seule) : clip simple, prolongé,
terminé avant la fin, enregistrement lancé pendant le « après », sources changées pendant le
« après », tampon sur disque, arrêt brutal puis récupération, synchro de part et d'autre de la
jonction contre un clip instantané. Journal dans `target\journals\post-roll-*.txt`.

Mémoire sans surveillance (`pwsh scripts/post-roll-memory.ps1`, ~12 min, lancé par Palawi) :
avant 60 s, après 600 s, 360p30 ; working set privé toutes les 10 s ; critère : pente < 20 Mo/h,
écart < 30 Mo, clip continu.

À la main (session de fin de version) : Réglages → Replay & qualité → Après l'appui = 10 s ; en
jeu, `Alt+F10` → carillon et notification « Clip en cours / +10 s » sans perte de focus ; second
appui pendant le « après » → toujours un seul clip, plus long ; « Terminer le clip maintenant »
dans la zone de notification ; à la fin, notification « Clip sauvegardé » et clip dans la
galerie.
