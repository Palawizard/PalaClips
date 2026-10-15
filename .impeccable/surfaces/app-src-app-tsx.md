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
