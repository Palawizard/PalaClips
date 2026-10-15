//! Replay instantané : buffer circulaire de **paquets déjà encodés**.
//!
//! - Une file par flux (vidéo + chaque piste audio), purgée **par âge** (horloge QPC),
//!   jamais par nombre de paquets.
//! - À la sauvegarde : on remonte à l'image clé vidéo la plus proche avant
//!   `maintenant - durée`, on aligne chaque piste audio sur ce PTS, puis on écrit
//!   le MP4 dans un thread à part (voir `mux`), sans bloquer la capture.
//!
//! La logique de file ([`PacketRing`]) est indépendante de Windows et testée ici.
//! Le stockage sur disque (option « Disque ») réutilise la même interface.

mod ring;

pub use ring::{PacketRing, TimedPacket};

/// Estimation de la taille du buffer, en octets, affichée en direct dans l'UI.
///
/// `vidéo (kbit/s) × durée / 8` + 192 kbit/s par piste audio AAC stéréo,
/// + 5 % de marge pour les en-têtes et la variabilité du débit.
pub fn estimate_buffer_bytes(video_kbps: u32, audio_tracks: usize, duration_secs: u32) -> u64 {
    const AUDIO_KBPS_PER_TRACK: u64 = 192;
    let total_kbps = video_kbps as u64 + AUDIO_KBPS_PER_TRACK * audio_tracks as u64;
    let bytes = total_kbps * 1_000 / 8 * duration_secs as u64;
    bytes + bytes / 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimation_exemple_doc() {
        // 50 Mb/s × 60 s / 8 = 375 Mo, sans audio, + 5 %
        let b = estimate_buffer_bytes(50_000, 0, 60);
        assert_eq!(b, 375_000_000 + 18_750_000);
    }

    #[test]
    fn estimation_avec_pistes() {
        let sans = estimate_buffer_bytes(40_000, 0, 60);
        let avec = estimate_buffer_bytes(40_000, 4, 60);
        assert!(avec > sans);
    }
}
