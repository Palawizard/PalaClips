# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

## [1.4.1] - 2026-10-28

### Corrigé
- État : on peut ouvrir un clip depuis « Derniers clips ». Double-clic ou Entrée sur une carte
  (cartes focusables, focus visible, nom lu par un lecteur d'écran) ouvre le clip dans le lecteur
  de la Galerie ; le fermer ramène à État. Si le fichier a disparu entre-temps, un message
  s'affiche sur État et la carte est retirée (GAL-03).

## [1.4.0] - 2026-10-28

### Ajouté
- Lecteur : réglage « Couper le Mix complet à la lecture » (Général, activé par défaut). À
  l'ouverture d'un clip qui a une piste Jeu ou une piste d'appli, la piste 1 est coupée pour ne
  rien entendre en double ; elle se réactive avec M. L'état muet/solo/volume est retenu par
  clip. L'export et la compression gardent la piste 1 tant qu'on ne l'a pas coupée soi-même
  (GAL-04).
- Son « clip terminé » à la fin d'un clip avec « après » (La → Mi → La à l'octave), distinct du
  carillon de l'appui et des sons d'enregistrement, soumis à « Son de confirmation » et exclu des
  clips en « Tous les sons sauf PalaClips » (RPL-10). La fin d'un enregistrement ne joue plus le
  carillon de sauvegarde par-dessus son propre son.
- Réglages → Replay & qualité en trois onglets (Replay, Enregistrement, Commun), chacun avec son
  estimation de taille ; l'onglet est retenu pendant la session, un point marque l'onglet qui a
  un changement non appliqué, et le résumé de l'enregistrement sur État ouvre l'onglet
  Enregistrement (APP-15).

### Modifié
- État : en fenêtre large, « Derniers clips » et la colonne des pistes et performances défilent
  chacun de leur côté sous le bandeau ; sous 1000 px, la page défile comme avant. 12 derniers
  clips au lieu de 5.
- État : la cellule Source affiche la vraie source (« 2560×1440 · 240 Hz » pour un écran, la
  taille de la fenêtre capturée pour une fenêtre), mise à jour en direct ; la qualité enregistrée
  reste dans le résumé de la cellule Replay.
- Lecteur : pendant le glisser sur la barre de temps ou une piste, le curseur, le temps et la
  zone jouée suivent la souris à chaque image ; l'image est chargée quand la souris s'arrête
  (120 ms) et au relâchement, une demande à la fois. Sur un clip 1440p120 HEVC de 3 min :
  2 chargements au lieu de 122, curseur redessiné toutes les 17 ms au lieu d'être figé (GAL-03).

## [1.3.1] - 2026-10-26

### Ajouté
- Lecteur : un clic ou un glisser sur n'importe quelle piste audio déplace la tête de lecture ;
  elle se cale sur l'entrée et la sortie quand elle passe tout près, et suit l'entrée quand on
  la déplace (GAL-03).

### Corrigé
- Les sons de PalaClips (carillon de sauvegarde, début et fin d'enregistrement) n'entrent plus
  dans la piste 1 en mode « Audio du bureau » : sa source est « Tous les sons sauf PalaClips »
  par défaut, ou une sortie précise au choix (ADR-041). Une config qui avait déjà ce mode garde
  sa sortie et son calibrage.
- Un clip supprimé ou renommé (galerie, Explorateur, quota), un fichier ajouté ou un changement
  de dossier met à jour tout de suite « Derniers clips » et l'espace disque sur État, sans
  relancer ni changer de page. Ouvrir un clip dont le fichier a disparu affiche « Ce fichier
  n'existe plus » et retire la carte ; le lecteur se ferme si son clip est supprimé ; le choix
  des clips à compresser et les brouillons de compression suivent aussi.

## [1.3.0] - 2026-10-26

### Ajouté
- Qualité propre de l'enregistrement (REC-07, ADR-040) : dans Réglages → Replay & qualité,
  l'enregistrement suit le replay ou prend son codec, sa qualité, ses images par seconde et sa
  résolution. Le replay continue pendant l'enregistrement et reste sauvegardable.
- Estimation de la taille des clips (RPL-11) : Mo par minute, taille d'un clip, avec l'« après »,
  et Mo par minute et par heure pour l'enregistrement.
- Accès direct à Replay & qualité : résumé de la qualité cliquable sur État, bouton sur Capture.
- Son au début et à la fin d'un enregistrement (désactivable avec le son de confirmation).

### Modifié
- Plus de maximum de pistes d'applis : au-delà de 12, une note dit ce qu'elles coûtent.
- Moins de processeur avec beaucoup de pistes d'applis (un seul relevé des processus pour
  toutes les pistes : 11,5 % → 7 % de la machine à 24 pistes).

