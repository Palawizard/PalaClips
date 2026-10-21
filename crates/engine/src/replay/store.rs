use std::fs::{self, File};
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::Arc;

use rsmpeg::avcodec::AVPacket;
use rsmpeg::ffi;

use super::TimedPacket;
use crate::clock::Hns;
use crate::error::{EngineError, Result};

const SEGMENT_DURATION: Hns = Hns::from_millis(4_000);
const SEGMENT_EXTENSION: &str = "seg";

pub struct ReplayPacket {
    pts: Hns,
    keyframe: bool,
    size: usize,
    payload: Payload,
}

enum Payload {
    Ram(AVPacket),
    Disk {
        segment: Arc<Segment>,
        offset: u64,
        timing: Timing,
    },
}

#[derive(Debug, Clone, Copy)]
struct Timing {
    pts: i64,
    dts: i64,
    duration: i64,
    flags: i32,
}

impl Timing {
    fn of(packet: &AVPacket) -> Self {
        Self {
            pts: packet.pts,
            dts: packet.dts,
            duration: packet.duration,
            flags: packet.flags,
        }
    }
}

impl std::fmt::Debug for ReplayPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplayPacket")
            .field("pts", &self.pts)
            .field("keyframe", &self.keyframe)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl TimedPacket for ReplayPacket {
    fn pts(&self) -> Hns {
        self.pts
    }

    fn size(&self) -> usize {
        self.size
    }

    fn is_keyframe(&self) -> bool {
        self.keyframe
    }
}

impl ReplayPacket {
    pub fn in_ram(packet: AVPacket, pts: Hns, keyframe: bool) -> Self {
        Self {
            pts,
            keyframe,
            size: packet_data(&packet).len(),
            payload: Payload::Ram(packet),
        }
    }

    pub fn share(&self) -> Result<Self> {
        let payload = match &self.payload {
            Payload::Ram(packet) => Payload::Ram(share_packet(packet)?),
            Payload::Disk {
                segment,
                offset,
                timing,
            } => Payload::Disk {
                segment: segment.clone(),
                offset: *offset,
                timing: *timing,
            },
        };
        Ok(Self {
            pts: self.pts,
            keyframe: self.keyframe,
            size: self.size,
            payload,
        })
    }
}

#[derive(Debug, Default)]
pub struct PacketReader {
    open: Option<(Arc<Segment>, File)>,
}

impl PacketReader {
    pub fn read(&mut self, packet: ReplayPacket) -> Result<AVPacket> {
        let (segment, offset, timing) = match packet.payload {
            Payload::Ram(packet) => return Ok(packet),
            Payload::Disk {
                segment,
                offset,
                timing,
            } => (segment, offset, timing),
        };
        if self
            .open
            .as_ref()
            .is_none_or(|(open, _)| !Arc::ptr_eq(open, &segment))
        {
            let file = File::open(&segment.path)?;
            self.open = Some((segment, file));
        }
        let Some((_, file)) = self.open.as_mut() else {
            return Err(EngineError::Mux("segment de replay introuvable".into()));
        };
        let mut restored = AVPacket::new();
        // SAFETY: av_new_packet allocates `size` bytes (plus padding) owned by `restored`.
        let status = unsafe { ffi::av_new_packet(restored.as_mut_ptr(), packet.size as i32) };
        if status < 0 {
            return Err(EngineError::Mux(
                "mémoire insuffisante pour relire le replay".into(),
            ));
        }
        if packet.size > 0 {
            // SAFETY: av_new_packet succeeded, so `data` points to `size` writable bytes.
            let data = unsafe { std::slice::from_raw_parts_mut(restored.data, packet.size) };
            file.seek(SeekFrom::Start(offset))?;
            file.read_exact(data)?;
        }
        restored.set_pts(timing.pts);
        restored.set_dts(timing.dts);
        restored.set_duration(timing.duration);
        restored.set_flags(timing.flags);
        Ok(restored)
    }
}

#[derive(Debug)]
struct Segment {
    path: PathBuf,
}

impl Drop for Segment {
    fn drop(&mut self) {
        fs::remove_file(&self.path).ok();
    }
}

#[derive(Debug)]
struct OpenSegment {
    segment: Arc<Segment>,
    writer: BufWriter<File>,
    offset: u64,
    started: Hns,
}

#[derive(Debug)]
pub struct Spool {
    dir: PathBuf,
    current: Option<OpenSegment>,
    next_id: u64,
}

