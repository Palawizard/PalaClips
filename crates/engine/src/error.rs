//! Erreurs du moteur.
//!
//! Chaque variante doit pouvoir être affichée telle quelle à l'utilisateur
//! (message en français, actionnable). Les détails techniques vont dans les logs.

use thiserror::Error;

/// Erreur du moteur PalaClips.
#[derive(Debug, Error)]
pub enum EngineError {
    /// Configuration invalide (valeur hors bornes, combinaison impossible).
    #[error("réglage invalide : {0}")]
    InvalidConfig(String),

    /// Aucun encodeur matériel utilisable (NVENC, AMF ou QSV).
    #[error("aucun encodeur vidéo matériel compatible n'a été trouvé")]
    NoHardwareEncoder,

    /// La version de Windows ne permet pas la capture audio par application.
    #[error("la capture audio par application nécessite Windows 10 build 20348 ou plus récent")]
    UnsupportedWindowsBuild,

    /// La source de capture (écran ou fenêtre) n'est plus disponible.
    #[error("la source de capture n'est plus disponible : {0}")]
    CaptureSourceLost(String),

    /// Erreur d'entrée/sortie (disque plein, dossier inaccessible...).
    #[error("erreur disque : {0}")]
    Io(#[from] std::io::Error),

    /// Fonctionnalité pas encore implémentée dans la phase en cours.
    /// Ne doit plus exister dans la V1 livrée (vérifié par la checklist de release).
    #[error("fonctionnalité pas encore implémentée : {0}")]
    NotImplemented(&'static str),
}

/// Alias de résultat du moteur.
pub type Result<T> = std::result::Result<T, EngineError>;
