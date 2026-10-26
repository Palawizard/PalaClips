use std::path::Path;
use std::time::Duration;

use anyhow::{Result, anyhow};
use palaclips_engine::chime::{self, Sound};
use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
use wasapi::{
    AudioClient, DeviceEnumerator, Direction, SampleType, StreamMode, WasapiError, WaveFormat,
    initialize_mta,
};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentThread, GetThreadTimes};

const SAMPLE_RATE: i64 = 48_000;
const CHANNELS: usize = 2;
const EVENT_TIMEOUT_MS: u32 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Capture {
    Endpoint,
    ExceptSelf,
    SelfOnly,
    Process(u32),
}

impl Capture {
    fn file_name(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint.f32",
            Self::ExceptSelf => "except-self.f32",
            Self::SelfOnly => "self-only.f32",
            Self::Process(_) => "process.f32",
        }
    }
}

struct Captured {
    capture: Capture,
    name: String,
    samples: Vec<f32>,
    packets: u64,
    cpu_ms: f64,
}

pub fn probe_exclusion(
    seconds: u64,
    device: Option<&str>,
    chimes_at: &[f64],
    include_pid: Option<u32>,
    out: &Path,
) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let start = Hns::now() + Hns::from_millis(500);
    let deadline = start + Hns(seconds as i64 * HNS_PER_SECOND);
    let device = device.map(str::to_owned);
    let threads: Vec<_> = [Capture::Endpoint, Capture::ExceptSelf, Capture::SelfOnly]
        .into_iter()
        .chain(include_pid.map(Capture::Process))
        .map(|capture| {
            let device = device.clone();
            std::thread::spawn(move || run_capture(capture, device.as_deref(), start, deadline))
        })
        .collect();
    let mut chimes: Vec<f64> = chimes_at.to_vec();
    chimes.sort_by(f64::total_cmp);
    for at in chimes {
        let due = start + Hns::from_secs_f64(at);
        let wait = due - Hns::now();
        if wait.0 > 0 {
            std::thread::sleep(Duration::from_nanos(wait.0 as u64 * 100));
        }
        chime::play(Sound::RecordingStarted);
        println!("son de début d'enregistrement joué par ce processus à {at:.2} s");
    }
    for thread in threads {
        let captured = thread
            .join()
            .map_err(|_| anyhow!("fil de capture arrêté"))??;
        let path = out.join(captured.capture.file_name());
        let bytes: Vec<u8> = captured
            .samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        std::fs::write(&path, bytes)?;
        let peak = captured
            .samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        println!(
            "{:?} « {} » : {} paquets, crête {peak:.3}, processeur {:.1} ms → {}",
            captured.capture,
            captured.name,
            captured.packets,
            captured.cpu_ms,
            path.display()
        );
    }
    Ok(())
}

fn run_capture(
    capture: Capture,
    device: Option<&str>,
    start: Hns,
    deadline: Hns,
) -> Result<Captured> {
    initialize_mta()
        .ok()
        .map_err(|error| anyhow!("COM : {error}"))?;
    let (mut client, name) = match capture {
        Capture::Endpoint => {
            let device = match device {
                Some(name) => crate::probe_audio::find_device(Direction::Render, name)?,
                None => DeviceEnumerator::new()?.get_default_device(&Direction::Render)?,
            };
            (device.get_iaudioclient()?, device.get_friendlyname()?)
        }
        Capture::ExceptSelf => (
            AudioClient::new_application_loopback_client(std::process::id(), false)?,
            format!("tout sauf le PID {}", std::process::id()),
        ),
        Capture::SelfOnly => (
            AudioClient::new_application_loopback_client(std::process::id(), true)?,
            format!("seulement le PID {}", std::process::id()),
        ),
        Capture::Process(pid) => (
            AudioClient::new_application_loopback_client(pid, true)?,
            format!("seulement le PID {pid}"),
        ),
    };
    client.initialize_client(
        &WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            SAMPLE_RATE as usize,
            CHANNELS,
            None,
        ),
        &Direction::Capture,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 1_000_000,
        },
    )?;
    let event = client.set_get_eventhandle()?;
    let reader = client.get_audiocaptureclient()?;
    let total = (deadline - start).to_samples(SAMPLE_RATE as u32) as usize;
    let mut samples = vec![0.0f32; total];
    let mut packets = 0;
    let mut buffer = Vec::new();
    let cpu_before = thread_cpu_ms();
    client.start_stream()?;
    while Hns::now() < deadline + Hns::from_millis(300) {
        match event.wait_for_event(EVENT_TIMEOUT_MS) {
            Ok(()) | Err(WasapiError::EventTimeout) => {}
            Err(error) => return Err(error.into()),
        }
        while let Some(frames) = reader.get_next_packet_size()?.filter(|frames| *frames > 0) {
            buffer.resize(frames as usize * CHANNELS * 4, 0);
            let (read, info) = reader.read_from_device(&mut buffer)?;
            packets += 1;
            let first = (Hns(info.timestamp as i64) - start).to_samples(SAMPLE_RATE as u32);
            for (offset, frame) in buffer[..read as usize * CHANNELS * 4]
                .as_chunks::<8>()
                .0
                .iter()
                .enumerate()
            {
                let index = first + offset as i64;
                if index < 0 || index as usize >= total || info.flags.silent {
                    continue;
                }
                let left = f32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
                let right = f32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
                samples[index as usize] = (left + right) / 2.0;
            }
        }
    }
    client.stop_stream().ok();
    Ok(Captured {
        capture,
        name,
        samples,
        packets,
        cpu_ms: thread_cpu_ms() - cpu_before,
    })
}

fn thread_cpu_ms() -> f64 {
    let mut times = [FILETIME::default(); 4];
    let [creation, exit, kernel, user] = &mut times;
    // SAFETY: GetCurrentThread returns a pseudo-handle valid for the calling thread, and the
    // four FILETIME outputs live on this stack frame.
    let read = unsafe { GetThreadTimes(GetCurrentThread(), creation, exit, kernel, user) };
    if read.is_err() {
        return 0.0;
    }
    let ticks =
        |time: &FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    (ticks(kernel) + ticks(user)) as f64 / 10_000.0
}
