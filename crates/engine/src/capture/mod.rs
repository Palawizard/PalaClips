//! Capture vidéo — Windows Graphics Capture via `windows-capture` 2.x.
//!
//! **Phase 1** (voir `docs/PLAN.md`).
//!
//! Responsabilités :
//! - Énumérer écrans et fenêtres capturables pour le sélecteur (`Window::enumerate`
//!   filtré par `is_valid()`, énumération des écrans), avec miniatures.
//! - Démarrer une session WGC sur la cible choisie (`CaptureTarget`), sans bordure
//!   jaune si l'utilisateur l'a désactivée (`DrawBorderSettings`), curseur selon réglage.
//! - Passer la **texture D3D11 brute** (`Frame::as_raw_texture`) à l'encodeur, sans
//!   copie CPU, avec l'horodatage QPC (`Frame::timestamp` = `SystemRelativeTime`).
//! - Produire un flux **CFR** : WGC n'émet une image que si l'écran change, donc on
//!   duplique la dernière image (ou on saute les images en trop) pour tenir le fps cible.
//! - Repli automatique sur l'écran contenant la fenêtre si la capture de fenêtre
//!   échoue (vieux plein écran exclusif), avec une alerte `capture_fallback_monitor`.
//! - Suivre les changements de résolution, la fermeture de la fenêtre cible et la
//!   perte du périphérique D3D (redémarrage du pilote).
//!
//! Interdit : toute injection dans le processus capturé (anti-cheat).
