# Phase 4 — Replay

- Date : 2026-10-16
- Branche : `feature/replay-engine`
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER pilote 617.42,
  Intel Core i5-13600KF ; sortie par défaut « Voicemeeter Input »

**Statut : en cours** — 4 critères sur 4 vérifiés (mémoire : ADR-020 corrigé ; Resolve :
noms affichés hors périmètre, ADR-021) ; validation par `/valider-phase`.

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| Sauvegarde < 500 ms (60 s, 40 Mb/s, 4 pistes) sur 20 sauvegardes | 🤖 | ✅ | `pwsh scripts\replay-save.ps1` : fenêtre de bruit ffplay mise à l'échelle en 1440p 60 i/s, 40 Mb/s, pistes mix / tone-a / tone-b / Jeu ; 20 clips de 290 Mo ; latence (commande → fichier fermé) **min 146 ms, moy 158 ms, max 200 ms** (`docs/reports/phase-4/replay-save-20.txt`) |
| Durée du clip = durée demandée ± 1 intervalle d'images clés ; pistes de même durée | 🤖 | ✅ | Même test : vidéo 60,250 à 60,267 s (intervalle 1 s), écart entre les 5 flux ≤ 16 ms, tous à `start_time = 0` ; 20/20 OK. Tests `plan_clip` (début sur image clé, audio sur le paquet qui couvre l'image clé, fin là où toutes les pistes ont des données) |
| RAM stable (ADR-020 corrigé) : 30 min armé, une sauvegarde toutes les 5 s, mise en régime de 10 min exclue ; sur 10–30 min, pente < 20 Mo/h et < 0,1 Mo par sauvegarde | 🤖 | ✅ | 2026-10-16, `pwsh scripts\replay-ram.ps1` : 360 clips sur 360 (latence moy. 156 ms, max 182 ms) ; 241 mesures sur 10–30 min : **−0,002 ± 0,001 Mo par sauvegarde**, **−1,8 ± 0,8 Mo/h** ; mémoire privée 649–652 Mo de 10 à 30 min (mise en régime 634 → 652 Mo pendant les 10 premières minutes) ; journal `docs/reports/phase-4/replay-ram-30min.txt` |
| Premier essai, critère d'origine (20 min, régression dès 2 min) | 🤖 | ❌ | 2026-10-16, `pwsh scripts\replay-ram.ps1` : 240 clips sur 240 (latence moy. 154 ms, max 175 ms) ; à partir de 120 s, **+0,045 ± 0,003 Mo par sauvegarde** ✅ mais **+32,0 ± 2,1 Mo/h** ❌. Courbe qui sature : 626 → 642 Mo de 2 à 9 min (~+119 Mo/h), puis plate de 10 à 20 min (640–644 Mo, −9 Mo/h). Le critère incluait la mise en régime : erreur de conception du test, corrigée par Palawi (ADR-020) ; journal `docs/reports/phase-4/replay-ram-20min.txt` |
| Import dans DaVinci Resolve : pistes séparées et synchronisées (STO-01, ADR-021) | 👤 | ✅ | Palawi, 2026-10-16, clip de 20 s (`palaclips-cli replay`, fenêtre ffplay `testsrc2`, pistes Mix complet / tone-a 440 Hz / tone-b 880 Hz / Jeu) : 4 pistes séparées, contenu attendu, alignées sur la vidéo ✅ ; **noms non affichés** (« Audio 1 » à « Audio 4 ») alors qu'ils sont dans `hdlr` et `udta/name` ; même résultat en MOV ; Palawi renonce à l'affichage des noms dans Resolve (ADR-021) |

## Tâches

| Tâche | Statut | Preuve |
|---|---|---|
| `Engine` / `EngineHandle`, fan-out | ✅ | `engine.rs` : un fil moteur (commandes + paquets + alertes), paquets partagés par `av_packet_clone` ; tests d'ordre des pistes et de redémarrage |
| `PacketRing` sur tous les flux, RAM et disque | ✅ | `replay/store.rs` (`ReplayPacket`, `Spool` en segments de 4 s, `PacketReader`) + 3 tests ; essai `--disk` : clips identiques, prêts en 10 ms, aucun segment restant |
| Sauvegarde : image clé, alignement audio, fil dédié, nom par jeu | ✅ | `plan_clip` + 5 tests ; `palaclips-replay-save` ; `paths` (`Vidéos\PalaClips\<Jeu>\<Jeu>_<date>_replay.mp4`, « Bureau » sans jeu, suffixe si même seconde) + 4 tests ; clips nommés `ffplay_…` (cible fenêtre) et `Bureau_…` (écran sans jeu) |
| Redémarrage du tampon quand sources/zone changent | ✅ | `configure` → `needs_restart` ; essai `r` dans `palaclips-cli replay` : pistes 2 → 1, réarmé en 650 ms, clips suivants corrects |
| CLI `replay --duration 60` | ✅ | Entrée = sauvegarder, `r` = changer les sources, `q` = quitter ; mémoire privée et tampon toutes les 5 s, latence par clip, résumé min/moy/max |

## Exigences SPEC couvertes

RPL-02 (durée 5 s à 60 min), RPL-04 (RAM ou disque), RPL-05 (sauvegarde < 500 ms, image clé,
pistes alignées), RPL-07 (redémarrage moteur ; message dans l'UI en phase 5), STO-03 (nom).
STO-01 (pistes séparées et synchronisées dans Resolve), RPL-08 (test accéléré, ADR-020).

## Écarts et corrections

- **Audio en retard sur la vidéo** (~150 ms : comblement, AAC, décalage de synchro) : au
  premier essai, pistes audio qui commençaient à +0,18 s et finissaient plus tôt. Fenêtre des
  tampons = durée + intervalle d'images clés + 2 s, et fin du clip là où toutes les pistes ont
  des données.
- **Deuxième capture dans le même processus** : crash (violation d'accès, `WinTypes.dll`
  déchargée sous une fabrique WinRT gardée en cache par windows-rs) que le gestionnaire de
  crash de TranslucentFlyouts, injecté chez Palawi, transformait en boîte de dialogue, d'où un
  faux « blocage ». Analyse des minidumps avec `cdb` ; correctif `CoIncrementMTAUsage`
  (`409355c`, `RESEARCH.md`).
- **Scripts** : premier essai de `replay-save.ps1` sans mesure (fenêtre ffplay pas encore
  ouverte, puis titre avec espaces coupé par `Start-Process`) ; le script attend maintenant la
  fenêtre, la CLI s'arrête si l'armement échoue, un clip manquant compte comme un échec.
- Mémoire privée pendant les 20 sauvegardes : 612 → 633 Mo (~1 Mo par sauvegarde) ; à suivre
  dans le test de 2 h (qui ne sauvegarde qu'à la fin).
