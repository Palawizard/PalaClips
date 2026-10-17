# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

### Ajouté
- Création du repo : workspace Cargo (`palaclips-engine`, `palaclips-cli`, app Tauri 2),
  front React 19 + TypeScript + Tailwind 4, CI Windows.
- Moteur : horloge `Hns` (QPC), `EngineConfig` validée, contrat `EngineEvent`, file de replay
  `PacketRing` purgée par âge, estimation de RAM — avec tests.
- CLI `palaclips-cli ffmpeg-info` : version de FFmpeg et encodeurs vidéo matériels
  disponibles (NVENC, AMF, QSV, Media Foundation).
- Liaison FFmpeg 8.1 LGPL via `rsmpeg` 0.18 ; `cargo run`/`cargo test` trouvent les DLL seuls.
- `check-env.ps1` détecte les problèmes clang/Visual Studio et les skills de design.
- Contexte produit `PRODUCT.md` et direction visuelle « Écran d'après-match ».
- Capture vidéo (phase 1) : écrans et fenêtres par Windows Graphics Capture, fenêtre retrouvée
  par nom d'exe, conversion NV12 et mise à l'échelle sur le GPU, encodage matériel H.264 /
  HEVC / AV1 sans copie CPU, fps constant sur l'horloge QPC, curseur et bordure réglables,
  changement de résolution à chaud, repli fenêtre → écran.
- Encodeurs détectés par un encodage de test réel (NVENC, AMF, Quick Sync) avec la raison d'un
  échec ; message clair si aucun ; logs FFmpeg dans les journaux au lieu de la console.
- Préréglages de débit Qualité / Équilibré / Léger et débit libre.
- Écran HDR : la vidéo garde les couleurs de l'écran au lieu d'être délavée.
- CLI : `list-targets`, `resolve-target`, `probe-wgc`, `probe-hdr`, `capture-video`.
- Documentation : SPEC V1, architecture, plan par phases, handoff, ADR, recherche, protocoles
  de test, brief design.
- Audio (phase 2) : piste « Mix complet » (tout ce qui sort par défaut) et piste « Micro »
  (gain réglable) en AAC dans le MP4, synchronisées sur la même horloge que la vidéo ; les
  silences et les coupures sont comblés sans décaler la synchro ; la capture suit le
  périphérique par défaut quand on change de sortie ou qu'on débranche le casque.
- Décalage de synchro par piste, pour compenser les périphériques virtuels comme Voicemeeter.
- CLI : `capture-av`, `probe-audio`, `probe-av-emitter` ; script de test de synchro sur une
  heure (`scripts/t2-av-sync.ps1`).

- Une piste par logiciel (capture de l'appli et de ses processus enfants), retrouvée par nom
  d'exe et rattachée seule quand l'appli est fermée puis relancée ; piste « Jeu » ; alerte si une piste d'appli reste muette alors que l'appli émet du
  son.
- CLI : `list-audio-sessions`, `capture-apps`, `probe-audio --app`, `play-tone`.
- Replay instantané (phase 4) : les dernières secondes (5 s à 60 min) gardées en RAM ou sur
  disque ; une sauvegarde écrit le clip en quelques millisecondes sans arrêter la capture, à
  partir d'une image clé, toutes les pistes alignées et de même durée, rangé dans
  `Vidéos\PalaClips\<Jeu>\` sous le nom `<Jeu>_<date>_<heure>_replay.mp4`.
- Changer de zone de capture ou de sources redémarre le tampon de replay proprement.
- CLI : `replay` (Entrée = sauvegarder, affiche la mémoire et la latence).
- Application (phase 5, socle) : icône dans la zone de notification (Ouvrir, Sauvegarder le
  replay, Quitter), une seule instance, fermer la fenêtre ne coupe pas le replay ; `Alt+F10`
  sauvegarde le replay depuis n'importe quel jeu, avec une alerte qui explique quoi faire si
  le raccourci est déjà pris (overlay NVIDIA).
- Chaque clip sauvegardé a sa miniature ; la liste des derniers clips donne leur durée et le
  nom de leurs pistes lu dans le fichier ; espace disque restant.
- Niveaux audio de chaque piste en direct quand la fenêtre est ouverte (moins souvent sans
  focus, rien quand elle est réduite ou fermée).
- Écran État : le replay (armé ou non, secondes en mémoire, RAM utilisée, Sauvegarder,
  Armer/Désarmer), la source capturée, l'espace disque, les alertes, chaque piste audio dans
  l'ordre du fichier avec sa couleur, son vumètre, sa crête et son état (en attente de l'appli,
  muette…), et les derniers clips avec miniature. Thèmes sombre et clair, polices embarquées.
- Menu de la zone de notification : armer et désarmer le replay.
- Les clips sont rangés sous le vrai nom du jeu (« Fortnite » au lieu de
  « FortniteClient-Win64-Shipping »), lu dans l'exe ou tiré de son nom ; le nom est gardé une
  fois choisi.
- CLI : `game-name` (nom de jeu choisi pour un exe en cours).
- Les réglages sont enregistrés (`%APPDATA%\PalaClips\settings.json`) et retrouvés au
  démarrage ; un fichier abîmé est mis de côté et remplacé par les réglages par défaut, avec
  une alerte ; retour aux réglages par défaut possible.
- CLI : `probe-memory`, `recent-clips`, `replay --levels`, `replay --no-arm` ; scripts
  `engine-memory.ps1`, `nvenc-quality.ps1`.

### Corrigé
- Arrêter puis relancer la capture dans la même session ne fait plus planter PalaClips.
- Replay : ~110 Mo de mémoire en moins (encodeur NVIDIA réglé plus sobrement, qualité
  identique sur une partie de Fortnite).
- Estimation de la RAM du replay plus juste : elle compte les secondes de marge du tampon et
  sa surcharge réelle.
- Piste « Jeu » : elle suit le jeu (dernière appli en plein écran ou sans bordure, l'appli de
  la fenêtre capturée, ou le processus choisi) au lieu de passer sur Discord, le navigateur
  ou le terminal à chaque Alt+Tab.
- Écran SDR avec gestion automatique des couleurs : il n'est plus pris pour un écran HDR.
- Écran HDR : les hautes lumières (ciel, soleil, explosions) gardent leurs détails et leur
  couleur au lieu de virer au blanc uniforme.
- Capture à 240 i/s : plus de retard au démarrage (première image encodée avant le départ de
  l'horloge).
- Audio : une sortie sur laquelle rien ne joue (Windows n'envoie alors aucune donnée) produit
  quand même une piste continue de silence, au lieu d'attendre la fin de la capture.
