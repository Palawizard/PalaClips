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
