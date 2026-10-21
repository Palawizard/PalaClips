mod ring;
#[cfg(windows)]
mod store;

pub use ring::{PacketRing, TimedPacket};
#[cfg(windows)]
pub use store::{PacketReader, ReplayPacket, Spool};
#[cfg(windows)]
pub(crate) use store::{packet_data, share_packet};

use crate::clock::{HNS_PER_SECOND, Hns};

const AUDIO_KBPS_PER_TRACK: u64 = 192;
const AUDIO_LAG_MARGIN: Hns = Hns::from_millis(2_000);
const BUFFER_OVERHEAD_PERCENT: u64 = 7;

pub fn buffer_window(duration_secs: u32, keyframe_interval_secs: f32) -> Hns {
    Hns(i64::from(duration_secs) * HNS_PER_SECOND)
        + Hns::from_secs_f64(f64::from(keyframe_interval_secs))
        + AUDIO_LAG_MARGIN
}

pub fn buffer_memory_bytes(payload_bytes: u64) -> u64 {
    payload_bytes * (100 + BUFFER_OVERHEAD_PERCENT) / 100
}

pub fn estimate_buffer_bytes(video_kbps: u32, audio_tracks: usize, window: Hns) -> u64 {
    let total_kbps = video_kbps as u64 + AUDIO_KBPS_PER_TRACK * audio_tracks as u64;
    let bytes = total_kbps * 1_000 / 8 * window.0.max(0) as u64 / HNS_PER_SECOND as u64;
    buffer_memory_bytes(bytes)
}

pub const MIN_CLIP: Hns = Hns::from_millis(3_000);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipPlan {
    pub start: Hns,
    pub end: Hns,
    pub video_from: usize,
    pub audio_from: Vec<usize>,
}

impl ClipPlan {
    pub fn worth_saving(&self) -> bool {
        self.end - self.start >= MIN_CLIP
    }
}

pub fn plan_clip<P: TimedPacket>(
    video: &PacketRing<P>,
    audio: &[PacketRing<P>],
    duration: Hns,
    frame: Hns,
    audio_packet: Hns,
) -> Option<ClipPlan> {
    let end = audio
        .iter()
        .filter_map(|ring| ring.newest())
        .map(|newest| newest + audio_packet)
        .fold(video.newest()? + frame, Hns::min);
    let video_from = video.start_index(end - duration)?;
    let start = video.get(video_from)?.pts();
    if start >= end {
        return None;
    }
    Some(ClipPlan {
        start,
        end,
        video_from,
        audio_from: audio
            .iter()
            .map(|ring| ring.index_at_or_before(start))
            .collect(),
    })
}

