---
version: 1
slug: "app-src-app-tsx"
primary_target: "app/src/App.tsx"
related_targets: []
---

# Surface : État (accueil) et shell de l'app

Mode : Operate. Chemin de build : code-led (pas de génération d'image).

## Tâche

Ouvrir l'app et savoir en un coup d'œil : le replay est-il armé, combien de secondes sont en
mémoire, ce que ça coûte en RAM, quelle zone et quelles pistes sont capturées, quel raccourci
sauvegarde, quels sont les derniers clips. États importants : replay armé / désarmé / en
erreur, piste muette anormale (`app_track_silent`), appli attendue non lancée, aucun encodeur.

## Direction contract

THESIS : chaque piste audio est un joueur de l'équipe, alignée en roster comme un tableau des
scores de fin de manche ; on refuse le tableau de bord « cartes sombres + un accent néon » de
la catégorie.

OWN-WORLD : fond nuit de stade bleu-noir (#0F1420, surfaces #171D2B), encre #E8ECF4 ; une
couleur de maillot mi-saturée par piste (mix encre, micro #F2B84B, applis #4FC1B9, #8F86F5,
#5AA9E6...), langue fixe jamais réattribuée ; rouge #FF4D5E réservé au seul sens « ça
enregistre ». Lignes de tableau des scores, chiffres tabulaires larges, bandes de couleur en
bord de ligne. Recomposition par cellules entières sur une grille stricte.

STORY : le joueur voit que tout tourne, reconnaît chaque piste à sa couleur, et retrouve
cette même couleur dans les Sources audio, le lecteur, la galerie et le toast.

FIRST VIEWPORT : rail de navigation à gauche (État, Galerie, Capture, Audio, Replay &
qualité, Raccourcis, Stockage, Général). En tête, bandeau d'état pleine largeur : témoin
rouge, « En mémoire 01:00 / 01:00 », RAM du buffer, raccourci de sauvegarde, action
armer/désarmer. Centre : roster des pistes, une ligne par piste (numéro, bande de couleur,
icône, nom, vumètre vivant, crête en dB, état). Colonne droite : derniers clips en cartes
de récap (miniature, durée, jeu, nombre de pistes en pastilles de couleur).

SIGNATURE : le roster. À la sauvegarde, la ligne de chaque piste se fige une fraction de
seconde et le clip glisse dans « Derniers clips » comme une carte de récap. Mouvement :
150–250 ms, easing personnalisé, interruptible, vumètres interpolés à 60 fps depuis ~20 Hz,
`prefers-reduced-motion` → changements d'état instantanés.

FORM : écran d'après-match des FPS compétitifs, candidat 4 de ma liste ordonnée, seed key
cabeab00. Raises : rouge réservé à un sens (signalétique d'aéroport), une couleur = une piste
(carte d'orientation), recomposition par cellules (grille de Crouwel).

FINISH : unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Non résolu

- Polices embarquées (workhorse UI à chiffres tabulaires) : à choisir au premier build.
- Thème clair complet : dérivé du même système au premier build.

## Brief confirmé par Palawi (shape, 2026-10-16) — prioritaire sur le contrat ci-dessus

- Rail : **État / Galerie / Réglages** ; Réglages en sous-sections Capture, Audio, Replay &
  qualité, Raccourcis, Stockage, Général.
- Bandeau d'état pleine largeur :
  - replay : armé / désarmé / armement / erreur, « En mémoire 00:47 / 01:00 », RAM du
    tampon, raccourci de sauvegarde (`Alt+F10` par défaut), Armer/Désarmer. Replay armé =
    témoin discret ;
  - enregistrement manuel : état, durée, taille, raccourci, Démarrer/Arrêter. **Rouge plein
    réservé à l'enregistrement en cours** : replay armé et enregistrement se distinguent d'un
    coup d'œil ;
  - source capturée : écran ou fenêtre, résolution, i/s ; repli fenêtre → écran signalé ;
  - espace disque restant.
- Zone d'alertes générales sous le bandeau : CAP-11 (G-Sync), périphérique audio changé,
  encodeur, raccourci indisponible, disque presque plein. Si `Alt+F10` ne peut pas être
  enregistré : alerte actionnable qui cite l'overlay NVIDIA comme cause probable.
- Roster au centre : une ligne par piste dans l'ordre du fichier (Mix, Micro, applis, Jeu),
  numéro, bande de couleur fixe, icône, nom, vumètre vivant, crête en dB, état (en attente de
  l'appli, `app_track_silent`). 1 à 14 pistes.
- Colonne droite « Derniers clips » : les 5 clips les plus récents du dossier, miniature JPEG
  écrite par le moteur à la sauvegarde (réutilisée par la galerie en phase 6) ; état vide qui
  renvoie au raccourci. Sous ~1000 px de large, la colonne passe sous le roster.
- Moment fort : **pas de gel des vumètres** ; bref surlignage des lignes du roster et carte
  du clip qui arrive dans « Derniers clips ».
- Vumètres : `audio_levels` ~20 Hz interpolé à 60 fps ; ~10 i/s quand la fenêtre n'a pas le
  focus ; flux coupé côté moteur quand la fenêtre est cachée, réduite ou fermée. Mesurer le
  CPU fenêtre ouverte non focalisée pendant une capture (PERF-03).
- États : aucun encodeur (armement impossible, expliqué), erreur du replay (message
  actionnable), FR plus long que EN, 900×600 à 1180×760+, mise à l'échelle 100–200 %,
  clavier complet, `prefers-reduced-motion` → changements instantanés.
- Moteur/IPC à ajouter : `get_state`, `replay_arm`/`replay_disarm`/`replay_save`,
  `record_start`/`record_stop`, `audio_levels`, liste des clips récents, miniature à la
  sauvegarde, raccourci global ; `ipc.ts` et `ARCHITECTURE.md` § IPC synchronisés.

## Panneau de performances (shape validé par Palawi, 2026-10-19)

- Section « Performances » repliable sous le roster, fermée par défaut, état mémorisé
  (localStorage) ; mesures demandées au moteur seulement panneau ouvert et fenêtre visible.
- Quatre cellules entières séparées par des filets : Moteur (CPU en % de la machine, max depuis
  l'armement, Mo hors tampon = working set privé − tampon × 1,07), Tampon (Mo, secondes),
  Capture (titre = encodeur, ou état du replay, ou « Aucun encodeur matériel » en ambre ;
  résolution et i/s ; décalage max), Images (% répétées sur 10 s, en retard depuis l'armement
  et sur la dernière minute). Pas de graphique.
- Replay désarmé : Moteur seul, les autres cellules montrent un tiret.
- Mouvement : contenu en opacité + translateY(−4 px) 200 ms à l'ouverture (@starting-style),
  fermeture instantanée, chevron 160 ms ; chiffres sans animation ; tout à 0 ms en
  mouvement réduit.
