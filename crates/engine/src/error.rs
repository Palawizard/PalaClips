use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("réglage invalide : {0}")]
    InvalidConfig(String),

    #[error(
        "aucun encodeur vidéo matériel n'a fonctionné : PalaClips encode uniquement avec la carte          graphique (NVIDIA NVENC, AMD AMF ou Intel Quick Sync). Mets à jour le pilote de ta carte          graphique puis relance PalaClips"
    )]
    NoHardwareEncoder,

    #[error("ta carte graphique ne sait pas encoder en {0:?} : choisis un autre codec")]
    CodecUnavailable(crate::config::VideoCodec),

    #[error("la capture audio par application nécessite Windows 10 build 20348 ou plus récent")]
    UnsupportedWindowsBuild,

    #[error("cible de capture introuvable : {0}")]
    TargetNotFound(String),

    #[error("la capture a échoué : {0}")]
    Capture(String),

    #[error("sortie vidéo refusée : {0}")]
    OutputRefused(String),

    #[error("l'encodeur vidéo a échoué : {0}")]
    Encoder(String),

    #[error("écriture du fichier vidéo impossible : {0}")]
    Mux(String),

    #[error("la capture audio a échoué : {0}")]
    Audio(String),

    #[error("la source de capture n'est plus disponible : {0}")]
    CaptureSourceLost(String),

    #[error("erreur disque : {0}")]
    Io(#[from] std::io::Error),

    #[error("un calibrage est déjà en cours")]
    CalibrationBusy,

    #[error("le moteur PalaClips est arrêté")]
    Stopped,

    #[error("aucun replay à sauvegarder : {0}")]
    NothingToSave(&'static str),

    #[error("moins de 3 s en mémoire depuis le dernier clip")]
    ClipTooShort,

    #[error("fonctionnalité pas encore implémentée : {0}")]
    NotImplemented(&'static str),
}

pub type Result<T> = std::result::Result<T, EngineError>;
