use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Result;
use palaclips_engine::audio::{AudioSource, AudioTrack, Mixdown};
use palaclips_engine::capture::VideoCapture;
use palaclips_engine::clock::Hns;
use palaclips_engine::encode::{create_device, hardware_video_encoders, pick_encoder};
use palaclips_engine::sources::CurrentGame;
use windows::Win32::System::Memory::{
    GetProcessHeaps, HEAP_SUMMARY, HeapSummary, MEM_COMMIT, MEM_IMAGE, MEM_MAPPED, MEM_PRIVATE,
    MEMORY_BASIC_INFORMATION, VirtualQuery,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::GetCurrentProcess;

const SETTLE: Duration = Duration::from_secs(5);

#[derive(Debug, clap::Args)]
pub struct ProbeMemoryArgs {
    #[command(flatten)]
    pub video: crate::VideoArgs,
    #[arg(long = "app", help = "Nom d'exe d'une appli (répétable)")]
    pub apps: Vec<String>,
}

#[derive(Debug, Default, Clone)]
struct Snapshot {
    private: u64,
    working_set: u64,
    heaps: u64,
    private_blocks: BTreeMap<usize, u64>,
    image: u64,
    mapped: u64,
}

impl Snapshot {
    fn take() -> Self {
        let mut snapshot = Self::default();
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        // SAFETY: the pseudo-handle of the current process is always valid and `counters` is sized by `cb`.
        if unsafe {
            GetProcessMemoryInfo(
                GetCurrentProcess(),
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                counters.cb,
            )
        }
        .is_ok()
        {
            snapshot.private = counters.PrivateUsage as u64;
            snapshot.working_set = counters.WorkingSetSize as u64;
        }
        let mut address = 0usize;
        loop {
            let mut info = MEMORY_BASIC_INFORMATION::default();
            // SAFETY: VirtualQuery only reads the address space layout into `info`.
            let written = unsafe {
                VirtualQuery(
                    Some(address as *const _),
                    &mut info,
                    size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            };
            if written == 0 {
                break;
            }
            if info.State == MEM_COMMIT {
                let size = info.RegionSize as u64;
                match info.Type {
                    MEM_PRIVATE => {
                        *snapshot
                            .private_blocks
                            .entry(info.AllocationBase as usize)
                            .or_default() += size
                    }
                    MEM_IMAGE => snapshot.image += size,
                    MEM_MAPPED => snapshot.mapped += size,
                    _ => {}
                }
            }
            address = info.BaseAddress as usize + info.RegionSize;
        }
        // SAFETY: a first call with an empty buffer returns the heap count.
        let count = unsafe { GetProcessHeaps(&mut []) } as usize;
        let mut heaps = vec![Default::default(); count + 16];
        // SAFETY: `heaps` is large enough for the returned handles.
        let count = unsafe { GetProcessHeaps(&mut heaps) } as usize;
        for heap in &heaps[..count.min(heaps.len())] {
            let mut summary = HEAP_SUMMARY {
                cb: size_of::<HEAP_SUMMARY>() as u32,
                ..Default::default()
            };
            // SAFETY: the handle comes from GetProcessHeaps and `summary` is sized by `cb`.
            if unsafe { HeapSummary(*heap, 0, &mut summary) }.as_bool() {
                snapshot.heaps += summary.cbCommitted as u64;
            }
        }
        snapshot
    }

    fn private_blocks_total(&self) -> u64 {
        self.private_blocks.values().sum()
    }
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1e6
}

fn signed_mb(after: u64, before: u64) -> f64 {
    (after as f64 - before as f64) / 1e6
}

fn report(step: &str, before: &Snapshot, after: &Snapshot) {
    println!(
        "{step:<34} privée {:>7.1} Mo ({:+7.1}), working set {:>7.1} Mo ({:+7.1}) | tas {:>6.1} ({:+6.1}), VirtualAlloc privé {:>6.1} ({:+6.1}), image {:>5.1}, mappé {:>6.1} ({:+6.1})",
        mb(after.private),
        signed_mb(after.private, before.private),
        mb(after.working_set),
        signed_mb(after.working_set, before.working_set),
        mb(after.heaps),
        signed_mb(after.heaps, before.heaps),
        mb(after.private_blocks_total()),
        signed_mb(after.private_blocks_total(), before.private_blocks_total()),
        mb(after.image),
        mb(after.mapped),
        signed_mb(after.mapped, before.mapped),
    );
    let mut by_size: BTreeMap<u64, usize> = BTreeMap::new();
    for (base, size) in &after.private_blocks {
        let grown = size.saturating_sub(before.private_blocks.get(base).copied().unwrap_or(0));
        if grown >= 1 << 20 {
            *by_size.entry(grown / 100_000).or_default() += 1;
        }
    }
    let sizes: Vec<String> = by_size
        .iter()
        .rev()
        .map(|(size, count)| format!("{:.1} × {count}", *size as f64 / 10.0))
        .collect();
    if !sizes.is_empty() {
        println!(
            "{:<34} blocs privés ≥ 1 Mo qui ont grandi (Mo × nombre) : {}",
            "",
            sizes.join(", ")
        );
    }
}

fn step(name: &str, previous: &mut Snapshot) {
    std::thread::sleep(SETTLE);
    let now = Snapshot::take();
    report(name, previous, &now);
    *previous = now;
}

pub fn probe_memory(args: &ProbeMemoryArgs) -> Result<()> {
    let mut last = Snapshot::take();
    report("démarrage", &last, &last);
    let settings = args.video.video_settings();
    let device = create_device()?;
    step("périphérique D3D11", &mut last);
    let encoders = hardware_video_encoders()?;
    step("sondage des encodeurs", &mut last);
    drop(device);
    step("périphérique D3D11 libéré", &mut last);
    let encoder = pick_encoder(&encoders, settings.codec)?;
    let capture = VideoCapture::start(&args.video.target, &settings, &encoder.name)?;
    let video_packets = capture.packets().clone();
    let drain = std::thread::spawn(move || while video_packets.recv().is_ok() {});
    step("capture WGC + encodeur", &mut last);
    let origin = capture.stream().origin;
    let mut sources = vec![AudioSource::SystemMix { device_id: None }];
    sources.extend(args.apps.iter().map(|exe_name| AudioSource::Application {
        exe_name: exe_name.clone(),
    }));
    sources.push(AudioSource::Game(CurrentGame::new(
        None,
        &args.video.target,
    )));
    let mut tracks = Vec::new();
    let mut drains = Vec::new();
    for (index, source) in sources.into_iter().enumerate() {
        let title = source.label();
        let track = AudioTrack::start(index, source, title, origin, Hns::ZERO, Mixdown::None)?;
        let packets = track.packets().clone();
        drains.push(std::thread::spawn(move || while packets.recv().is_ok() {}));
        tracks.push(track);
    }
    step(&format!("+ {} pistes audio", tracks.len()), &mut last);
    step("régime (5 s de plus)", &mut last);
    let now = Hns::now();
    for track in tracks {
        track.stop(now)?;
    }
    capture.stop()?;
    drain.join().ok();
    for drain in drains {
        drain.join().ok();
    }
    step("tout arrêté", &mut last);
    Ok(())
}
