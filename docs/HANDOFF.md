# HANDOFF — où on en est

> Mis à jour à la fin de **chaque** session (`/handoff`). Une nouvelle session doit pouvoir
> reprendre en lisant uniquement `CLAUDE.md` + ce fichier.

## État actuel

- **Phase en cours :** 0 — Mise en route (pas commencée)
- **Branche :** `main`
- **Dernière mise à jour :** 2026-10-15, création du repo

## Ce qui existe

- Workspace Cargo (`crates/engine`, `crates/cli`, `app/src-tauri`), edition 2024.
- Moteur : modules `clock` (`Hns`, conversions, testé), `config` (`EngineConfig` + validation,
  testé), `events` (contrat `EngineEvent`), `error`, `replay` (`PacketRing` purgé par âge +
  estimation RAM, testés). Les autres modules n'existent pas encore : leur cahier des charges
  est dans `docs/ARCHITECTURE.md` § 2.
- Code sans commentaires ; conventions de commentaires, commits et branches dans `CLAUDE.md` § 4.
- CLI : `palaclips-cli info`, `palaclips-cli estimate`.
- App Tauri 2.12 minimale : une fenêtre, commandes `engine_version` et `default_config`.
- Front : Vite 8 + React 19 + TS 5.9 + Tailwind 4, écran provisoire, `src/lib/ipc.ts` (miroir
  des types Rust).
- Scripts : `fetch-ffmpeg.ps1` (BtbN LGPL partagée n8.1), `check-env.ps1`.
- CI Windows : fmt, clippy `-D warnings`, tests, build front.
- Docs : SPEC, ARCHITECTURE, PLAN, DECISIONS, RESEARCH, TESTING, DESIGN-BRIEF.

Vérifié à la création (sous Linux, hors code Windows) : `cargo fmt --check` OK,
`cargo clippy --workspace --all-targets -- -D warnings` OK (app Tauri comprise), `cargo test`
OK (17 tests), `npm run build` OK, génération des icônes depuis le SVG OK.
**Jamais compilé sous Windows** : c'est le premier point de la phase 0.

## Prochaines actions (dans l'ordre)

1. Phase 0 de `docs/PLAN.md`, en commençant par `pwsh scripts/check-env.ps1`.
2. Icônes (générées depuis `app-icon.svg`), `Cargo.lock` et `app/package-lock.json` sont
   commités : rien à régénérer pour compiler.
3. Vérifier la liaison rsmpeg 0.18 ↔ FFmpeg 8.1 (ADR-004) avant d'écrire du code d'encodage.

## Bloquants

Aucun.

## Points d'attention

- `rsmpeg` avance lentement (dernière version : août 2025, FFmpeg 8.0) : voir ADR-004 plan B.
- La capture audio par appli a un rapport de piste muette sur Windows 11 build 26200 (avec
  NAudio, cause non établie) : à tester dès la phase 3 sur la machine de Palawi.
- Tauri 3 est en alpha sur crates.io : ne pas suivre les suggestions de mise à jour vers 3.x.

## Journal des sessions

| Date | Phase | Fait | Suite |
|---|---|---|---|
| 2026-10-15 | — | Création du repo, docs, squelette, CI | Phase 0 |
