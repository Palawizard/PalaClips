# Phase 7 — Robustesse et performances

- Mesures sur la build installée : 2026-10-19, `dev` après #59 + `test/isolated-migration`
- Machine : Windows 11 build 26200, NVIDIA GeForce RTX 4070 SUPER, Intel Core i5-13600KF
  (20 cœurs logiques), écran principal 2560×1440 240 Hz

**Statut : en cours.** PERF-01 à 04 mesurés et tenus (PERF-03 GPU-bound en priorité « jeu »,
défaut) ; séance T7 du 2026-10-19 et ses trois défauts corrigés le 2026-10-20 (#64, #65).
Changement de résolution fait le même jour. Essai de la priorité « capture » sans HAGS jugé
inutile par Palawi (accordée sans admin avec HAGS ; sans HAGS, un refus est seulement journalisé).

## Critères

| Critère | Type | Résultat | Preuve |
|---|---|---|---|
| PERF-01 : zone de notification, replay désarmé, < 30 Mo, 0 % CPU | 🤖 | ✅ | Build installée (`setup.exe /S`), réglages isolés, `--autostart` : 4,9 Mo (working set privé, médiane sur 60 s), CPU 0,00 % de la machine, 1 seul processus (aucun WebView). `phase-7/perf-installed-2026-10-19.txt` |
| PERF-02 : replay armé, moteur hors tampon < 150 Mo | 🤖 | ✅ | Écran principal 2560×1440 à 60 i/s, 3 pistes, tampon 30 s : 243,4 Mo privés − tampon 164,7 Mo × 1,07 = **67,3 Mo** (même calcul que le panneau de performances) |
| PERF-03 : < 2 % CPU (1440p60 NVENC) | 🤖 | ✅ (CPU) | Replay armé, fenêtre fermée : 0,39 % de la machine |
| PERF-03 : perte de fps ≤ 2 %, GPU-bound, 1440p60 + 4 pistes | 👤 | ✅ (priorité « jeu », défaut) | 2026-10-20, après le correctif du défaut 1, Kombustor fenêtré (GPU 100 %, ~84 i/s), essais sans capture intercalés : **0,7 %** en priorité « jeu » (défaut, ADR-033 révisé) ; 7,5 % en priorité « capture » (temps réel, réglage au choix). `phase-7/fps-impact-2026-10-20_17-35.txt` (capture) et `_17-43.txt` (jeu). Historique du 2026-10-19 : | Fortnite reste limité par le CPU de Palawi (GPU sous 99 %) → banc GPU. MSI Kombustor (Vulkan) plein écran exclusif : perte 0,6 % mais capture sans images (`captured=2`, mesure invalide). Kombustor fenêtré (« Composed: Copy with GPU GDI ») : 3,7 % (1440p60 + 4 pistes), 3,0–3,5 % (60 i/s), 1,5 % (WGC seul), mais dérive thermique de 1,8 % entre le début et la fin. Unigine Superposition DX11 : mode de présentation ancien lui aussi, bruit ±5 % à 46 i/s, et **capture bloquée sous GPU à 100 %** (`max_lateness_ms=14357`, 1 132 images sur 1 560) : défaut 1. Journaux `phase-7/fps-impact-2026-10-19_20-*.txt`. `fps-impact.ps1` intercale désormais un essai sans capture entre chaque essai avec capture (référence = moyenne des deux voisins) |
| PERF-04 : démarrage de l'UI < 1 s à chaud | 🤖 | ✅ | Relance (instance unique) → premier rendu avec l'état : première 652 ms, puis 534, 538, 563 ms à chaud |
| Alerte G-Sync (CAP-11) | 👤 | ✅ | App isolée sur l'écran de gauche, capture de la fenêtre Fortnite à 240 i/s. G-Sync actif, autre écran à 165 Hz : `capture_source_rate_limited { rate: 165, fps: 240 }`. G-Sync coupé, jeu plafonné à 120 i/s, app relancée : aucune alerte en 30 s |
| T7 casque débranché / périphérique par défaut changé (AUD-10) | 👤 | ✅ | Sortie et entrée par défaut (Voicemeeter, VB-Cable) changées puis remises, casque débranché : `audio_device_changed` (Micro revenu sur CABLE Output) ; replay de 60 s couvrant les changements : vidéo et 3 pistes à 60,9 s, aucune coupure |
| T7 plein écran (Fortnite) | 👤 | ✅ | Essais G-Sync et casque faits en plein écran Fortnite (pas un vrai exclusif) : clip de 60 s sans aucune image figée ≥ 2 s (`freezedetect`) |
| T7 plein écran exclusif (« Hardware: Legacy Flip ») | 👤 | ✅ | 2026-10-20 (#65) : Kombustor exclusif, 2 images reçues en 60 s, propriété exclusive vraie les 60 s, `capture_exclusive_fullscreen` levée après 5 s (`phase-7/exclusive-2026-10-20_17-17.txt`). Le 2026-10-19 : vidéo figée sans alerte (défaut 2) |
| T7 veille / sortie de veille | 👤 | ✅ | Replay armé (écran principal), veille ~30 s : au réveil replay toujours armé, aucune alerte ; clip sauvegardé une minute après le réveil, mire animée visible sur ses 13 dernières secondes, pistes complètes |
| T7 redémarrage du pilote GPU (TDR) | 👤 | ✅ | `dxcap -forcetdr` après fermeture du navigateur, de Discord et du jeu : alerte `encoder_reset { reason: video_capture_stopped }`, réarmement automatique (tampon reparti de zéro, ADR-031) ; clip suivant animé, 4 flux complets (30,9 s) |
| T7 appli capturée qui crashe | 🤖 | ✅ | `scripts/app-crash.ps1` (2026-10-20) : processus de test tué à 8 s puis relancé à 16 s pendant un replay ; piste du clip son 11 s → silence 7 s → son 12 s, rattachement au nouveau PID, replay jamais arrêté (`phase-7/app-crash-2026-10-20_16-51.txt`) |
| T7 GPU saturé (défaut 1) | 🤖 | ✅ | #64 : charge GPU au premier plan, retard max 11 213 ms → 57 ms (priorité « jeu ») / 63 ms (« capture »), 1 200 images CFR sur 20 s, 0 erreur de décodage ; `capture_gpu_saturated` levée en priorité « jeu », muette en « capture » (`phase-7/gpu-*.txt`) |
| T7 alerte G-Sync retirée au redémarrage (défaut 3) | 🤖 | ✅ | #64 : alertes de capture retirées au démarrage et à l'arrêt de la capture, et après 5 s de retour à la normale (`alert_cleared`) ; tests unitaires du moteur et du shell |
| T7 changement de résolution | 👤 | ✅ | 2026-10-20 : replay armé sur l'écran principal, Palawi passe l'écran en 1920×1080 puis Windows revient seul en 2560×1440 : deux « changements de résolution de la source », convertisseur recréé, replay jamais arrêté, aucune alerte ; clip de 44,6 s en 640×360 constant, aucune image figée ≥ 2 s (`freezedetect`), 0 erreur de décodage (`phase-7/resolution-2026-10-20_17-59.txt`). Mémoire privée +30 Mo après le changement, gardée au retour (à surveiller) |

## Défauts trouvés en séance T7 (corrigés le 2026-10-20 : #64 pour 1 et 3, #65 pour 2 ; ADR-033)

1. **Capture bloquée sous GPU saturé** : avec un jeu qui prend 100 % du GPU, la conversion
   d'image de PalaClips (même GPU) n'obtient plus son tour ; l'encodeur a pris 14 s de retard
   et la vidéo saute. Piste : priorité GPU des travaux de capture
   (`IDXGIDevice::SetGPUThreadPriority`), à vérifier d'abord par une sous-commande CLI. PERF-03
   GPU-bound à remesurer ensuite.
2. **Plein écran exclusif sans alerte** : en « Legacy Flip », Windows ne compose plus l'écran
   et WGC ne livre plus d'image ; il faut une alerte claire (« passe le jeu en plein écran
   fenêtré »). Piste : `D3DKMTCheckExclusiveOwnership` quand la capture ne reçoit plus rien.
3. **Alerte CAP-11 qui reste affichée** après le redémarrage de la capture alors que la cadence
   est redevenue normale : effacer les alertes liées à la capture quand elle redémarre.

## Calibrage du micro (2026-10-19)

Fait par Palawi dans l'app (écouteur contre le micro) : Mix complet −106 ms (excédent
Voicemeeter habituel), Micro −485 ms, Jeu 0 ms. La piste 1 attend donc le micro jusqu'à
635 ms (ADR-029). Premier lancement de la nouvelle build avec ses vrais réglages : migration
ADR-032 faite (réglages et bibliothèque dans `%APPDATA%\com.palawi.palaclips`, cache et
replay dans `%LOCALAPPDATA%\com.palawi.palaclips`).

