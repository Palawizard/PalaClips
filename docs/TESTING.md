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
8. **Bordure** : pendant `& $cli capture-video --fps 30 --seconds 10 --border --out target\clips\border.mp4`,
   une bordure jaune entoure l'écran ; sans `--border`, aucune bordure.
9. **Curseur** : avec `--no-cursor`, aucun curseur dans la vidéo ; sans l'option, curseur
   visible.
10. Facultatif (CAP-08) : mettre un écran à 150 % de mise à l'échelle et le capturer avec
    `--target "monitor:\\.\DISPLAYn"` : la vidéo est dans la définition native de l'écran.

## T2 — Synchro audio/vidéo sur 1 h (phase 2)
1. Lancer `capture-av --seconds 3700`.
2. À 0:00, 30:00 et 60:00 : taper dans les mains **devant une vidéo de chronomètre affichée à
   l'écran avec un son de « bip » à chaque seconde** (ou utiliser une page de test A/V sync).
3. Dans Resolve : mesurer l'écart image du bip / son du bip, sur la piste mix et la piste micro.
4. Attendu : écart < 1 image (< 16,7 ms à 60 fps) aux trois instants.
5. Variante silence : couper tout son 2 min au milieu, refaire la mesure à la fin.

## T3 — Isolation par logiciel (phase 3)
1. Navigateur : jouer une sinusoïde 440 Hz en boucle. Lecteur audio : 880 Hz en boucle.
2. `capture-apps --app chrome.exe --app <lecteur>.exe --seconds 30 --out t3.mp4`
3. Spectre de chaque piste (`ffmpeg -i t3.mp4 -map 0:a:2 -lavfi showspectrumpic=s=800x300 a2.png`).
4. Attendu : 440 Hz seul sur la piste du navigateur, 880 Hz seul sur l'autre, les deux sur la piste 1.
5. Rattachement : fermer puis rouvrir le lecteur pendant la capture → silence puis reprise.

## T4 — Replay (phase 4)
- Latence : 20 sauvegardes, noter min/moy/max (attendu max < 500 ms).
- Fuite : replay armé 2 h, relever la RAM à 5 min, 30 min, 1 h, 2 h (écart < 5 %).
- Resolve : importer un replay, vérifier pistes nommées et synchro.

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

## PERF — mesures (phase 7)
- Build release **installée** (pas `tauri dev`).
- RAM : Gestionnaire des tâches, colonne « Mémoire (jeu de travail privé) ».
- CPU : moyenne sur 5 min (Analyseur de performances ou `typeperf`).
- Impact en jeu : fps moyen Fortnite (mode créatif, même scène) avec et sans PalaClips, 3 essais.
