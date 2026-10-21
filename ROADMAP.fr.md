# Feuille de route

[English](ROADMAP.md) · **Français**

La version 1.0.0 est la première version publique : tout ce qu'elle contient est dans le
[README](README.fr.md). Cette page dit ce qui vient ensuite. Rien n'y a de date, et l'ordre peut
changer selon les retours (les [issues](https://github.com/Palawizard/PalaClips/issues) sont
bienvenues).

## 1. Compatibilité testée sur plus de systèmes

Jusqu'ici, PalaClips a été testé sur un PC (Windows 11, NVIDIA RTX 4070 Super, écran 1440p
240 Hz et deux autres écrans) et dans une machine virtuelle Windows 11 sans carte graphique.
Ensuite :

- Cartes graphiques AMD (AMF) et Intel (Quick Sync) sur du vrai matériel : le code les prend en
  charge, mais elles n'ont jamais été essayées hors de la détection automatique.
- PC portables à deux puces graphiques (intégrée et dédiée).
- Autres versions de Windows 11 et Windows Server 2022 ; mise à l'échelle de 100 à 200 %.
- Plus d'anti-triches : testé avec Easy Anti-Cheat (Fortnite) ; BattlEye et Vanguard à essayer.
- Plus de configurations audio : casques Bluetooth, interfaces audio USB, périphériques virtuels.

## 2. Encodage par le processeur (CPU)

Aujourd'hui, PalaClips n'encode que par la carte graphique (NVIDIA NVENC, AMD AMF, Intel Quick
Sync) : un PC sans encodeur matériel ne peut pas enregistrer. Ensuite : un encodeur sur le
processeur, utilisé quand aucun encodeur matériel ne marche, avec un avertissement clair sur la
charge qu'il ajoute en jeu. PalaClips livre FFmpeg sous licence LGPL : les candidats sont des
encodeurs à licence compatible, comme OpenH264 ou SVT-AV1 (x264 et x265 sont sous GPL).

## 3. Autres pistes, à décider

- Signer l'installeur (Authenticode) pour que Windows SmartScreen n'avertisse plus.
- D'autres langues que le français et l'anglais.
- Vérifications sur la durée : mémoire après des centaines de sauvegardes, sessions de
  plusieurs heures.
