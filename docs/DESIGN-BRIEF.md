# Brief design — à donner à `/impeccable init`

> Ce brief alimente `/impeccable init`, qui écrit `PRODUCT.md` et `DESIGN.md` à la racine.
> Ensuite, ce sont **ces deux fichiers** qui font foi pour le design.

## Produit

PalaClips enregistre ce qui se passe à l'écran et garde les dernières secondes de jeu en
mémoire, pour les sauvegarder d'un raccourci. Sa différence : **chaque logiciel sur sa propre
piste audio** (jeu, Discord, musique, micro), prêt pour le montage.

Utilisateur : joueur et créateur de contenu sur Windows, à l'aise avec la technique, qui monte
ses vidéos dans DaVinci Resolve. Il ouvre l'app pour régler, puis l'oublie : elle vit dans la
zone de notification. Il revient surtout pour **la galerie** (revoir, découper, partager).

## Personnalité

- Outil de précision, calme et rapide. Pas « gamer RGB », pas de néon, pas de dégradés criards.
- Confiance : on voit tout de suite si le replay tourne, ce qui est capturé, et ce que ça coûte
  (RAM, CPU).
- Densité maîtrisée : beaucoup de réglages, mais jamais tous en même temps.
- Natif Windows 11 dans l'esprit (sobriété, coins, rythme), avec sa propre identité.
- Sombre par défaut, thème clair complet.

## Écrans (V1, tous obligatoires)

1. **État** (accueil) : replay armé ou non, durée en mémoire, RAM du buffer, ce qui est
   capturé (zone + pistes), raccourcis rappelés, derniers clips.
2. **Galerie** : grille de clips (miniature, durée, jeu, date), filtres, recherche, favoris,
   sélection multiple. Doit rester fluide à 5 000 clips.
3. **Lecteur / découpe** : vidéo, timeline, poignées de découpe, **une ligne par piste audio**
   (forme d'onde, mute, solo, volume), export rapide/précis, pistes à garder ou mixer.
4. **Zone de capture** : écrans et fenêtres en **miniatures vivantes**, icône et nom d'appli,
   options curseur et bordure.
5. **Sources audio** : le sélecteur « propre et sympa ». Applis qui font du son en tête avec
   **vumètre vivant**, icône, nom ; recherche ; ajout d'une appli non lancée ; glisser pour
   ordonner ; renommer ; micro + gain ; piste « Jeu » automatique.
6. **Replay & qualité** : durée, fps, résolution, codec, débit, préréglages ; **estimation de
   la RAM qui bouge en direct** pendant qu'on règle.
7. **Raccourcis** : capture d'une combinaison au clavier, conflits signalés.
8. **Stockage** : dossier, quota, espace utilisé.
9. **Général / À propos** : démarrage avec Windows, langue, thème, mises à jour, diagnostic,
   performances.
10. **Premier lancement** : parcours guidé court qui finit par un vrai clip sauvegardé.
11. Hors fenêtre : **menu de la zone de notification** et **toasts Windows**.

## Mouvement (skill `emil-design-eng`)

- Les animations servent la compréhension (apparition d'un clip sauvegardé, vumètres,
  changement d'état du replay), jamais la décoration.
- Interactions déclenchées par l'utilisateur : rapides (ordre de 150–250 ms), easing
  personnalisé, interruptibles.
- `prefers-reduced-motion` respecté partout.
- Vumètres : 60 fps fluides sans faire monter le CPU (données à ~20 Hz, interpolation côté UI).

## Contraintes techniques

- React 19 + TypeScript + Tailwind 4 (jetons dans `@theme`, dans `app/src/styles.css`).
- Aucune police ni ressource chargée depuis Internet (app hors ligne) : polices embarquées.
- Fenêtre min. 900 × 600, cible 1180 × 760, redimensionnable ; mise à l'échelle Windows 100–200 %.
- Accessibilité : clavier complet, focus visible, contrastes AA minimum.
- FR et EN : prévoir des libellés plus longs en français.
