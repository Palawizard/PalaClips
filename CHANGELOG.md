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
- Écran Réglages : sections Raccourcis (saisie en appuyant sur la combinaison, conflits
  signalés) et Général (démarrage avec Windows, armement au démarrage, son et notification de
  confirmation, thème Système / Sombre / Clair, réinitialisation) ; les autres sections
  suivent.
- Raccourcis personnalisables (sauvegarder, armer/désarmer le replay) : refus des
  combinaisons de Windows et des touches qui gêneraient en jeu, retour aux raccourcis
  précédents si une autre appli a déjà pris la combinaison.
- Réglages > Replay & qualité : durée, tampon en RAM ou sur disque, codec (ceux que la carte
  graphique ne sait pas encoder sont grisés), qualité et débit libre, i/s et résolution de
  sortie avec valeur libre, intervalle d'images clés. Taille du tampon estimée en direct,
  alerte au-delà de 40 % de la RAM disponible avec passage sur disque en un clic ; un
  changement qui redémarre le replay armé demande confirmation.
- La résolution de sortie ne dépasse jamais celle de la source. Un changement vidéo est
  vérifié avant d'être appliqué (limite du codec, puis essai réel de l'encodeur) avec un
  message clair en cas de refus, par exemple 5120×1440 en H.264. « Réglages appliqués »
  s'affiche quand un changement en attente part au désarmement du replay.
- `PALACLIPS_CONFIG_DIR` : dossier de réglages séparé, pour les essais.
- Assistant de calibrage de la synchro (Réglages > Audio) : flash + bip sur l'écran principal
  pendant 10 s, mesure de chaque piste dans PalaClips même (sans Python ni FFmpeg externe),
  décalage proposé par piste à appliquer d'un clic. La référence est la capture par processus
  de l'émetteur : seul l'excédent de chaque piste est compensé (Jeu et applis 0 ms). Au
  casque, l'assistant demande de coller un écouteur contre le micro. CLI
  `palaclips-cli calibrate`.
- Réglages > Audio : pistes dans l'ordre du fichier avec vumètre, nom modifiable (Mix, Micro
  et Jeu compris), décalage de synchro par piste, sortie et micro au choix, gain du micro,
  piste Jeu automatique, nombre maximal de pistes d'applis ; ajout d'une appli parmi celles
  qui font du son (avec vumètre), les autres applis ouvertes ou un .exe à parcourir.
- CLI `palaclips-cli audio-sources` : périphériques et applis qui ont une session audio.
- Réglages > Capture : écrans et fenêtres en vignettes avec aperçu en direct (~1 image/s,
  seulement pendant que la section est visible), icône et nom de l'appli ; curseur et bordure
  jaune réglables ; changer de cible pendant que le replay tourne demande confirmation.
- CLI `palaclips-cli preview` : aperçus JPEG et icône d'une cible.
- Réglages > Stockage : dossier des clips choisi avec le sélecteur de Windows et ouvert dans
  l'Explorateur, espace libre affiché, noms des jeux modifiables. Un nom qui ne peut pas
  servir de nom de dossier (caractères interdits, noms réservés comme CON ou NUL) est refusé
  avec un message clair.
- CLI `palaclips-cli try-output` : essaie l'encodeur matériel à une taille et un fps donnés.
- Démarrage avec Windows (option) : PalaClips s'ouvre directement dans la zone de
  notification, sans fenêtre.
- Un nouveau clip commence où le précédent s'est arrêté : plus de minute répétée d'un clip à
  l'autre ; le temps en mémoire repart de zéro après chaque sauvegarde. Un double appui sur
  le raccourci ne crée plus de clip d'une seconde.
- Sauvegarde confirmée par un petit carillon joué par PalaClips, même en jeu quand Windows
  bloque les notifications, et par une notification ; les deux sont désactivables.
- Les clips sont rangés sous le vrai nom du jeu (« Fortnite » au lieu de
  « FortniteClient-Win64-Shipping »), lu dans l'exe ou tiré de son nom ; le nom est gardé une
  fois choisi.
- CLI : `game-name` (nom de jeu choisi pour un exe en cours).
- Les réglages sont enregistrés (`%APPDATA%\PalaClips\settings.json`) et retrouvés au
  démarrage ; un fichier abîmé est mis de côté et remplacé par les réglages par défaut, avec
  une alerte ; retour aux réglages par défaut possible.
- CLI : `probe-memory`, `recent-clips`, `replay --levels`, `replay --no-arm` ; scripts
  `engine-memory.ps1`, `nvenc-quality.ps1`.