impl Spool {
    pub fn new(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        for entry in fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == SEGMENT_EXTENSION) {
                fs::remove_file(path).ok();
            }
        }
        Ok(Self {
            dir,
            current: None,
            next_id: 0,
        })
    }

    pub fn store(&mut self, packet: AVPacket, pts: Hns, keyframe: bool) -> Result<ReplayPacket> {
        if self
            .current
            .as_ref()
            .is_none_or(|open| pts - open.started >= SEGMENT_DURATION)
        {
            self.rotate(pts)?;
        }
        let Some(open) = self.current.as_mut() else {
            return Err(EngineError::Mux("segment de replay non ouvert".into()));
        };
        let data = packet_data(&packet);
        open.writer.write_all(data)?;
        let stored = ReplayPacket {
            pts,
            keyframe,
            size: data.len(),
            payload: Payload::Disk {
                segment: open.segment.clone(),
                offset: open.offset,
                timing: Timing::of(&packet),
            },
        };
        open.offset += data.len() as u64;
        Ok(stored)
    }

    pub fn flush(&mut self) -> Result<()> {
        if let Some(open) = self.current.as_mut() {
            open.writer.flush()?;
        }
        Ok(())
    }

    fn rotate(&mut self, started: Hns) -> Result<()> {
        self.flush()?;
        let path = self
            .dir
            .join(format!("{:08}.{SEGMENT_EXTENSION}", self.next_id));
        self.next_id += 1;
        let writer = BufWriter::new(File::create(&path)?);
        self.current = Some(OpenSegment {
            segment: Arc::new(Segment { path }),
            writer,
            offset: 0,
            started,
        });
        Ok(())
    }
}

pub(crate) fn packet_data(packet: &AVPacket) -> &[u8] {
    if packet.data.is_null() || packet.size <= 0 {
        return &[];
    }
    // SAFETY: a non-null packet buffer holds `size` readable bytes for the packet's lifetime.
    unsafe { std::slice::from_raw_parts(packet.data, packet.size as usize) }
}

pub(crate) fn share_packet(packet: &AVPacket) -> Result<AVPacket> {
    // SAFETY: av_packet_clone returns a new packet that references the same refcounted buffer.
    let cloned = unsafe { ffi::av_packet_clone(packet.as_ptr()) };
    NonNull::new(cloned)
        // SAFETY: `cloned` was just allocated by FFmpeg and is owned by the returned wrapper.
        .map(|cloned| unsafe { AVPacket::from_raw(cloned) })
        .ok_or_else(|| EngineError::Mux("mémoire insuffisante pour copier un paquet".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(bytes: &[u8], pts: i64) -> AVPacket {
        let mut packet = AVPacket::new();
        // SAFETY: av_new_packet allocates `len` bytes owned by `packet`.
        let status = unsafe { ffi::av_new_packet(packet.as_mut_ptr(), bytes.len() as i32) };
        assert!(status >= 0);
        // SAFETY: the buffer was just allocated with exactly `len` bytes.
        unsafe { std::slice::from_raw_parts_mut(packet.data, bytes.len()) }.copy_from_slice(bytes);
        packet.set_pts(pts);
        packet.set_dts(pts);
        packet.set_duration(1);
        packet
    }

    fn spool_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("palaclips-spool-{name}-{}", std::process::id()))
    }

    #[test]
    fn shared_ram_packets_keep_their_bytes() {
        let original = ReplayPacket::in_ram(packet(b"abcdef", 7), Hns(70), true);
        let copy = original.share().expect("share");
        drop(original);
        let restored = PacketReader::default().read(copy).expect("read");
        assert_eq!(packet_data(&restored), b"abcdef");
        assert_eq!(restored.pts, 7);
    }

    #[test]
    fn disk_packets_read_back_identical() {
        let dir = spool_dir("read");
        let mut spool = Spool::new(dir.clone()).expect("spool");
        let first = spool
            .store(packet(b"first", 1), Hns(0), true)
            .expect("store");
        let second = spool
            .store(packet(b"second!", 2), SEGMENT_DURATION, true)
            .expect("store");
        spool.flush().expect("flush");
        let mut reader = PacketReader::default();
        let a = reader.read(first.share().expect("share")).expect("read");
        let b = reader.read(second.share().expect("share")).expect("read");
        assert_eq!(packet_data(&a), b"first");
        assert_eq!(packet_data(&b), b"second!");
        assert_eq!((a.pts, b.pts), (1, 2));
        drop((first, second, reader, spool));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn purged_segments_are_deleted_from_disk() {
        let dir = spool_dir("purge");
        let mut spool = Spool::new(dir.clone()).expect("spool");
        let old = spool.store(packet(b"old", 0), Hns(0), true).expect("store");
        let new = spool
            .store(packet(b"new", 1), SEGMENT_DURATION, true)
            .expect("store");
        let count = || fs::read_dir(&dir).expect("dir").count();
        assert_eq!(count(), 2);
        drop(old);
        assert_eq!(count(), 1);
        drop((new, spool));
        assert_eq!(count(), 0);
        fs::remove_dir_all(dir).ok();
    }
}
