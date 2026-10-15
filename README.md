# PalaClips

Enregistrement et **replay instantané** pour Windows, léger, gratuit et open source — avec
**une piste audio par logiciel** (jeu, Discord, musique, navigateur, micro), prête pour le
montage dans DaVinci Resolve.

> **Statut : en développement — phase 0.** Voir [`docs/HANDOFF.md`](docs/HANDOFF.md) et
> [`docs/PLAN.md`](docs/PLAN.md).

## Ce que fait la V1

- Replay instantané : les dernières secondes (5 s à 60 min) gardées en mémoire, sauvegardées
  d'un raccourci sans quitter le jeu.
- Enregistrement manuel, résistant aux crashs.
- **Séparation réelle de l'audio** : mix complet, micro, puis une piste par logiciel choisi,
  nommée, synchronisée.
- Capture d'un écran ou d'une fenêtre, fps constant, encodage matériel (NVIDIA, AMD, Intel).
- Galerie avec lecteur (écoute piste par piste), découpe et export.
- Vit dans la zone de notification ; fenêtre fermée = un seul petit processus.
- Aucune injection dans les jeux (compatible anti-cheat), aucun compte, aucune télémétrie.

Détail complet : [`docs/SPEC.md`](docs/SPEC.md).

## Prérequis (développement)

- Windows 11 (ou Windows 10 build 20348+), GPU avec encodeur matériel.
- [Rust](https://rustup.rs) (toolchain MSVC), Visual Studio Build Tools (C++), LLVM
  (`winget install LLVM.LLVM`), Node.js ≥ 20.19, PowerShell 7 (`pwsh`).
- Claude Code avec les skills de design :
  - impeccable : dans Claude Code, `/plugin marketplace add pbakaus/impeccable`, puis
    `/plugin` → installer **impeccable** ;
  - emil-design-eng : `npx -y skills add emilkowalski/skill --skill emil-design-eng --agent claude-code`.

## Démarrer

```powershell
git clone https://github.com/Palawizard/PalaClips.git
cd PalaClips
pwsh scripts/check-env.ps1          # vérifie la machine
pwsh scripts/fetch-ffmpeg.ps1       # FFmpeg LGPL (DLL) dans third_party/
cd app
npm ci
npm run tauri dev
```

Banc de test du moteur : `cargo run -p palaclips-cli -- info`.

## Documentation

| Fichier | Contenu |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | Règles de travail pour Claude Code |
| [`docs/SPEC.md`](docs/SPEC.md) | Tout ce que contient la V1 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Modules, threads, IPC, données |
| [`docs/PLAN.md`](docs/PLAN.md) | Phases et critères d'acceptation |
| [`docs/HANDOFF.md`](docs/HANDOFF.md) | Où on en est |
| [`docs/DECISIONS.md`](docs/DECISIONS.md) | Choix d'architecture (ADR) |
| [`docs/RESEARCH.md`](docs/RESEARCH.md) | Faits vérifiés et pièges, avec sources |
| [`docs/TESTING.md`](docs/TESTING.md) | Protocoles de test manuels |
| [`docs/DESIGN-BRIEF.md`](docs/DESIGN-BRIEF.md) | Brief pour la conception de l'interface |

## Installation (utilisateurs, à partir de la V1)

L'installeur n'est pas signé par un certificat payant : au premier lancement, Windows
SmartScreen peut afficher « Windows a protégé votre ordinateur » → *Informations
complémentaires* → *Exécuter quand même*.

## Licence

[MIT](LICENSE). PalaClips utilise FFmpeg (LGPL 2.1+) en bibliothèques dynamiques.
