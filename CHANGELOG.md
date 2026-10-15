# Changelog

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions
[SemVer](https://semver.org/lang/fr/).

## [Non publié]

### Ajouté
- Création du repo : workspace Cargo (`palaclips-engine`, `palaclips-cli`, app Tauri 2),
  front React 19 + TypeScript + Tailwind 4, CI Windows.
- Moteur : horloge `Hns` (QPC), `EngineConfig` validée, contrat `EngineEvent`, file de replay
  `PacketRing` purgée par âge, estimation de RAM — avec tests.
- Documentation : SPEC V1, architecture, plan par phases, handoff, ADR, recherche, protocoles
  de test, brief design.
