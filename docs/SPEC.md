# SPEC — PalaClips V1

> Ce document définit **tout** ce que contient la V1 (`v1.0.0`). Rien de cette liste n'est
> reportable (voir `CLAUDE.md` § 0). Chaque exigence a un identifiant (`CAP-01`...) repris
> dans `docs/PLAN.md` et dans les rapports de validation.

## 1. Pour qui, pour quoi

Un joueur / créateur de contenu sur Windows qui veut :
- sauvegarder les dernières secondes de jeu d'un raccourci, sans y penser ;
- enregistrer des sessions entières ;
- récupérer **chaque logiciel sur sa propre piste audio** (jeu, Discord, musique, navigateur,
  micro) pour le montage dans DaVinci Resolve ;
- le tout avec un outil **léger**, gratuit, open source, sans compte ni télémétrie.

Plateforme : **Windows 11** (et Windows Server 2022, build 20348 ou plus récent), x64 : la
capture audio par processus exige le build 20348, que Windows 10 grand public (19045 au plus)
n'atteint pas. GPU avec encodeur matériel conseillé (NVIDIA NVENC, AMD AMF ou Intel QSV) ;
sans lui, encodage par le processeur (ENC-06, v1.1).

## 2. Exigences

### 2.1 Capture vidéo (CAP)

| ID | Exigence |
|---|---|
| CAP-01 | Capture d'un **écran** au choix ou d'une **fenêtre** au choix, via Windows Graphics Capture. |
| CAP-02 | Sélecteur visuel : écrans et fenêtres avec **miniatures en direct**, nom de l'appli et icône. |
| CAP-03 | Cible mémorisée par nom de périphérique (écran) ou nom d'exe (fenêtre), retrouvée au redémarrage. |
| CAP-04 | Repli automatique sur l'écran de la fenêtre si la capture de fenêtre échoue, avec message. |
| CAP-05 | Sortie en **fps constant** au choix : 30, 60, 120, 144, 165, 240 ou valeur libre (10–240). |
| CAP-06 | Résolution de sortie : native ou mise à l'échelle (720p, 1080p, 1440p, 2160p ou libre), sur le GPU. |
| CAP-07 | Curseur affiché ou non ; bordure jaune Windows affichée ou non. |
| CAP-08 | Multi-écrans et mise à l'échelle Windows (100–200 %) gérés ; changement de résolution à chaud géré. |
| CAP-09 | Écran HDR : la vidéo SDR produite a des couleurs correctes (pas délavées ni sombres). |
| CAP-10 | Aucune injection dans le processus capturé ; compatible Easy Anti-Cheat / BattlEye / Vanguard (capture via API Windows uniquement). |
| CAP-11 | Alerte quand l'écran capturé est composé par Windows (cadence DWM) moins vite que le fps demandé, cas typique de G-Sync avec des écrans à fréquences différentes : message en français qui donne la cadence réelle et la marche à suivre. Un jeu plafonné sous le fps demandé ne la déclenche pas. |

### 2.2 Encodage (ENC)

| ID | Exigence |
|---|---|
| ENC-01 | Encodage matériel H.264, HEVC et AV1 selon ce que le GPU supporte (NVENC, AMF, QSV), détecté par un **encodage de test réel**. |
| ENC-02 | Débit réglable (1–200 Mb/s), préréglages « Qualité / Équilibré / Léger » + réglage libre. |
| ENC-03 | Intervalle d'images clés réglable (défaut 1 s) pour un replay précis. |
| ENC-04 | Aucune copie CPU des images dans le chemin nominal (texture D3D11 → encodeur). |
| ENC-05 | Sans encodeur matériel : le premier lancement et l'app disent que le processeur encodera, avec un avertissement sur la charge en jeu ; message clair au démarrage seulement si aucun encodeur, ni matériel ni processeur, ne fonctionne (v1.1). |
| ENC-06 | **Encodage par le processeur** (v1.1, ADR-035) : H.264 par l'encodeur logiciel de Windows et AV1 par SVT-AV1, détectés par un encodage de test réel ; réglage « Encodeur » (Automatique = matériel si disponible sinon processeur, Carte graphique, Processeur) ; fils d'encodage en priorité basse et en nombre limité ; même contrat que le matériel (CFR, images clés réglables, file bornée, replay, enregistrement, découpe précise et export) ; 1080p30 par défaut sur le processeur ; alerte quand l'encodage ne suit pas. |

### 2.3 Audio (AUD)

