# Rapports de validation

Un fichier par phase : `phase-N.md`, créé par `/valider-phase`. Le rapport final de release
est `release-v1.md` (chaque exigence de `docs/SPEC.md` cochée avec sa preuve).

Modèle :

```markdown
# Phase N — <nom>

- Date :
- Commit :
- Machine : Windows <build>, <GPU> <pilote>, <CPU>

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| ... | 🤖/👤 | ✅/❌ | commande + sortie / capture / fichier |

## Exigences SPEC couvertes

CAP-01, ...

## Écarts et corrections
```