## Installation et désinstallation

- Installation silencieuse par utilisateur : 7 DLL FFmpeg, 4 licences, entrée de
  désinstallation ; aucun processus lancé, aucune entrée « Démarrer avec Windows ».
- Désinstallation sans la case : programme, entrée et raccourci retirés ;
  `%LOCALAPPDATA%\PalaClips` ne garde que `cache` et `replay` de Palawi ; ses 583 fichiers de
  données (`%APPDATA%\PalaClips`, `cache`, `replay`, dossiers `com.palawi.palaclips`) identiques
  en SHA-256 avant et après (`scripts/real-data-fingerprint.ps1`) ; dossier des clips inchangé.
- Migration en mode isolé : jamais (`an_isolated_app_never_migrates_the_real_data_folders`), et
  les empreintes ci-dessus le confirment sur la vraie machine.
- Migration avec l'app installée présente (#61) : sur une copie de la vraie installation, les
  13 fichiers du programme restent en place et identiques, les 5 fichiers de données migrent à
  l'identique ; installation réelle et entrée de désinstallation inchangées.
- Désinstallation avec la case (APP-11), faite par Palawi depuis Paramètres > Applications :
  sauvegarde vérifiée avant (`uninstall-check.ps1 -Step Backup`, 578 fichiers de
  `%LOCALAPPDATA%\com.palawi.palaclips`, pas de dossier Roaming de l'identifiant), clip témoin
  copié dans `F:\Fast Documents\Vidéos\PalaClips`. Après : programme, entrée et raccourci
  retirés ; les deux dossiers `com.palawi.palaclips` supprimés ; `%LOCALAPPDATA%\PalaClips` ne
  garde que `cache` et `replay` ; clip témoin intact (SHA-256 `E14A67BB…` identique), puis
  supprimé. Données restaurées, identiques en SHA-256 ; empreinte complète des 583 fichiers de
  données de Palawi identique à celle d'avant la première installation.