| ID | Exigence |
|---|---|
| AUD-01 | Piste **Mix complet**, toujours en piste 1 si activée : somme des pistes enregistrées et choisies (micro si « Aussi dans Mix complet », Jeu, applis), ou, avec le réglage « Audio du bureau » (désactivé par défaut), tout le son du PC plus le micro. Source au choix (v1.3.1) : « Tous les sons sauf PalaClips » (par défaut, sons de PalaClips exclus, toutes les sorties) ou une sortie précise (ses sons de PalaClips compris). Les pistes suivantes restent séparées pour le montage. |
| AUD-02 | Piste **Micro** : choix du périphérique, gain (−20 à +20 dB), suivi du micro par défaut. |
| AUD-03 | Une **piste par logiciel** choisi, sans maximum (v1.3 : au-delà de 12 pistes d'applis, une note dit ce qu'elles coûtent en processeur, mémoire et Mo par minute), capture par arbre de processus (PID parent + enfants). |
| AUD-04 | Sélecteur de sources audio soigné : applis ayant du son en tête, **icône, nom, vumètre en direct**, recherche, ajout d'une appli **non lancée** (liste des processus ou parcourir un .exe). |
| AUD-05 | Chaque piste est renommable ; le nom est écrit dans les métadonnées du fichier (lu par VLC, ffprobe… ; DaVinci Resolve ne l'affiche pas, ADR-021). |
| AUD-06 | Option « Piste Jeu automatique » : une piste dédiée suit **le jeu**, pas le premier plan. Capture d'une fenêtre → l'appli de cette fenêtre. Capture d'un écran → la dernière appli passée au premier plan en plein écran ou en fenêtre sans bordure couvrant un écran ; un Alt+Tab vers une appli fenêtrée (Discord, navigateur, terminal…), le bureau ou PalaClips ne la change pas. L'utilisateur peut aussi la fixer sur un processus choisi dans le sélecteur AUD-04. |
| AUD-07 | Appli fermée puis relancée : la piste se rattache seule, silence entre-temps, aucun fichier cassé. |
| AUD-08 | Synchronisation audio/vidéo < 1 image sur **1 h**, y compris avec de longs silences. |
| AUD-09 | Alerte si une piste reste muette alors que l'appli émet du son. |
| AUD-10 | Changement de périphérique de sortie/entrée par défaut (casque branché/débranché) suivi à chaud. |
| AUD-11 | **Décalage de synchro par piste** (−1000 à +1000 ms, positif = son retardé) pour compenser les périphériques virtuels (Voicemeeter…), avec un **assistant de calibrage** : flash + bip émis comme par un jeu, capturés et mesurés, valeur proposée pour chaque piste. |

### 2.4 Replay instantané (RPL)

| ID | Exigence |
|---|---|
| RPL-01 | Replay armé au démarrage (option), armé/désarmé par raccourci et par le menu de la zone de notification. |
| RPL-02 | Durée réglable de 5 s à 60 min. |
| RPL-03 | **Estimation de la RAM** affichée en direct pendant le réglage ((débit vidéo + audio) × fenêtre du tampon, + 7 % de surcharge mesurée). |
| RPL-11 | **Estimation de la taille** (v1.3), à jour pendant le réglage : Mo par minute, taille d'un clip à la durée du replay et avec l'« après » ; pour l'enregistrement, Mo par minute et par heure. Débit vidéo effectif (résolution et fps réels de la source) + AAC × pistes enregistrées + conteneur ; encodage en CBR, donc un seul chiffre « ≈ », à ±10 % des vrais clips ; unités binaires comme la galerie. |
| RPL-04 | Stockage du buffer en **RAM** (défaut) ou sur **disque** (longues durées). |
| RPL-05 | Sauvegarde par raccourci : fichier prêt en **< 500 ms** pour 60 s à 40 Mb/s, début sur image clé, pistes alignées. |
| RPL-06 | La sauvegarde ne vole **jamais** le focus du jeu ; confirmation par toast Windows + son (désactivable). |
| RPL-07 | Changer de sources ou de zone redémarre le buffer ; l'UI le dit avant d'appliquer. |
| RPL-08 | RAM stable en replay armé : pas de croissance avec le temps ni avec le nombre de sauvegardes. |
| RPL-09 | Deux sauvegardes successives ne se répètent pas : le clip suivant commence où le précédent s'est arrêté (au plus un intervalle d'images clés en commun) ; « En mémoire » repart de zéro après chaque sauvegarde. Moins de 3 s en mémoire : pas de clip, message discret dans l'app (ni notification ni son en jeu). |
| RPL-10 | **Durée « après »** (v1.2) : 0 s par défaut, préréglages 10 s, 30 s, 1 min, 5 min ou valeur libre jusqu'à 10 min, appliquée au raccourci de sauvegarde. L'avant est écrit tout de suite dans un MP4 hybride, puis le direct y est ajouté jusqu'à la fin de la durée (aucune RAM en plus, fichier récupérable après un arrêt brutal). Carillon et notification « Clip en cours, +X s » à l'appui, « Clip sauvegardé » à la fin ; un nouvel appui prolonge le même clip ; « Terminer maintenant » sur État et dans la zone de notification. Pas de trou ni d'image en double à la jonction, synchro A/V au niveau des clips actuels. |

