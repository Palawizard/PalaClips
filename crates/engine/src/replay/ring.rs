use std::collections::VecDeque;

use crate::clock::Hns;

pub trait TimedPacket {
    fn pts(&self) -> Hns;
    fn size(&self) -> usize;
    fn is_keyframe(&self) -> bool;
}

#[derive(Debug)]
pub struct PacketRing<P: TimedPacket> {
    packets: VecDeque<P>,
    bytes: usize,
    window: Hns,
}

impl<P: TimedPacket> PacketRing<P> {
    pub fn new(window: Hns) -> Self {
        Self {
            packets: VecDeque::new(),
            bytes: 0,
            window,
        }
    }

    pub fn push(&mut self, packet: P) {
        debug_assert!(
            self.packets
                .back()
                .is_none_or(|last| last.pts() <= packet.pts())
        );
        self.bytes += packet.size();
        let newest = packet.pts();
        self.packets.push_back(packet);
        self.purge(newest);
    }

    fn purge(&mut self, newest: Hns) {
        let limit = newest - self.window;
        let cut = self
            .packets
            .iter()
            .take_while(|p| p.pts() <= limit)
            .enumerate()
            .filter(|(_, p)| p.is_keyframe())
            .map(|(i, _)| i)
            .last();
        if let Some(cut) = cut {
            for old in self.packets.drain(..cut) {
                self.bytes -= old.size();
            }
        }
    }

    pub fn set_window(&mut self, window: Hns) {
        self.window = window;
        if let Some(newest) = self.packets.back().map(TimedPacket::pts) {
            self.purge(newest);
        }
    }

    pub fn start_index(&self, start: Hns) -> Option<usize> {
        let mut candidate = None;
        for (i, p) in self.packets.iter().enumerate() {
            if !p.is_keyframe() {
                continue;
            }
            if p.pts() <= start {
                candidate = Some(i);
            } else {
                return candidate.or(Some(i));
            }
        }
        candidate
    }

    pub fn iter_from(&self, index: usize) -> impl Iterator<Item = &P> {
        self.packets.iter().skip(index)
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn buffered(&self) -> Hns {
        match (self.packets.front(), self.packets.back()) {
            (Some(a), Some(b)) => b.pts() - a.pts(),
            _ => Hns::ZERO,
        }
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn clear(&mut self) {
        self.packets.clear();
        self.bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
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

    fn push_video_10fps_keyframe_every_second(ring: &mut PacketRing<Pkt>, secs: i64) {
        for i in 0..secs * 10 {
            ring.push(Pkt {
                pts: Hns::from_millis(i * 100),
                key: i % 10 == 0,
            });
        }
    }

    #[test]
    fn purge_keeps_a_leading_keyframe() {
        let mut ring = PacketRing::new(Hns::from_millis(3_000));
        push_video_10fps_keyframe_every_second(&mut ring, 10);
        assert!(ring.iter_from(0).next().is_some_and(|p| p.key));
        assert!(ring.buffered() <= Hns::from_millis(4_000));
        assert!(ring.buffered() >= Hns::from_millis(3_000));
    }

    #[test]
    fn byte_count_matches_after_purge() {
        let mut ring = PacketRing::new(Hns::from_millis(2_000));
        push_video_10fps_keyframe_every_second(&mut ring, 20);
        assert_eq!(ring.bytes(), ring.len() * 100);
    }

    #[test]
    fn replay_starts_on_keyframe() {
        let mut ring = PacketRing::new(Hns::from_millis(10_000));
        push_video_10fps_keyframe_every_second(&mut ring, 10);
        let idx = ring.start_index(Hns::from_millis(4_500)).expect("index");
        let first = ring.iter_from(idx).next().expect("packet");
        assert!(first.key);
        assert_eq!(first.pts, Hns::from_millis(4_000));
    }

    #[test]
    fn shrinking_window_purges_immediately() {
        let mut ring = PacketRing::new(Hns::from_millis(10_000));
        push_video_10fps_keyframe_every_second(&mut ring, 10);
        ring.set_window(Hns::from_millis(2_000));
        assert!(ring.buffered() <= Hns::from_millis(3_000));
    }

    #[test]
    fn audio_packets_purge_to_window() {
        let mut ring = PacketRing::new(Hns::from_millis(1_000));
        for i in 0..500 {
            ring.push(Pkt {
                pts: Hns::from_millis(i * 10),
                key: true,
            });
        }
        assert!(ring.buffered() <= Hns::from_millis(1_000));
    }
}
