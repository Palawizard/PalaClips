---
description: Vérifier tous les critères d'acceptation de la phase en cours et écrire le rapport
---

Valide la phase en cours (ou celle donnée en argument : $ARGUMENTS).

1. Relis la phase dans `docs/PLAN.md` et les exigences de `docs/SPEC.md` qu'elle couvre.
2. Pour chaque critère 🤖 : exécute réellement la vérification (tests, commande CLI, mesure)
   et garde la sortie comme preuve.
3. Pour chaque critère 👤 : donne-moi le protocole exact (depuis `docs/TESTING.md`), les
   commandes à lancer, et ce que je dois te rapporter. Attends mes résultats.
4. Vérifie qu'il ne reste ni `todo!()` ni `EngineError::NotImplemented` dans le code de cette
   phase.
5. Écris `docs/reports/phase-N.md` (modèle dans `docs/reports/README.md`). Un critère sans
   preuve reste ❌.
6. Si tout est ✅ : coche la phase dans `docs/PLAN.md`, mets à jour `docs/HANDOFF.md` vers la
   phase suivante, ouvre la PR de la branche en cours **vers `dev`** (titre au format
   conventionnel ; vers `main` et `dev` seulement pour une `hotfix/`). La PR `dev` → `main`
   n'est ouverte que pour publier une version, à la demande de Palawi.
   Sinon : liste ce qui manque et corrige-le avant toute autre chose.