### 2.5 Enregistrement manuel (REC)

| ID | Exigence |
|---|---|
| REC-01 | Démarrer/arrêter par raccourci et par la zone de notification ; icône de notification différente pendant l'enregistrement ; son au début et à la fin (v1.3, distinct du carillon de sauvegarde, avec le réglage « Son de confirmation »). |
| REC-02 | Partage la capture du replay ; partage aussi son encodeur quand les deux qualités sont identiques (pas de double coût). |
| REC-03 | Fichier lisible jusqu'au dernier fragment après un crash ou une coupure de courant (MP4 hybride). |
| REC-04 | Option « inclure le replay en cours au début de l'enregistrement » ; si la qualité de l'enregistrement diffère de celle du replay (v1.3), le replay en mémoire est sauvegardé en clip à part juste avant. |
| REC-05 | Découpage automatique optionnel par durée ou par taille. |
| REC-06 | Récupération au démarrage d'un enregistrement interrompu : il apparaît dans la galerie, finalisé. |
| REC-07 | **Qualité propre de l'enregistrement** (v1.3, ADR-040) : « Comme le replay » par défaut, ou codec, qualité/débit, images par seconde et résolution propres ; source, audio, encodeur, images clés et priorité GPU communs. Un 2e encodage tourne seulement pendant l'enregistrement, sans coupure du replay ; les sauvegardes de replay restent possibles pendant l'enregistrement ; 2e encodage matériel refusé → processeur, avec une alerte. |

### 2.6 Fichiers et stockage (STO)

| ID | Exigence |
|---|---|
| STO-01 | MP4, vidéo H.264/HEVC/AV1, audio AAC 48 kHz stéréo, une piste par source, **CFR** — importable tel quel dans DaVinci Resolve. |
| STO-02 | Dossier des clips réglable (défaut `Vidéos\PalaClips`), sous-dossier par jeu/appli. |
| STO-03 | Nom de fichier : `<Jeu>_<AAAA-MM-JJ_HH-MM-SS>[_replay].mp4`. |
| STO-04 | Quota disque optionnel : suppression des plus anciens clips **non favoris** au-delà du quota. |
| STO-05 | Vérification d'espace avant écriture ; alerte et arrêt propre si le disque est plein. |

### 2.7 Galerie et lecteur (GAL)

| ID | Exigence |
|---|---|
| GAL-01 | Grille de clips avec miniature, durée, jeu, date, taille, type (replay / enregistrement). |
| GAL-02 | Filtres (jeu, type, favoris, période), tri, recherche par nom ; fluide avec 5 000 clips. |
| GAL-03 | Lecteur intégré avec déplacement fluide, y compris sur un fichier de 4 Go. |
| GAL-04 | Dans le lecteur : **chaque piste audio** peut être coupée, isolée (solo) ou réglée en volume. |
| GAL-05 | Renommer, favori, supprimer (vers la Corbeille), ouvrir l'emplacement, **copier le fichier** dans le presse-papiers (glisser dans Discord). |
| GAL-06 | Découpe : export **rapide** sans réencodage (sur images clés) ou **précis** (réencodage matériel). |
| GAL-07 | Export : choix des pistes à garder, ou **mixage en une seule piste** (partage Discord/réseaux). |
| GAL-08 | Les clips ajoutés hors de l'app dans le dossier apparaissent automatiquement. |

### 2.8 Application (APP)

