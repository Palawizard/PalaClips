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
- Fortnite en plein écran, appuyer sur le raccourci de sauvegarde : le jeu ne perd pas le focus
  (pas de minimisation, pas de clignotement), toast visible.
- Fenêtre fermée : Gestionnaire des tâches → un seul `palaclips.exe`, aucun `msedgewebview2.exe`
  rattaché.

## T6 — Enregistrement et galerie (phase 6)
- Crash : pendant un enregistrement, `taskkill /F /IM palaclips.exe` → relancer → le clip est
  dans la galerie et se lit jusqu'à la fin.
- Gros fichier : enregistrement de 4 Go, déplacements rapides dans le lecteur.
- Discord : copier un clip depuis la galerie, coller dans Discord, vérifier le son.

## T7 — Robustesse (phase 7)

| Scénario | Comment le provoquer | Attendu |
|---|---|---|
| Changement de résolution | Paramètres d'affichage pendant la capture | capture continue, alerte si redémarrage du buffer |
| Plein écran exclusif | jeu ancien en plein écran exclusif | repli écran + alerte `capture_fallback_monitor` |
| Casque débranché | débrancher le périphérique par défaut | `audio_device_changed`, piste continue |
| Appli capturée qui crashe | tuer Discord | silence puis rattachement |
| Disque plein | dossier de clips sur une petite clé USB / VHD de 1 Go | `disk_full`, arrêt propre, aucun fichier corrompu |
| Redémarrage du pilote GPU | `dxcap -forcetdr` (Outils graphiques Windows) ou Win+Ctrl+Maj+B | `encoder_reset`, reprise automatique |
| Veille / sortie de veille | mettre le PC en veille avec replay armé | reprise propre au réveil |
| Cadence limitée (G-Sync) | Fortnite à 225 i/s, G-Sync actif, autre écran à 165 Hz, capture à 240 i/s ; puis G-Sync coupé et jeu plafonné à 120 i/s | `capture_source_rate_limited` (165 Hz) dans le premier cas, aucune alerte dans le second |

## PERF — mesures (phase 7)
- Build release **installée** (pas `tauri dev`).
- RAM (PERF-01, PERF-02, ADR-023) : mémoire privée réellement occupée (*private working
  set*), colonne « Mémoire (jeu de travail privé) » du Gestionnaire des tâches ou
  `Win32_PerfFormattedData_PerfProc_Process.WorkingSetPrivate` ; machine hors pression
  mémoire (pas d'autre gros consommateur, RAM libre > 4 Go). Relever aussi la mémoire engagée
  (`PrivateUsage`) pour information dans le rapport.
- CPU : moyenne sur 5 min (Analyseur de performances ou `typeperf`).
- Impact en jeu : fps moyen Fortnite (mode créatif, même scène) avec et sans PalaClips, 3 essais.
