//! Enregistrement manuel (démarrage/arrêt par raccourci).
//!
//! **Phase 6**.
//!
//! - Partage la même session de capture et d'encodage que le replay : démarrer un
//!   enregistrement n'ajoute ni capture ni encodeur.
//! - Peut inclure les N dernières secondes du replay au début (option « Inclure le
//!   replay au démarrage »).
//! - Écriture via `mux` en `hybrid_fragmented`.
//! - Découpe automatique en plusieurs fichiers au-delà d'une taille/durée réglable.
