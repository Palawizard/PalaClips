# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

L'interface est une app web (React 19 + TypeScript + Tailwind 4) affichée dans le WebView2
d'une application de bureau Tauri 2, Windows 10 build 20348+ et Windows 11 uniquement.

## Users

Joueur et créateur de contenu sur Windows, à l'aise avec la technique, qui monte ses vidéos
dans DaVinci Resolve. Il ouvre l'app pour régler la capture, puis l'oublie : elle vit dans la
zone de notification pendant qu'il joue. Il revient surtout pour la galerie : revoir,
découper, partager ses clips.

## Product Purpose

PalaClips enregistre ce qui se passe à l'écran et garde les dernières secondes de jeu en
mémoire pour les sauvegarder d'un raccourci (replay instantané), et enregistre aussi des
sessions entières. C'est une alternative légère à NVIDIA ShadowPlay, gratuite, open source
(MIT), sans compte ni télémétrie.

Succès : un raccourci pendant la partie produit un clip propre sans interrompre le jeu, et ce
clip arrive dans Resolve avec chaque son déjà séparé et synchronisé.

## Positioning

Chaque logiciel sur sa propre piste audio (jeu, Discord, musique, navigateur, micro), par
capture audio par processus de Windows, sans injection dans le jeu, donc compatible avec les
anti-triches (Easy Anti-Cheat, BattlEye, Vanguard). Piste 1 = mix complet, piste 2 = micro,
puis une piste par appli, titrées dans le fichier.

## Operating Context

- Utilisé pendant une partie, souvent en plein écran : l'app ne doit jamais voler le focus ;
  la confirmation d'une sauvegarde passe par un toast et un son.
- Fenêtre fermée la plupart du temps (WebView détruit) ; contrôle par la zone de notification,
  son menu et des raccourcis globaux.
- La fenêtre s'ouvre pour régler (zone de capture, sources audio, replay et qualité,
  raccourcis, stockage, général) ou pour la galerie, le lecteur et la découpe.
- Aval : import des clips dans DaVinci Resolve, partage rapide sur Discord (copier dans le
  presse-papiers).
- Machine de référence : RTX 4070 Super, écran 1440p 240 Hz ; jeu de test principal Fortnite.

## Capabilities and Constraints

- Écrans V1, tous obligatoires : État (accueil), Galerie, Lecteur / découpe, Zone de capture,
  Sources audio, Replay & qualité, Raccourcis, Stockage, Général / À propos, Premier
  lancement, plus le menu de la zone de notification et les toasts Windows.
- La galerie reste fluide à 5 000 clips.
- Le lecteur affiche une ligne par piste audio (forme d'onde, mute, solo, volume).
- Sélecteurs vivants : miniatures en direct des écrans et fenêtres, vumètres en direct des
  applis qui font du son (données à ~20 Hz).
- Estimation de la RAM du buffer de replay mise à jour en direct pendant les réglages.
- Hors ligne : aucune police ni ressource chargée depuis Internet, polices embarquées.
- Fenêtre min. 900 × 600, cible 1180 × 760, redimensionnable ; mise à l'échelle Windows
  100–200 %.
- Sombre par défaut, thème clair complet.
- Interface en français et en anglais ; prévoir des libellés plus longs en français.
- Légèreté : pas de polling à vide, CPU au repos minimal.
- Hors périmètre V1 : streaming, cloud, comptes, partage par lien, éditeur multi-clips,
  overlay en jeu, effets audio, webcam, Linux et macOS.

## Brand Commitments

- Nom : PalaClips.
- Ton : outil de précision, calme et rapide. Pas « gamer RGB », pas de néon, pas de
  dégradés criards. Natif Windows 11 dans l'esprit (sobriété, coins, rythme), avec sa
  propre identité.
- Voix française : tutoiement (« Ton replay est armé », « Choisis les applis à séparer »).
  Messages d'erreur actionnables.
- Logo actuel (`app/src-tauri/icons/app-icon.svg`, anneau « retour en arrière » rouge
  `#FF4D5E` et triangle lecture sur ardoise) : **provisoire**, peut changer entièrement.

## Evidence on Hand

- Aucun témoignage, chiffre d'usage, capture d'écran ni benchmark publié : ne pas en inventer.
- Objectifs de performance chiffrés dans `docs/SPEC.md` (PERF-01 à 04), à mesurer en phase 7.

## Product Principles

1. Le jeu d'abord : rien dans PalaClips ne doit gêner la partie (focus, performances).
2. La confiance se voit : on sait d'un coup d'œil si le replay tourne, ce qui est capturé et
   ce que ça coûte (RAM, CPU).
3. Densité maîtrisée : beaucoup de réglages, jamais tous en même temps.
4. Les pistes séparées sont le cœur : chaque écran audio rend la séparation évidente et sûre.
5. Le mouvement sert la compréhension (clip sauvegardé, vumètres, état du replay), jamais la
   décoration ; `prefers-reduced-motion` respecté.

## Accessibility & Inclusion

Navigation clavier complète, focus visible, contrastes WCAG AA minimum,
`prefers-reduced-motion` respecté partout, mise à l'échelle Windows 100–200 %.
