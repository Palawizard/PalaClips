---
version: 1
slug: "app-src-components-settings-tsx"
primary_target: "app/src/components/Settings.tsx"
related_targets: []
---

# Surface : Réglages

Mode : Operate. Monde établi (`DESIGN.md`, « nuit de stade ») : pas de nouveau monde, pas de
tirage de concepts. Chemin de build : code-led.

## Brief confirmé par Palawi (shape, 2026-10-17)

- Structure : rail (Réglages) + sous-menu à gauche (Capture, Audio, Replay & qualité,
  Raccourcis, Stockage, Général), une section à la fois ; contenu en lignes de réglage
  (libellé, contrôle, ligne d'aide ou d'erreur) séparées par des filets, cellules entières.
- Application immédiate et enregistrée (`set_config`, `set_feedback`, `set_hotkeys`,
  `set_autostart`) ; les réglages qui redémarrent le tampon (zone, sources, qualité)
  demandent d'abord confirmation en ligne : « Appliquer redémarre le replay : les secondes en
  mémoire seront perdues » + Appliquer / Annuler (RPL-07).
- Premier lot : Raccourcis et Général complets ; les autres sections suivent dans des lots
  dédiés.
- Raccourcis : « Sauvegarder le replay », « Armer ou désarmer le replay » ; clic sur le champ →
  « Appuie sur la combinaison… » ; Échap annule, Retour arrière retire le raccourci ; erreurs
  sous le champ (réservé, doublon, déjà pris → ancien gardé) ; raccourci indisponible au
  démarrage signalé ; en pied : « Aucun conflit détecté » (jamais « aucun conflit ») + rappel :
  Discord et l'overlay NVIDIA lisent le clavier sans le déclarer, vérifier leurs raccourcis.
- Général : Démarrer avec Windows, Armer le replay au démarrage, Son de confirmation,
  Notification de confirmation, Thème (Système / Sombre / Clair), Réinitialiser les réglages
  (confirmation en ligne, pas de fenêtre modale).
- Clavier complet ; textes sobres (règle de copie de `PRODUCT.md`) ; 900×600 à 1180×760.

## Signature

Les champs de raccourci se lisent comme des touches du tableau des scores (Barlow Condensed,
plaques `kbd`) ; l'état « Appuie sur la combinaison… » est le seul moment animé.

## Contraintes par section (Palawi, 2026-10-17)

- **Capture** : les miniatures en direct ouvrent une capture par écran ou fenêtre ; seulement
  pendant que la section est visible, rafraîchies lentement (~1 par seconde), en nombre limité
  (fenêtres visibles d'abord, le reste à la demande), toutes arrêtées en quittant la section,
  sans bordure de capture. Elles ne servent jamais à la capture réelle.
- **Audio, assistant de calibrage** : il occupe l'écran principal et émet des bips → il
  prévient avant de démarrer, dure moins de 30 s, et rappelle que le retard d'une chaîne
  Voicemeeter/LightHost peut changer à chaque relance ; bouton « Recalibrer » toujours
  visible.
- **Replay & qualité** : en plus de l'estimation (+7 %), alerte si le tampon demandé dépasse une
  part raisonnable de la RAM libre, et propose alors le tampon sur disque (RPL-04).
- **Stockage, quota** : quand il est atteint, ne jamais supprimer un favori ; dire clairement ce
  qui sera supprimé (les plus anciens) et demander confirmation la première fois. Si le disque
  se remplit pendant un enregistrement : arrêt propre avec un fichier lisible et une alerte,
  jamais un fichier cassé (phase 6, REC/STO-05).

## Capture, brief confirmé (shape, 2026-10-17)

- En haut : « Afficher le curseur » et « Bordure jaune de Windows » (CAP-07), lignes de réglage.
- Puis « Écrans » et « Fenêtres » en grille de vignettes 16:9 (3 colonnes à 1180 px, 2 à
  900 px). Carte : miniature, icône de l'appli, nom de l'appli, titre ou définition. Cible
  actuelle cerclée ; point rouge « En capture » si le replay est armé.
- Miniatures en direct : tous les écrans + 6 fenêtres visibles, ~1 image/s, sans bordure ni
  curseur, arrêtées en quittant la section ou quand la fenêtre est cachée. Autres fenêtres :
  icône + « Aperçu » (remplace la plus ancienne). Fenêtre réduite : icône + « Réduite ».
- Exclues : PalaClips, fenêtres minuscules et invisibles.
- Clic = nouvelle cible ; replay armé → barre de confirmation en pied (RPL-07).
- Cible mémorisée dont l'appli est fermée : carte grisée « Pas lancée », toujours
  sélectionnée.
- Clavier : flèches dans la grille, Entrée pour choisir.

## Audio, brief confirmé (shape, 2026-10-17)

- « Pistes » dans l'ordre du fichier, une ligne par piste, vumètre en direct (style de
  l'État) et décalage (−1000 à +1000 ms) :
  - 1 Mix complet : activé, sortie (Défaut ou un périphérique) ;
  - 2 Micro : activé, périphérique (Défaut suit le micro par défaut), gain −20 à +20 dB ;
  - 3 Jeu (auto) : activé, jeu suivi affiché (AUD-06) ;
  - applis : icône, nom renommable (écrit dans le fichier), retirer ;
  - « Pistes d'applis au maximum » (1–12, défaut 6) ; compteur « 4/6 ».
- « Ajouter une appli » sous la liste : recherche ; applis qui font du son en tête avec
  vumètre (rafraîchi seulement pendant que la section est visible) ; puis les autres applis
  ouvertes ; « Parcourir un .exe… » pour une appli non lancée. Maximum atteint : + désactivé
  avec la raison.
- Changement qui redémarre le replay armé → barre de confirmation habituelle.
- Calibrage (AUD-11) : « Calibrer » en haut → encart d'avertissement en ligne (écran
  principal ~20 s, bips, rappel Voicemeeter/LightHost) → plein écran flash + bip → valeurs
  proposées par piste + Appliquer ; « Recalibrer » toujours visible. Deuxième PR.

