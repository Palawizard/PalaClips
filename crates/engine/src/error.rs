use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("réglage invalide : {0}")]
    InvalidConfig(String),

    #[error("aucun encodeur vidéo matériel compatible n'a été trouvé")]
    NoHardwareEncoder,

    #[error("la capture audio par application nécessite Windows 10 build 20348 ou plus récent")]
    UnsupportedWindowsBuild,

    #[error("cible de capture introuvable : {0}")]
    TargetNotFound(String),

    #[error("la capture a échoué : {0}")]
    Capture(String),

    #[error("l'encodeur vidéo a échoué : {0}")]
    Encoder(String),

    #[error("la source de capture n'est plus disponible : {0}")]
    CaptureSourceLost(String),

    #[error("erreur disque : {0}")]
    Io(#[from] std::io::Error),

    #[error("fonctionnalité pas encore implémentée : {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, EngineError>;
