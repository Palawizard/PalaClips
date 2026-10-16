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

Plateforme : **Windows 10 build 20348+ et Windows 11**, x64. GPU avec encodeur matériel
(NVIDIA NVENC, AMD AMF ou Intel QSV).

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
| ENC-05 | Sans encodeur matériel : message clair au démarrage expliquant pourquoi l'app ne peut pas capturer. |

### 2.3 Audio (AUD)

| ID | Exigence |
|---|---|
| AUD-01 | Piste **Mix complet** (tout ce que l'on entend), toujours en piste 1 si activée. |
| AUD-02 | Piste **Micro** : choix du périphérique, gain (−20 à +20 dB), suivi du micro par défaut. |
| AUD-03 | Une **piste par logiciel** choisi (jusqu'à 6 par défaut, réglable jusqu'à 12), capture par arbre de processus (PID parent + enfants). |
| AUD-04 | Sélecteur de sources audio soigné : applis ayant du son en tête, **icône, nom, vumètre en direct**, recherche, ajout d'une appli **non lancée** (liste des processus ou parcourir un .exe). |
| AUD-05 | Chaque piste est renommable ; le nom est écrit dans les métadonnées du fichier (visible dans Resolve). |
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
| RPL-03 | **Estimation de la RAM** affichée en direct pendant le réglage (débit × durée + audio). |
| RPL-04 | Stockage du buffer en **RAM** (défaut) ou sur **disque** (longues durées). |
| RPL-05 | Sauvegarde par raccourci : fichier prêt en **< 500 ms** pour 60 s à 40 Mb/s, début sur image clé, pistes alignées. |
| RPL-06 | La sauvegarde ne vole **jamais** le focus du jeu ; confirmation par toast Windows + son (désactivable). |
| RPL-07 | Changer de sources ou de zone redémarre le buffer ; l'UI le dit avant d'appliquer. |
| RPL-08 | RAM stable sur 2 h de replay armé (pas de fuite). |

### 2.5 Enregistrement manuel (REC)

| ID | Exigence |
|---|---|
| REC-01 | Démarrer/arrêter par raccourci et par la zone de notification ; icône de notification différente pendant l'enregistrement. |
| REC-02 | Partage la capture et l'encodeur du replay (pas de double coût). |
| REC-03 | Fichier lisible jusqu'au dernier fragment après un crash ou une coupure de courant (MP4 hybride). |
| REC-04 | Option « inclure le replay en cours au début de l'enregistrement ». |
| REC-05 | Découpage automatique optionnel par durée ou par taille. |
| REC-06 | Récupération au démarrage d'un enregistrement interrompu : il apparaît dans la galerie, finalisé. |

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
| APP-06 | **Premier lancement guidé** : vérifications (Windows, encodeur, WebView2, dossier), choix des sources, du replay et des raccourcis, test de sauvegarde. |
| APP-07 | Réglages persistants, versionnés avec migration, bouton « Réinitialiser ». |
| APP-08 | Interface en **français et en anglais** (langue du système par défaut). |
| APP-09 | Journaux avec rotation ; **Exporter un diagnostic** (zip : logs, réglages anonymisés, infos système). |
| APP-10 | **Mises à jour automatiques** depuis GitHub Releases (signature Tauri vérifiée), avec notes de version. |
| APP-11 | Installeur Windows (NSIS, par utilisateur, sans droits admin) ; la désinstallation conserve les clips. |
| APP-12 | Panneau de performances : RAM/CPU du moteur, taille du buffer, images perdues, encodeur utilisé. |
| APP-13 | Accessibilité : navigation clavier complète, contrastes conformes, `prefers-reduced-motion` respecté. |

### 2.9 Performances (PERF) — mesurées sur la version installée, en release

| ID | Exigence |
|---|---|
| PERF-01 | Zone de notification, replay désarmé : **< 30 Mo** de RAM, 0 % CPU au repos. |
| PERF-02 | Replay armé : RAM du moteur hors buffer **< 150 Mo** ; taille du buffer affichée. |
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