## Encodeur CPU, brief confirmé (shape, 2026-10-22, v1.1)

- Replay & qualité : ligne « Encodeur » au-dessus de « Codec », segments Automatique · Carte
  graphique · Processeur ; un segment est grisé si aucun encodeur de ce type ne marche sur ce PC.
- Ligne d'aide = ce qui encode réellement ; quand c'est le processeur, en ambre : charge en jeu
  (environ 1 cœur en 1080p30, le jeu peut perdre des fps).
- Passer à « Processeur » ramène les i/s à 30 et la résolution à 1080p si elles étaient plus
  hautes, et HEVC à H.264 ; application par la barre de confirmation habituelle (RPL-07).
- Codecs grisés selon l'encodeur effectif (« Indisponible sur le processeur : HEVC. »).
- Premier lancement sans encodeur matériel : la vérification « Encodeur vidéo » passe en ambre,
  dit que le processeur encodera avec le même avertissement, « Continuer » au lieu du blocage,
  1080p30 appliqué. Blocage seulement si rien ne marche (`no_video_encoder`).
- Panneau de performances : « Processeur · H.264 » / « Processeur · AV1 » au lieu du nom
  FFmpeg de l'encodeur.

## Replay & qualité v1.3, brief confirmé (shape, 2026-10-26)

- Bandeau d'estimation en tête (figures en Barlow Condensed) : taille d'un clip à la durée du
  replay, Mo par minute, taille avec l'« après » quand il est réglé, RAM du tampon ; ligne
  technique (définition · i/s · débit · pistes). Unités binaires de `format::bytes`. Encodage en
  CBR : un seul chiffre « ≈ » (écart mesuré −1,4 à −5,4 %).
- Une partie par usage (choix de Palawi) : « Replay » (durée, après l'appui, tampon, codec,
  qualité, images/s, résolution), « Enregistrement » (Mo/min et Go/h, interrupteur « Comme le
  replay » activé par défaut ; désactivé → codec, qualité, images/s, résolution propres),
  « Commun » (encodeur, images clés, priorité GPU). Titres de partie = lignes `title` sur filet,
  pas de libellé en capitales.
- Les réglages de l'enregistrement ne redémarrent pas le replay (appliqués tout de suite).
- Ancre `#replay-quality` : « Replay et qualité » sur État et Capture ouvre Réglages > Replay &
  qualité, fait défiler et met le focus sur le bandeau d'estimation, en passant par la garde de
  sortie.
- État : sous l'état du replay, « 1440p60 · H.264 · ≈ 103 Mo/min » + bouton secondaire
  « Replay et qualité » ; résumé propre dans la cellule Enregistrement si la qualité diffère.
- Audio : plus de « Pistes d'applis au maximum », plus de compteur « X / max » ni de message
  « plein » ; dès la 13e piste d'appli, note en caption (ink-muted) sous la liste : coût des
  pistes au-delà de 12 (processeur, mémoire, Mo/min), sans couleur d'alerte.
