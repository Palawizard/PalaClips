//! File circulaire de paquets horodatés, purgée par âge.

use std::collections::VecDeque;

use crate::clock::Hns;

/// Paquet encodé horodaté sur l'horloge QPC.
pub trait TimedPacket {
    /// Horodatage de présentation (QPC, 100 ns).
    fn pts(&self) -> Hns;
    /// Taille en octets (pour les statistiques).
    fn size(&self) -> usize;
    /// Image clé (toujours `true` pour l'audio).
    fn is_keyframe(&self) -> bool;
}

/// File de paquets d'un flux, ordonnée par PTS croissant.
#[derive(Debug)]
pub struct PacketRing<P: TimedPacket> {
    packets: VecDeque<P>,
    bytes: usize,
    window: Hns,
}

impl<P: TimedPacket> PacketRing<P> {
    /// Crée une file qui conserve `window` de durée.
    pub fn new(window: Hns) -> Self {
        Self {
            packets: VecDeque::new(),
            bytes: 0,
            window,
        }
    }

    /// Ajoute un paquet et purge ce qui est plus vieux que la fenêtre.
    ///
    /// Pour la vidéo, on garde toujours au moins une image clé au début de la
    /// fenêtre, sinon le début du replay serait indécodable.
    pub fn push(&mut self, packet: P) {
        debug_assert!(
            self.packets
                .back()
                .is_none_or(|last| last.pts() <= packet.pts()),
            "les paquets doivent arriver dans l'ordre"
        );
        self.bytes += packet.size();
        let newest = packet.pts();
        self.packets.push_back(packet);
        self.purge(newest);
    }

    fn purge(&mut self, newest: Hns) {
        let limit = newest - self.window;
        // Dernière image clé dont le PTS est ≤ limite : tout ce qui la précède peut partir.
        // S'il n'y en a pas, on ne purge rien (le début doit rester décodable).
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

    /// Change la durée conservée (réglage modifié à chaud).
    pub fn set_window(&mut self, window: Hns) {
        self.window = window;
        if let Some(newest) = self.packets.back().map(TimedPacket::pts) {
            self.purge(newest);
        }
    }

    /// Indice du premier paquet à écrire pour un replay commençant au plus tôt à `start` :
    /// la dernière image clé dont le PTS est ≤ `start`, ou la première image clé disponible.
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

    /// Paquets à partir de l'indice donné (copie bon marché si `P` est `Arc`-backed).
    pub fn iter_from(&self, index: usize) -> impl Iterator<Item = &P> {
        self.packets.iter().skip(index)
    }

    /// Octets occupés.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Durée couverte par la file.
    pub fn buffered(&self) -> Hns {
        match (self.packets.front(), self.packets.back()) {
            (Some(a), Some(b)) => b.pts() - a.pts(),
            _ => Hns::ZERO,
        }
    }

    /// Nombre de paquets.
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// File vide.
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Vide la file (changement de sources ou de zone : le buffer redémarre).
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

    /// Flux vidéo 10 fps, image clé toutes les secondes.
    fn video(ring: &mut PacketRing<Pkt>, secs: i64) {
        for i in 0..secs * 10 {
            ring.push(Pkt {
                pts: Hns::from_millis(i * 100),
                key: i % 10 == 0,
            });
        }
    }

    #[test]
    fn purge_par_age_garde_une_image_cle_au_debut() {
        let mut ring = PacketRing::new(Hns::from_millis(3_000));
        video(&mut ring, 10);
        assert!(ring.iter_from(0).next().is_some_and(|p| p.key));
        // Fenêtre de 3 s + au plus 1 s pour remonter à l'image clé.
        assert!(ring.buffered() <= Hns::from_millis(4_000));
        assert!(ring.buffered() >= Hns::from_millis(3_000));
    }

    #[test]
    fn octets_coherents_apres_purge() {
        let mut ring = PacketRing::new(Hns::from_millis(2_000));
        video(&mut ring, 20);
        assert_eq!(ring.bytes(), ring.len() * 100);
    }

    #[test]
    fn debut_du_replay_sur_image_cle() {
        let mut ring = PacketRing::new(Hns::from_millis(10_000));
        video(&mut ring, 10);
        let idx = ring.start_index(Hns::from_millis(4_500)).expect("indice");
        let first = ring.iter_from(idx).next().expect("paquet");
        assert!(first.key);
        assert_eq!(first.pts, Hns::from_millis(4_000));
    }

    #[test]
    fn reduire_la_fenetre_a_chaud() {
        let mut ring = PacketRing::new(Hns::from_millis(10_000));
        video(&mut ring, 10);
        ring.set_window(Hns::from_millis(2_000));
        assert!(ring.buffered() <= Hns::from_millis(3_000));
    }

    #[test]
    fn audio_toutes_cles() {
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
