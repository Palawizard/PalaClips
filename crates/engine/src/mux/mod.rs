//! Écriture des fichiers MP4.
//!
//! **Phases 4 et 6**.
//!
//! - Replay : MP4 classique écrit dans un thread à part à partir des paquets du
//!   buffer (`replay::PacketRing`), début sur image clé, audio aligné.
//! - Enregistrement manuel : `movflags +hybrid_fragmented` — fragmenté pendant
//!   l'écriture (lisible après un crash), MP4 normal à la fin. **Jamais** de MP4
//!   fragmenté brut (mal supporté par les éditeurs).
//! - Métadonnées : titre de chaque piste audio (`Mix complet`, `Micro`, nom de
//!   l'appli) pour que DaVinci Resolve et les lecteurs les affichent.
//! - Nom de fichier : `<Jeu ou appli au premier plan>_<AAAA-MM-JJ_HH-MM-SS>.mp4`,
//!   dans `<dossier clips>/<Jeu>/`.
//! - Vérifier l'espace disque avant d'écrire ; alerte `disk_almost_full`.