| ID | Exigence |
|---|---|
| APP-01 | Vit dans la **zone de notification** ; menu : armer/désarmer, sauvegarder le replay, démarrer/arrêter l'enregistrement, ouvrir, quitter. |
| APP-02 | Fenêtre créée à la demande et **détruite** à la fermeture (un seul processus natif ensuite). |
| APP-03 | **Instance unique** : relancer l'app ouvre la fenêtre existante. |
| APP-04 | **Démarrage avec Windows** (option), directement en zone de notification. |
| APP-05 | Raccourcis globaux **personnalisables**, avec détection des conflits et touches réservées refusées. |
| APP-06 | **Premier lancement guidé** : vérifications (Windows, encodeur, WebView2, dossier), choix des sources (dont le micro, « Aussi dans Mix complet » et « Audio du bureau »), du dossier des clips, du replay et des raccourcis, test de sauvegarde. |
| APP-07 | Réglages persistants, versionnés avec migration, bouton « Réinitialiser ». |
| APP-08 | Interface en **français et en anglais** (langue du système par défaut). |
| APP-09 | Journaux avec rotation ; **Exporter un diagnostic** (zip : logs, réglages anonymisés, infos système). |
| APP-10 | **Mises à jour automatiques** depuis GitHub Releases (signature Tauri vérifiée), avec notes de version. |
| APP-11 | Installeur Windows (NSIS, par utilisateur, sans droits admin) ; la désinstallation conserve les clips. |
| APP-12 | Panneau de performances : RAM/CPU du moteur, taille du buffer, images perdues, encodeur utilisé. |
| APP-13 | Accessibilité : navigation clavier complète, contrastes conformes, `prefers-reduced-motion` respecté. |
| APP-15 | Accès direct (v1.3) : sur État, le résumé de la qualité (« 1440p60 · H.264 · ≈ X Mo/min », et celui de l'enregistrement s'il diffère) est un lien ; sur Capture, un bouton « Replay et qualité ». Les deux ouvrent Réglages → Replay & qualité (défilement et focus, via la garde de sortie). |
| APP-14 | Barre latérale (v1.2) : État, Galerie, Capture, Audio, Compression, Réglages ; Capture et Audio sont des pages à part entière. Quitter une page (ou fermer la fenêtre) avec des changements non appliqués ouvre une alerte « Appliquer / Ne pas appliquer / Rester » qui dit si appliquer redémarre le replay. |

### 2.8 bis Compression (CMP, v1.2)

| ID | Exigence |
|---|---|
| CMP-01 | Page **Compression** dans la barre latérale, bouton « Compresser » dans la galerie et le lecteur (sélection découpée comprise), glisser-déposer depuis l'Explorateur (mp4, mkv, mov…) avec un message clair si un fichier n'est pas lisible ; « Choisir des clips » (un ou plusieurs clips de la galerie) ; file de plusieurs fichiers. |
| CMP-02 | Cible : 20 Mo (Discord), 50 Mo (Nitro Basic), 500 Mo (Nitro) ou valeur libre en Mo (1 Mo = 1 000 000 octets) ; le fichier livré n'est **jamais** au-dessus de la cible (taille mesurée, jusqu'à 3 essais). |
| CMP-03 | Débit vidéo = (cible × marge − audio − surcoût du conteneur) / durée ; résolution (1440 → 1080 → 720 → 480) et fps (60 → 30) baissés quand le débit par pixel est trop bas ; aperçu modifiable (résolution, fps, débit, qualité estimée) avant de lancer. |
| CMP-04 | H.264 par défaut, AV1 en option avec mise en garde (lecture mobile) ; encodeur matériel d'abord, repli sur le processeur ; HDR → SDR ; piste 1 seule par défaut, « Garder toutes les pistes » au choix. |
| CMP-05 | Priorité basse, sans gêner le replay ni la partie ; progression, annulation propre sans fichier partiel. Fichier à côté de l'original (`…_20MB.mp4`), dans la galerie et le quota ; original entier déjà sous la cible : copie sans réencodage ; à la fin : taille finale, « Copier » (presse-papiers) et « Ouvrir le dossier ». |

### 2.9 Performances (PERF) — mesurées sur la version installée, en release

| ID | Exigence |
|---|---|
| PERF-01 | Zone de notification, replay désarmé : **< 30 Mo** de RAM, 0 % CPU au repos. |
| PERF-02 | Replay armé : RAM du moteur hors buffer **< 150 Mo** ; taille du buffer affichée. |
| | PERF-01 et PERF-02 : RAM = mémoire privée réellement occupée (*private working set*, ce qu'affiche le Gestionnaire des tâches), mesurée hors pression mémoire ; la mémoire engagée (*commit*) est relevée pour information (ADR-023). |
| PERF-03 | Capture 1440p60 NVENC + 4 pistes : **< 2 % CPU** sur i5 13e gen, perte de fps en jeu ≤ 2 %. |
| PERF-04 | Fenêtre ouverte : démarrage de l'UI **< 1 s** à chaud. |

## 3. Hors périmètre de la V1 (décisions, pas reports)

- Streaming en direct, upload cloud, comptes, partage par lien.
- Éditeur vidéo au-delà de la découpe et de l'export (pas de timeline multi-clips).
- Overlay dans le jeu (incompatible avec la règle « aucune injection »).
- Suppression de bruit du micro, effets audio.
- Capture webcam.
- Linux et macOS.
- Signature Authenticode de l'exécutable (payante) : Windows SmartScreen avertira au premier
  lancement ; c'est documenté dans le README.
