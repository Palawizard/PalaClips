---
description: Concevoir ou retravailler un écran avec impeccable + emil-design-eng
argument-hint: <écran ou composant>
---

Travail front sur : $ARGUMENTS

Workflow obligatoire (CLAUDE.md § 2) :

1. Vérifie que `PRODUCT.md` et `DESIGN.md` existent à la racine. Sinon : `/impeccable init`
   en lui donnant `docs/DESIGN-BRIEF.md`, et arrête-toi pour que je valide la direction.
2. `/impeccable shape` pour cet écran : structure, hiérarchie, états (vide, chargement,
   erreur, beaucoup de données), libellés FR et EN.
3. Implémente dans `app/src/` avec les jetons de `DESIGN.md` (déclarés dans `@theme` de
   `app/src/styles.css`). Données réelles via `app/src/lib/ipc.ts`, pas de maquette figée.
4. Mouvement et interactions : applique le skill `emil-design-eng` (durées, easing, ressorts,
   interruptibilité, `prefers-reduced-motion`).
5. `/impeccable critique` puis `/impeccable audit` ; corrige tout ce qui est bloquant.
6. Lance `npm run tauri dev`, vérifie l'écran à 100 % et 150 % de mise à l'échelle, au clavier,
   en thème sombre et clair.
7. `/impeccable polish` avant de considérer l'écran terminé. Montre-moi le résultat.
