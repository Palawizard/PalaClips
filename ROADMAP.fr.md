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
- Le test demande une vraie machine Windows 10. Depuis l'encodage par le processeur (1.1.0),
  une machine virtuelle sans carte graphique peut aussi servir.

## 3. Marqueurs

- Un raccourci pour poser un marqueur pendant l'enregistrement ou pendant que le replay tourne.
- Les marqueurs affichés sur la barre du lecteur, pour y aller directement.
- Export vers DaVinci Resolve en EDL de marqueurs, pour les retrouver sur la timeline de montage.

## 4. Autres pistes, à décider

- Signer l'installeur (Authenticode) pour que Windows SmartScreen n'avertisse plus.
- D'autres langues que le français et l'anglais.
- Vérifications sur la durée : mémoire après des centaines de sauvegardes, sessions de
  plusieurs heures.

## Fait

- **Version 1.2.0** : Capture et Audio sont des pages à part entière dans la barre latérale, avec
  un avertissement quand on quitte une page avec un changement pas encore appliqué ; le clip peut
  continuer jusqu'à 10 min après le raccourci (un nouvel appui le prolonge) ; les vidéos se
  compressent à une taille cible (20 Mo Discord, 50 / 500 Mo Nitro ou une taille libre) depuis la
  page Compression, la galerie, le lecteur ou par glisser-déposer depuis l'Explorateur.
- **Encodage par le processeur (1.1.0)** : sans encodeur matériel qui marche, PalaClips encode
  sur le processeur, en H.264 par l'encodeur intégré à Windows ou en AV1 par SVT-AV1, avec un
  avertissement sur la charge en jeu. Le choix se fait dans Réglages → Replay & qualité →
  Encodeur.
