//! Sources sélectionnables : écrans, fenêtres, sessions audio, périphériques.
//!
//! **Phases 3 et 5**. Alimente les sélecteurs de l'UI.
//!
//! - Liste des applis ayant une session audio (actives d'abord), avec nom d'exe,
//!   nom lisible, chemin de l'exe (pour l'icône) et crête en direct.
//! - Liste des processus pour ajouter une appli **pas encore lancée**.
//! - Résolution exe → PID parent courant, et surveillance des démarrages/arrêts
//!   (rattachement automatique des pistes).
//! - Appli au premier plan (piste « Jeu » automatique et nom des clips).
//! - Périphériques audio d'entrée/sortie et notifications de changement du
//!   périphérique par défaut.
//!
//! Persistance : toujours par nom d'exe / identifiant de périphérique, jamais par PID.