pub fn forget_saved<P: TimedPacket>(
    video: &mut PacketRing<P>,
    audio: &mut [PacketRing<P>],
    end: Hns,
) {
    let Some(keep) = video.start_index(end) else {
        return;
    };
    video.discard_before(keep);
    let Some(start) = video.get(0).map(TimedPacket::pts) else {
        return;
    };
    for ring in audio {
        let index = ring.index_at_or_before(start);
        ring.discard_before(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Pkt {
        pts: Hns,
        key: bool,
    }

    impl TimedPacket for Pkt {
        fn pts(&self) -> Hns {
            self.pts
        }
        fn size(&self) -> usize {
            100
        }
        fn is_keyframe(&self) -> bool {
            self.key
        }
    }

    const FRAME: Hns = Hns(166_667);
    const AAC_PACKET: Hns = Hns(213_333);

    fn rings(seconds: i64, duration: Hns) -> (PacketRing<Pkt>, Vec<PacketRing<Pkt>>) {
        let window = duration + Hns::from_millis(3_000);
        let mut video = PacketRing::new(window);
        let mut audio = vec![PacketRing::new(window), PacketRing::new(window)];
        let mut next_audio = Hns(-AAC_PACKET.0);
        for frame in 0..seconds * 60 {
            let pts = Hns(frame * FRAME.0);
            while next_audio <= pts {
                for ring in &mut audio {
                    ring.push(Pkt {
                        pts: next_audio,
                        key: true,
                    });
                }
                next_audio = next_audio + AAC_PACKET;
            }
            video.push(Pkt {
                pts,
                key: frame % 60 == 0,
            });
        }
        (video, audio)
    }

    #[test]
    fn estimate_covers_the_whole_window_with_measured_overhead() {
        assert_eq!(
            estimate_buffer_bytes(50_000, 0, buffer_window(60, 1.0)),
            393_750_000 * 107 / 100
        );
    }

    #[test]
    fn audio_tracks_increase_estimate() {
        let window = buffer_window(60, 1.0);
        assert!(
            estimate_buffer_bytes(40_000, 4, window) > estimate_buffer_bytes(40_000, 0, window)
        );
    }

    #[test]
    fn clip_starts_on_a_keyframe_within_one_interval_of_the_duration() {
        let duration = Hns::from_millis(60_000);
        let (video, audio) = rings(120, duration);
        let plan = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("plan");
        let first = video.get(plan.video_from).expect("first frame");
        assert!(first.key);
        let length = plan.end - plan.start;
        assert!(length >= duration, "{length:?}");
        assert!(length <= duration + Hns::from_millis(1_000), "{length:?}");
    }

    #[test]
    fn every_audio_track_starts_on_the_packet_covering_the_keyframe() {
        let duration = Hns::from_millis(10_000);
        let (video, audio) = rings(30, duration);
        let plan = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("plan");
        for (ring, from) in audio.iter().zip(&plan.audio_from) {
            let first = ring.get(*from).expect("first audio packet").pts();
            assert!(first <= plan.start);
            assert!(plan.start < first + AAC_PACKET);
        }
        assert_eq!(plan.audio_from[0], plan.audio_from[1]);
    }

    #[test]
    fn lagging_audio_ends_the_clip_where_every_track_has_data() {
        let duration = Hns::from_millis(10_000);
        let (mut video, audio) = rings(30, duration);
        let audio_end = audio[0].newest().expect("audio") + AAC_PACKET;
        let mut pts = video.newest().expect("video");
        for _ in 0..12 {
            pts = pts + FRAME;
            video.push(Pkt { pts, key: false });
        }
        let plan = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("plan");
        assert_eq!(plan.end, audio_end);
        assert!(plan.end - plan.start >= duration);
    }

    #[test]
    fn a_short_buffer_saves_everything_it_has() {
        let (video, audio) = rings(3, Hns::from_millis(60_000));
        let plan =
            plan_clip(&video, &audio, Hns::from_millis(60_000), FRAME, AAC_PACKET).expect("plan");
        assert_eq!(plan.video_from, 0);
        assert_eq!(plan.start, Hns::ZERO);
    }

    #[test]
    fn the_next_clip_starts_where_the_saved_one_ended() {
        let duration = Hns::from_millis(60_000);
        let (mut video, mut audio) = rings(90, duration);
        let first = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("first");
        forget_saved(&mut video, &mut audio, first.end);
        assert!(
            video.buffered() < Hns::from_millis(1_000),
            "{:?}",
            video.buffered()
        );
        let mut next_audio = audio[0].newest().expect("audio") + AAC_PACKET;
        let mut pts = video.newest().expect("video");
        for frame in 1..=600 {
            pts = pts + FRAME;
            while next_audio <= pts {
                for ring in &mut audio {
                    ring.push(Pkt {
                        pts: next_audio,
                        key: true,
                    });
                }
                next_audio = next_audio + AAC_PACKET;
            }
            video.push(Pkt {
                pts,
                key: (90 * 60 + frame) % 60 == 0,
            });
        }
        let second = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("second");
        assert!(second.start <= first.end);
        assert!(first.end - second.start < Hns::from_millis(1_000));
        assert!(second.end - second.start < Hns::from_millis(12_000));
        for (ring, from) in audio.iter().zip(&second.audio_from) {
            assert!(ring.get(*from).expect("audio").pts() <= second.start);
        }
    }

    #[test]
    fn a_save_one_second_after_another_is_too_short_to_keep() {
        let duration = Hns::from_millis(60_000);
        let (mut video, mut audio) = rings(90, duration);
        let first = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("first");
        forget_saved(&mut video, &mut audio, first.end);
        let mut pts = video.newest().expect("video");
        let mut next_audio = audio[0].newest().expect("audio") + AAC_PACKET;
        for frame in 1..=60 {
            pts = pts + FRAME;
            while next_audio <= pts {
                for ring in &mut audio {
                    ring.push(Pkt {
                        pts: next_audio,
                        key: true,
                    });
                }
                next_audio = next_audio + AAC_PACKET;
            }
            video.push(Pkt {
                pts,
                key: frame % 60 == 0,
            });
        }
        let second = plan_clip(&video, &audio, duration, FRAME, AAC_PACKET).expect("second");
        assert!(!second.worth_saving(), "{:?}", second.end - second.start);
    }

    #[test]
    fn an_empty_buffer_has_no_clip() {
        let video: PacketRing<Pkt> = PacketRing::new(Hns::from_millis(1_000));
        assert_eq!(
            plan_clip(&video, &[], Hns::from_millis(1_000), FRAME, AAC_PACKET),
            None
        );
    }
}