### Corrigé
- Compression « Garder toutes les pistes » avec beaucoup de pistes : le débit audio baisse au
  lieu de refuser une cible pourtant suffisante.
- Des réglages ne semblaient pris en compte qu'après une relance (ADR-039) : l'écran État
  affichait encore l'ancienne durée du replay et l'ancien raccourci, et une page ouverte avant
  un changement fait ailleurs pouvait l'écraser. Le moteur appliquait déjà chaque réglage ; la
  config affichée suit maintenant chaque changement, et chaque page n'envoie que ce qu'elle
  modifie.

## [1.2.0] - 2026-10-24

### Ajouté
- Enregistrer après la sauvegarde (RPL-10, ADR-037) : réglage « Après l'appui » (0 s par défaut,
  jusqu'à 10 min) dans Réglages → Replay & qualité. Le tampon est écrit tout de suite dans un MP4
  hybride, puis le direct y est ajouté jusqu'à la fin ; un nouvel appui prolonge le même clip ;
  « Terminer maintenant » sur État et dans la zone de notification ; carillon et notification
  « Clip en cours » à l'appui. Clip récupéré au démarrage après un arrêt brutal.
- Compression à une taille cible (CMP-01 à 05, ADR-038) : page Compression (20 Mo Discord,
  50 Mo Nitro Basic, 500 Mo Nitro ou taille libre ; H.264 ou AV1 ; piste 1 seule ou toutes les
  pistes), résolution et images par seconde choisies selon la taille, taille vérifiée et
  nouvel essai si besoin, jamais au-dessus de la cible ; HDR converti en SDR ; original déjà
  sous la cible copié sans réencodage. Depuis la galerie (un ou plusieurs clips), le lecteur
  (découpe comprise) ou par glisser-déposer depuis l'Explorateur ; file d'attente qui continue
  fenêtre fermée ; Copier, Ouvrir le dossier, Réessayer.
- CLI : `replay --after`, `compress` ; bancs `post-roll.ps1`, `post-roll-memory.ps1`,
  `compress-check.ps1`, `clip-continuity.py`.

### Changé
- Barre latérale : État, Galerie, Capture, Audio, Compression, Réglages ; Capture et Audio
  sortent de Réglages (APP-14, ADR-036). Quitter une page, une section de Réglages ou fermer la
  fenêtre avec un changement non appliqué demande « Appliquer et redémarrer », « Ne pas
  appliquer » ou « Rester ». Liens directs depuis État et les alertes.
- Tailles affichées en unités binaires, comme l'Explorateur et Discord.

## [1.1.1] - 2026-10-22

### Corrigé
- Encodage H.264 sur le processeur : environ 170 Mo de mémoire en 1080p au lieu de ~720 Mo, et
  moins de CPU, en limitant les fils de l'encodeur de Windows (2 jusqu'à 1080p60, 4 au-delà).
- Windows en édition N ou KN sans Media Feature Pack : l'encodage sur le processeur passe tout
  seul en AV1 au lieu d'échouer en H.264 ; Réglages le dit sous Codec.

