---
description: Démarrer une session de travail sur PalaClips (lecture du contexte + plan du jour)
---

Tu reprends le travail sur PalaClips.

1. Lis en entier `CLAUDE.md`, puis `docs/HANDOFF.md`, puis la phase en cours dans `docs/PLAN.md`
   et les exigences de `docs/SPEC.md` qu'elle couvre.
2. Lance `git status`, `git log --oneline -10` et `git branch --show-current`.
3. Si la phase en cours touche le front : vérifie que les skills `impeccable` et
   `emil-design-eng` sont disponibles. Sinon, arrête-toi et donne-moi les commandes
   d'installation du README.
4. Donne-moi en 10 lignes maximum :
   - où on en est (phase, branche, dernier commit) ;
   - les bloquants éventuels ;
   - ce que tu proposes de faire maintenant, en tâches courtes, avec les critères
     d'acceptation visés ;
   - ce dont tu auras besoin de moi (tests 👤).
5. Si tu es sur `main` ou `dev`, crée la branche conventionnelle de la tâche à partir de `dev`
   à jour (`feature/...`, `fix/...`, `chore/...` ; `hotfix/...` à partir de `main`, voir
   `CLAUDE.md` § 4 Git) avant toute modification. Jamais de commit direct sur `main` ni `dev`.
6. Puis commence directement par la première tâche, sauf s'il y a un bloquant.

$ARGUMENTS
