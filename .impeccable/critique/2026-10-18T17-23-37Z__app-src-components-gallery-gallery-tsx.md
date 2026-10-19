---
target: Galerie + lecteur
total_score: 22
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
target_identity: "file:C:\\Projets-NAS-Sync\\Code\\apps\\PalaClips\\app\\src\\components\\gallery\\Gallery.tsx"
target_fingerprint: "sha256:691cb56e77535f64363330c1a95e3565c1beb20560d5a8849245f807b30cee44"
target_path: "C:\\Projets-NAS-Sync\\Code\\apps\\PalaClips\\app\\src\\components\\gallery\\Gallery.tsx"
timestamp: 2026-10-18T17-23-37Z
slug: app-src-components-gallery-gallery-tsx
closed: true
---
Method: dual-agent (A: design review · B: detector)

| # | Heuristique | Note | Problème principal |
|---|---|---|---|
| 1 | Visibilité de l'état | 3 | Pas à pas image par image sans retour visible (mm:ss) ; piste coupée/non isolée garde sa forme d'onde pleine |
| 2 | Correspondance au réel | 3 | Titres de cartes = noms de fichier machine ; « Découpe » vs « Exporter » |
| 3 | Contrôle et liberté | 2 | Corbeille sans annulation ; focus perdu au retour du lecteur ; pas de remise à zéro de la plage ni du mix |
| 4 | Cohérence | 2 | « Tri » seul étiqueté ; pas des poignées 1/30 s vs image 1/60 s ; M et S actifs identiques ; drop-shadow sur l'icône lecture |
| 5 | Prévention des erreurs | 2 | Corbeille aussi visible que Favoris ; « Mixées » + piste Mix complet = son doublé |
| 6 | Reconnaissance plutôt que rappel | 1 | Aucun raccourci affiché ; étoile et menu « … » seulement au survol |
| 7 | Flexibilité | 3 | Bons raccourcis, mais pas de touche pour le menu de carte ni pour aller à l'entrée/sortie |
| 8 | Esthétique minimaliste | 3 | Noms tronqués, durée doublée sans libellé, Période seule sur une ligne à 900 px |
| 9 | Récupération après erreur | 2 | Pas de « Réessayer » ; échec de préparation = faux « pas de piste audio » |
| 10 | Aide | 1 | Aucune référence des raccourcis |
| **Total** | | **22/40** | **Acceptable** |

Spécificité : lecteur authored (plaques de maillot, formes d'onde colorées, roster) ; galerie interchangeable (grille + barre de filtres génériques, points de pistes minuscules et muets). Détecteur : 0 résultat (galerie, styles.css, gallery.ts). Overlay navigateur : impossible (WebView2, rien à l'écran pendant que Palawi joue).

Priorités :
- [P1] Actions de carte inatteignables au clavier (étoile, « … » en tabIndex -1), raccourcis invisibles. Correctif : menu par touche Menu / Maj+F10, aria-keyshortcuts, touches dans les infobulles et le menu, aide « ? ».
- [P1] Corbeille sans retour arrière. Correctif : « Annuler » dans l'avis (restauration depuis la Corbeille), Corbeille en icône discrète à droite dans le lecteur.
- [P1] Vidéo, transport et Exporter défilent hors de vue. Correctif : vidéo + transport fixes, bande d'export en pied fixe repliée sur une ligne, seules les pistes défilent.
- [P2] Titres de cartes gaspillés par le nom auto. Correctif : jeu + heure comme titre tant que le clip n'est pas renommé.
- [P2] Précision invisible. Correctif : mm:ss.ff à l'arrêt / en Précise, pas d'image selon les i/s du clip, piste coupée atténuée.

Personas : Alex (pas de touche menu ni « montrer », pas d'aller à l'entrée/sortie, volume 150 % sans repère à 100 %) ; Sam (focus non rendu à la carte, lecteur ouvert sur un div muet, tête de lecture non exposée, pistes des cartes non annoncées, aria-rowindex manquant).

Mineurs : Période isolée à 900 px ; durée d'export sans libellé ; nom d'export sans extension ni dossier ; avis jamais effacés ; plage de découpe peu visible ; puces « Enr. » et durée identiques.