### Ajouté
- CLI : `probe-mf` (pilote l'encodeur H.264 de Windows et mesure sa mémoire selon ses réglages).

## [1.1.0] - 2026-10-22

### Ajouté
- Encodage par le processeur quand aucun encodeur de la carte graphique ne marche : H.264 par
  l'encodeur logiciel de Windows, AV1 par SVT-AV1, détectés par un encodage de test réel
  (ENC-06, ADR-035). Conversion d'image sur le GPU puis une seule relecture, ou par le
  processeur quand il n'y a pas de Video Processor (machine virtuelle) ; fils d'encodage en
  priorité basse ; au-delà d'une demi-seconde de retard, des images sont sautées et l'alerte
  « le processeur n'arrive pas à encoder » s'affiche.
- Réglage « Encodeur » dans Réglages → Replay & qualité : Automatique (carte graphique si elle
  marche, sinon processeur), Carte graphique ou Processeur. Sur le processeur : passage en
  1080p30, avertissement sur la charge en jeu, mémoire de l'encodeur ajoutée à l'estimation.
- Découpe précise et export réencodent aussi sur le processeur sans encodeur matériel du codec.
- CLI : `--encoder auto|materiel|cpu`, `probe-encoder-threads` ; banc `scripts/cpu-encoders.ps1`
  (coût et qualité des encodeurs logiciels), `fps-impact.ps1 -Cpu`, `t2-av-sync.ps1 -VideoArgs`.

### Changé
- Premier lancement : sans encodeur matériel, il propose le processeur au lieu de bloquer ;
  il ne bloque plus que si aucun encodeur ne marche.
- Panneau de performances : « Processeur · H.264 » au lieu du nom de l'encodeur FFmpeg.
- Message « aucun encodeur de la carte graphique » : propose de choisir le processeur.
- Notes de version : un court résumé rédigé en français et en anglais remplace la liste brute
  des PR, sur GitHub comme dans l'app ; l'app n'affiche que la partie dans sa langue.
- Essai de mise à jour (`scripts/update-test.ps1`) : version prise dans le tag de test, notes
  de version réelles dans le flux local.

## [1.0.0] - 2026-10-21

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
- Feuille de route publique (`ROADMAP.md`, `ROADMAP.fr.md`) : compatibilité sur plus de
  systèmes, encodage par le processeur, autres pistes.
- Mises à jour : recherche au démarrage puis toutes les 6 h, alerte quand une version sort,
  Réglages → Général → Mises à jour avec les notes de version et « Installer et redémarrer »
  (signature vérifiée, refusé pendant un enregistrement).
- Workflow de release : un tag `vX.Y.Z` construit et signe l'installeur, puis prépare une
  release brouillon avec `latest.json` et les notes du CHANGELOG.
- Revue finale de la V1 (`docs/reports/release-v1.md`) : chaque exigence de la SPEC avec sa
  preuve ; bancs `shared-encoder.ps1` (un seul encodeur pour le replay et l'enregistrement).

### Changé
- Nouveau logo : un triangle de lecture découpé en trois bandes, les pistes séparées qui jouent
  ensemble ; icônes de l'appli, de l'installeur et de la zone de notification régénérées.
- Premier lancement en questions : une question à la fois avec de grandes réponses cliquables
  (écran, micro, piste 1, durée et stockage du replay, dossier, raccourci), au clavier aussi ;
  les vérifications ne s'affichent qu'en cas de problème ; récapitulatif où chaque réponse se
  modifie avant le clip de test.
- Notes de release générées automatiquement à partir des PR mergées, classées par type
  (fonctionnalités, corrections, performances, documentation, maintenance).
- README anglais et français restructurés : logo, badges, sommaire, configuration requise,
  contribution.

### Corrigé
- Minecraft Java en plein écran (F11) : l'alerte « plein écran exclusif » donne la solution qui
  marche (fenêtre agrandie ou mod de plein écran sans bordure), et la FAQ du README aussi.
- Sans encodeur matériel : le message ne se coupe plus au milieu d'un mot au premier lancement
  et se lit d'une traite dans l'alerte de l'accueil.
- L'écran choisi pour la capture est retrouvé même quand Windows renumérote ses écrans
  (`\\.\DISPLAY1`, `2`…) : il est mémorisé par le chemin stable du moniteur ; un écran débranché
  n'est jamais remplacé par un autre.
- Windows requis : Windows 11 (le build 20348 de la capture audio par appli n'existe pas en
  Windows 10 grand public) ; sur un Windows trop ancien, une alerte le dit dès le démarrage.
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
