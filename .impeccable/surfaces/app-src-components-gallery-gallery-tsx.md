---
version: 1
slug: "app-src-components-gallery-gallery-tsx"
primary_target: "app/src/components/gallery/Gallery.tsx"
related_targets: ["app/src/components/gallery/Player.tsx"]
---

# Surface : Galerie + lecteur

Mode : Operate. Chemin de build : code-led. Brief confirmé par Palawi le 2026-10-18.

## Tâche

Revoir, trier, découper et partager ses clips après une session (des dizaines, jusqu'à 5 000).
Succès : retrouver un clip en quelques secondes, l'écouter piste par piste, sortir un extrait
(rapide ou à l'image près, pistes choisies ou mixées) et le copier dans Discord.

## Direction

Monde « The After-Match Scoreboard » de DESIGN.md, inchangé.
- Galerie : barre de filtres en haut (recherche, jeu, type, favoris, période, tri), grille de
  cartes rangée par jour (« Aujourd'hui · 6 », « Hier · 11 », « 14 oct. ») en tri par date,
  plate pour les autres tris. Carte : miniature, durée condensée, jeu, heure, taille, points
  de maillot par piste, étoile favori, repère « Enr. » pour un enregistrement.
- Lecteur en vue pleine (retour = Échap) : vidéo, frise (lecture, temps, poignées entrée /
  sortie), une ligne par piste façon roster (plaque, nom, forme d'onde à la couleur de la
  piste, M / S, volume), bande d'export (plage, Rapide / Précise, cases des pistes,
  Séparées / Mixées, nom, Exporter → progression + Annuler → Montrer / Copier).
- Moment fort : la ligne de pistes ; couper ou isoler une piste s'entend immédiatement.

## États et plages

Chargement : squelette discret des cartes. Vide : cadre pointillé nommant le raccourci de
sauvegarde. Aucun résultat : « Effacer les filtres ». Erreur : alerte actionnable. 0 à 5 000
clips, 1 à 15 pistes, 3 s à plusieurs heures. Préparation des pistes : la vidéo joue déjà.

## Interactions

Grille virtualisée maison par rangées, focus mobile (flèches, Entrée ouvrir, F favori, F2
renommer, Suppr Corbeille sans confirmation, Ctrl+C copier) ; menu « … » par carte (mêmes
actions + Ouvrir l'emplacement). Lecteur : Espace, ← / → ±1 s, , / . image par image, I / O
entrée / sortie. Audio : un `<audio>` par piste branché sur Web Audio (gain, mute, solo),
recalé au-delà de ~40 ms ; jamais de piste décodée en entier (ADR-008). Formes d'onde
calculées en Rust (pics depuis les .m4a extraits). Mouvement informatif seulement, 0 ms sous
`prefers-reduced-motion`.

## Contraintes

Clés i18n fr / en, textes sobres. AA, focus visible, 900 × 600 à 200 % ; sous 1000 px la
bande d'export passe sous les pistes. Mesures : galerie < 500 ms et 60 i/s à 5 000 clips
(dossier isolé), déplacement < 300 ms dans un clip de 4 Go, RAM du lecteur sur 1 h.
