---
description: Mettre à jour HANDOFF.md et CHANGELOG.md en fin de session
---

Fin de session. Mets à jour la mémoire du projet pour que la prochaine session reprenne sans
rien me demander.

1. `docs/HANDOFF.md` :
   - « État actuel » : phase, branche, date du jour ;
   - « Ce qui existe » : ce qui a changé aujourd'hui (fichiers, modules, commandes CLI) ;
   - « Prochaines actions » : liste ordonnée, précise, actionnable ;
   - « Bloquants » et « Points d'attention » à jour ;
   - une ligne dans le journal des sessions.
2. `CHANGELOG.md` : section « Non publié », ce qui est visible ou utile.
3. Coche dans `docs/PLAN.md` les tâches réellement terminées (pas les critères non vérifiés).
4. Si une décision a été prise : nouvel ADR dans `docs/DECISIONS.md`. Si un fait a été vérifié :
   ajoute-le à `docs/RESEARCH.md` avec la source et la date.
5. Vérifie `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test --workspace` et `npm run build` (dans `app/`), puis commite :
   `docs(handoff): update session notes` (le code a ses propres commits conventionnels, voir
   `CLAUDE.md` § 4 Git).
6. Pousse la branche. Si elle a une PR vers `dev` dont la tâche est terminée : attends la CI,
   merge et supprime la branche (`CLAUDE.md` § 4 Git, « Merge et ménage »). Sinon, laisse la
   PR ouverte et note-la dans « État actuel ».
7. Résume-moi en 5 lignes : fait, reste, ce que j'ai à tester.
