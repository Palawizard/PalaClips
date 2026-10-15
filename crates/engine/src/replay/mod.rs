mod ring;

pub use ring::{PacketRing, TimedPacket};

const AUDIO_KBPS_PER_TRACK: u64 = 192;

pub fn estimate_buffer_bytes(video_kbps: u32, audio_tracks: usize, duration_secs: u32) -> u64 {
    let total_kbps = video_kbps as u64 + AUDIO_KBPS_PER_TRACK * audio_tracks as u64;
    let bytes = total_kbps * 1_000 / 8 * duration_secs as u64;
    bytes + bytes / 20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_video_only_buffer_with_margin() {
        assert_eq!(
            estimate_buffer_bytes(50_000, 0, 60),
            375_000_000 + 18_750_000
        );
    }

    #[test]
    fn audio_tracks_increase_estimate() {
        assert!(estimate_buffer_bytes(40_000, 4, 60) > estimate_buffer_bytes(40_000, 0, 60));
    }
}
