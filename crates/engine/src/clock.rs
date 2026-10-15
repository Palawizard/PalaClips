//! Horloge unique du moteur.
//!
//! Toutes les sources sont horodatées sur la même horloge : le compteur de
//! performance Windows (QPC), exprimé en **unités de 100 ns** (« hns »).
//!
//! - Vidéo : `Direct3D11CaptureFrame.SystemRelativeTime` (QPC au rendu par le compositeur).
//! - Audio : `IAudioCaptureClient::GetBuffer` → `pu64QPCPosition`, déjà converti en 100 ns.
//!
//! Les PTS des fichiers sont calculés **uniquement** à partir de ces horodatages,
//! jamais en additionnant des durées théoriques (voir CLAUDE.md, règle 1).

use std::ops::{Add, Sub};

use serde::{Deserialize, Serialize};

/// Nombre d'unités de 100 ns par seconde.
pub const HNS_PER_SECOND: i64 = 10_000_000;

/// Instant ou durée exprimé en unités de 100 ns sur l'horloge QPC.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Hns(pub i64);

impl Hns {
    /// Zéro.
    pub const ZERO: Hns = Hns(0);

    /// Construit depuis des secondes (arrondi à l'unité la plus proche).
    pub fn from_secs_f64(secs: f64) -> Self {
        Hns((secs * HNS_PER_SECOND as f64).round() as i64)
    }

    /// Construit depuis des millisecondes.
    pub const fn from_millis(ms: i64) -> Self {
        Hns(ms * 10_000)
    }

    /// Valeur en secondes.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / HNS_PER_SECOND as f64
    }

    /// Convertit en PTS dans une base de temps `num/den` (ex. 1/48000 pour l'audio,
    /// 1/90000 ou 1/fps pour la vidéo), avec arrondi au plus proche.
    ///
    /// Calcul en i128 pour éviter tout débordement sur de longues sessions.
    pub fn to_timebase(self, num: i64, den: i64) -> i64 {
        debug_assert!(num > 0 && den > 0, "base de temps invalide");
        let value = self.0 as i128 * den as i128;
        let divisor = HNS_PER_SECOND as i128 * num as i128;
        // Arrondi au plus proche, correct aussi pour les valeurs négatives.
        let rounded = if value >= 0 {
            (value + divisor / 2) / divisor
        } else {
            (value - divisor / 2) / divisor
        };
        rounded as i64
    }

    /// Nombre d'échantillons audio correspondant à cette durée, à `sample_rate` Hz.
    /// Utilisé pour combler les trous de silence (CLAUDE.md, règle 3).
    pub fn to_samples(self, sample_rate: u32) -> i64 {
        self.to_timebase(1, sample_rate as i64)
    }
}

impl Add for Hns {
    type Output = Hns;
    fn add(self, rhs: Hns) -> Hns {
        Hns(self.0 + rhs.0)
    }
}

impl Sub for Hns {
    type Output = Hns;
    fn sub(self, rhs: Hns) -> Hns {
        Hns(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondes_aller_retour() {
        let t = Hns::from_secs_f64(1.5);
        assert_eq!(t, Hns(15_000_000));
        assert!((t.as_secs_f64() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn conversion_base_audio_48k() {
        // 1 s = 48 000 échantillons
        assert_eq!(Hns(HNS_PER_SECOND).to_samples(48_000), 48_000);
        // 10 ms = 480 échantillons
        assert_eq!(Hns::from_millis(10).to_samples(48_000), 480);
    }

    #[test]
    fn conversion_base_video_60fps() {
        // 1 s à 60 fps = 60 images
        assert_eq!(Hns(HNS_PER_SECOND).to_timebase(1, 60), 60);
        // 1 image à 60 fps ≈ 166 667 hns → 1
        assert_eq!(Hns(166_667).to_timebase(1, 60), 1);
    }

    #[test]
    fn pas_de_debordement_sur_24h() {
        let day = Hns(HNS_PER_SECOND * 86_400);
        assert_eq!(day.to_timebase(1, 90_000), 90_000 * 86_400);
    }

    #[test]
    fn arrondi_valeurs_negatives() {
        assert_eq!(Hns(-15_000_000).to_timebase(1, 1), -2);
        assert_eq!(Hns(-14_999_999).to_timebase(1, 1), -1);
    }
}