- Enregistrement manuel (écran État, raccourci `Ctrl+Alt+F9`, zone de notification avec un
  point rouge pendant l'enregistrement) sur la même capture que le replay, sans coût en plus ;
  fichier lisible jusqu'à la dernière seconde même après un arrêt brutal ; option pour
  commencer par le replay en mémoire et découpage par durée ou par taille (Réglages >
  Enregistrement). Un enregistrement interrompu (plantage, coupure) est réparé au démarrage
  suivant et apparaît comme un clip normal.
- Bibliothèque des clips : index rapide du dossier des clips, mis à jour tout seul quand un clip
  est ajouté, renommé ou supprimé hors de l'app ; favoris ; renommer, mettre à la Corbeille,
  montrer dans l'Explorateur.
- Quota de disque (Réglages > Stockage) : au-delà, les plus anciens clips non favoris sont
  supprimés définitivement ; PalaClips dit combien et quelle taille avant d'appliquer.
- Disque plein : alerte sous 2 Go libres pendant un enregistrement, arrêt propre sous 300 Mo
  (le fichier reste lisible) ; une sauvegarde du replay qui ne tiendrait pas est refusée avec
  un message clair.
- Interface en français et en anglais (Réglages > Général > Langue), langue de Windows par
  défaut : écrans, alertes, erreurs, menu de la zone de notification et notifications. Les
  pistes non renommées (Mix, Micro, Jeu) prennent le nom de la langue choisie, à l'écran
  comme dans les clips ; renommer une piste ne redémarre plus le replay.
- Galerie : grille de clips fluide même avec 5 000 clips, filtres (jeu, type, favoris,
  période), tri, recherche, menu d'actions au clic droit et au clavier. Lecteur intégré avec
  pistes audio séparées (muet, solo, volume), déplacements rapides même sur un clip de 4 Go,
  mémoire stable sur un enregistrement d'une heure ; découpe rapide ou précise et export des
  pistes choisies ou mixées. Un clic sélectionne un clip (les raccourcis agissent dessus), un
  double-clic ou Entrée l'ouvre.
- La piste 1 (Mix complet) est la somme des pistes que tu enregistres : ton micro, le jeu et
  les applis choisies, comme la piste du stream sur OBS. Un clip collé dans Discord contient ta
  voix, sans les notifications ni la musique du bureau. Les autres pistes restent séparées pour
  le montage. Réglages > Audio : « Aussi dans Mix complet » (micro) et « Audio du bureau »
  (toute la sortie à la place des pistes Jeu et applis).
- Écran État : section « Performances » repliable, CPU de PalaClips en % de la machine, RAM
  hors tampon, tampon, encodeur, images répétées et en retard ; rien n'est mesuré quand elle
  est fermée.
- Installateur Windows par utilisateur : FFmpeg et ses licences (LGPL v3, GPL v3) inclus, page
  de licence ; à la désinstallation, une case dit ce qui part (réglages, historique et favoris
  de la galerie), jamais les clips.
- Réglages > Replay & qualité : « Quand le GPU est à fond » — Priorité au jeu (défaut, le jeu
  garde ses fps) ou Priorité à la capture (vidéo fluide, environ 7 % de fps en moins).
- Alertes « GPU à fond » (limite les fps du jeu ou choisis la priorité à la capture) et « plein
  écran exclusif » (passe le jeu en plein écran fenêtré ou sans bordure).
- CLI : `probe-gpu-priority`, `probe-fullscreen` ; scénario `scripts/app-crash.ps1`.
- Journaux de l'app : un fichier par jour, 7 jours gardés, paniques comprises.
- Réglages > Général : « Exporter un diagnostic », un zip avec les journaux, les réglages et
  les infos système (Windows, processeur, carte graphique et pilote, écrans, encodeurs,
  périphériques audio, fuseau horaire), sans le chemin du profil, le nom du PC, le nom du
  compte ni le titre de la fenêtre capturée ; « Ouvrir le zip » pour voir ce qui part. CLI
  `diagnostic --out`.
- Premier lancement guidé (seulement à la toute première ouverture) : vérifications (Windows,
  encodeur, WebView2, dossier des clips), écran à capturer en miniatures vivantes, micro et
  contenu de la piste 1, dossier des clips, durée et emplacement du replay (mémoire ou disque),
  raccourci de sauvegarde, puis un vrai clip de test ; « Passer la configuration » garde les
  réglages par défaut.
- README utilisateur en anglais (`README.md`) et en français (`README.fr.md`) : installation,
  SmartScreen, raccourcis, FAQ (plein écran exclusif, Minecraft Java, anti-triches, GPU à fond,
  pistes dans Resolve, diagnostic), captures faites dans l'app isolée par
  `scripts/readme-screenshots.ps1`.

### Corrigé
- Une appli lancée en plusieurs instances indépendantes (ex. deux Vesktop) : sa piste était
  muette quand le son passait par l'autre instance. Toutes les instances sont maintenant
  capturées et additionnées sur la piste, y compris celles lancées après le démarrage.
- Jeu qui prend tout le GPU : la capture ne prend plus des secondes de retard ; les images
  manquées répètent la dernière, le son reste aligné.
- Une alerte de capture (G-Sync, GPU, plein écran exclusif) disparaît quand sa cause disparaît
  ou que la capture redémarre.
- Activer ou couper le HDR de l'écran pendant une capture : elle repart d'elle-même au bon
  format au lieu de garder des couleurs fausses jusqu'au prochain armement.
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
