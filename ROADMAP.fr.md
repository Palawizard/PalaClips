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

## 2. Windows 10 (2004 et après)

PalaClips demande aujourd'hui Windows 11. Windows 10 version 2004 (build 19041) et après
pourrait marcher :

- La capture du son par appli n'est pas documentée officiellement avant Windows 11, mais elle
  marche déjà dès la build 19041 : OBS (avec le plugin win-capture-audio) et GStreamer s'en
  servent.
- La bordure jaune que Windows dessine autour d'un écran capturé ne pourra probablement pas être
  masquée sous Windows 10.
- Le test demande une vraie machine Windows 10. Avec l'encodage par le processeur (section
  suivante), une machine virtuelle sans carte graphique pourrait aussi servir.

## 3. Encodage par le processeur (CPU)

Aujourd'hui, PalaClips n'encode que par la carte graphique (NVIDIA NVENC, AMD AMF, Intel Quick
Sync) : un PC sans encodeur matériel ne peut pas enregistrer. Ensuite : un encodeur sur le
processeur, utilisé quand aucun encodeur matériel ne marche, avec un avertissement clair sur la
charge qu'il ajoute en jeu. PalaClips livre FFmpeg sous licence LGPL : les candidats sont des
encodeurs à licence compatible, comme OpenH264 ou SVT-AV1 (x264 et x265 sont sous GPL).

## 4. Marqueurs

- Un raccourci pour poser un marqueur pendant l'enregistrement ou pendant que le replay tourne.
- Les marqueurs affichés sur la barre du lecteur, pour y aller directement.
- Export vers DaVinci Resolve en EDL de marqueurs, pour les retrouver sur la timeline de montage.

## 5. Compression à une taille cible

- Une page dédiée dans la barre latérale, et un bouton depuis la galerie et le lecteur.
- Taille en Mo, avec des préréglages (20 Mo pour Discord, 500 Mo pour Nitro) ou une valeur libre.
- Débit calculé d'après la taille et la durée ; son mixé en une seule piste.
- Si la cible est trop basse pour une qualité correcte, la résolution et les images par seconde
  baissent automatiquement.
- La taille finale est vérifiée, et la vidéo est réencodée si elle dépasse la cible.
- Accepte aussi une vidéo glissée depuis l'explorateur de fichiers.

## 6. Capture et Audio dans la barre latérale

- Capture et Audio deviennent des pages à part entière : État, Galerie, Capture, Audio, Réglages.
- Un avertissement quand on quitte une page avec un changement pas encore appliqué.
- Sur l'écran État, la source et les pistes audio ouvrent ces pages au clic.

## 7. Autres pistes, à décider

- Signer l'installeur (Authenticode) pour que Windows SmartScreen n'avertisse plus.
- D'autres langues que le français et l'anglais.
- Vérifications sur la durée : mémoire après des centaines de sauvegardes, sessions de
  plusieurs heures.
