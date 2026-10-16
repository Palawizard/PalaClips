use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use palaclips_engine::clock::{HNS_PER_SECOND, Hns};
use wasapi::{
    DeviceEnumerator, DeviceEventCallbacks, Direction, Role, SampleType, StreamMode, WasapiError,
    WaveFormat, initialize_mta,
};

const SAMPLE_RATE: i64 = 48_000;
const BYTES_PER_FRAME: usize = 8;
const EVENT_TIMEOUT_MS: u32 = 500;

#[derive(Debug, Default)]
struct Second {
    packets: u64,
    frames: u64,
    silent: u64,
    zero_without_flag: u64,
    discontinuities: u64,
    timestamp_errors: u64,
    timeouts: u64,
    peak: f32,
    max_hole_ms: f64,
    max_overlap_ms: f64,
    min_latency_ms: Option<f64>,
    max_latency_ms: f64,
}

#[derive(Debug)]
struct Previous {
    qpc: Hns,
    index: u64,
    frames: u64,
}

#[derive(Debug, Default)]
struct StreamTotals {
    first_qpc: Option<Hns>,
    end_qpc: Hns,
    frames: u64,
    index_jumps: u64,
}

pub fn probe_audio(mic: bool, seconds: u64) -> Result<()> {
    initialize_mta()
        .ok()
        .map_err(|error| anyhow!("COM : {error}"))?;
    let direction = if mic {
        Direction::Capture
    } else {
        Direction::Render
    };
    let start = Hns::now();
    let deadline = start + Hns(seconds as i64 * HNS_PER_SECOND);
    let default_changed = Arc::new(AtomicBool::new(false));

    let enumerator = DeviceEnumerator::new()?;
    let mut callbacks = DeviceEventCallbacks::new();
    callbacks.set_default_device_callback({
        let default_changed = default_changed.clone();
        move |changed_direction, role, id| {
            println!(
                "[{:>8.3} s] notification : nouveau défaut {changed_direction} / {role} → {}",
                elapsed(start),
                id.as_deref().unwrap_or("aucun")
            );
            if changed_direction == direction && role == Role::Console {
                default_changed.store(true, Ordering::Relaxed);
            }
        }
    });
    callbacks.set_device_state_callback(move |id, state| {
        println!("[{:>8.3} s] notification : {id} → {state}", elapsed(start));
    });
    callbacks.set_device_removed_callback(move |id| {
        println!("[{:>8.3} s] notification : retiré {id}", elapsed(start));
    });
    let _registration = enumerator.register_notification_callback(callbacks)?;

    while Hns::now() < deadline {
        default_changed.store(false, Ordering::Relaxed);
        if let Err(error) = run_stream(&enumerator, direction, start, deadline, &default_changed) {
            println!("[{:>8.3} s] flux interrompu : {error}", elapsed(start));
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    Ok(())
}

fn run_stream(
    enumerator: &DeviceEnumerator,
    direction: Direction,
    start: Hns,
    deadline: Hns,
    default_changed: &AtomicBool,
) -> Result<()> {
    let device = enumerator.get_default_device(&direction)?;
    let mut client = device.get_iaudioclient()?;
    let mix_format = client.get_mixformat()?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, SAMPLE_RATE as usize, 2, None);
    client.initialize_client(
        &format,
        &Direction::Capture,
        &StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 2_000_000,
        },
    )?;
    let event = client.set_get_eventhandle()?;
    let capture = client.get_audiocaptureclient()?;
    let (default_period, _) = client.get_device_period()?;
    // SAFETY: GetStreamLatency only reads the state of an initialized audio client.
    let stream_latency = unsafe { client.as_iaudioclient().GetStreamLatency() }.unwrap_or(-1);
    println!(
        "[{:>8.3} s] ouvert : « {} » ({}), format du moteur {} Hz × {} canaux, période {:.1} ms, latence du flux {:.1} ms, tampon {} images",
        elapsed(start),
        device.get_friendlyname()?,
        if direction == Direction::Render {
            "loopback"
        } else {
            "micro"
        },
        mix_format.get_samplespersec(),
        mix_format.get_nchannels(),
        default_period as f64 / 10_000.0,
        stream_latency as f64 / 10_000.0,
        client.get_buffer_size()?
    );
    client.start_stream()?;

    let mut buffer = Vec::new();
    let mut second = Second::default();
    let mut second_index = (Hns::now() - start).0 / HNS_PER_SECOND;
    let mut previous: Option<Previous> = None;
    let mut totals = StreamTotals::default();
    let result = loop {
        if Hns::now() >= deadline {
            break Ok(());
        }
        if default_changed.load(Ordering::Relaxed) {
            break Err(anyhow!("le périphérique par défaut a changé, réouverture"));
        }
        match event.wait_for_event(EVENT_TIMEOUT_MS) {
            Ok(()) => {}
            Err(WasapiError::EventTimeout) => second.timeouts += 1,
            Err(error) => break Err(error.into()),
        }
        if let Err(error) = drain(
            &capture,
            &mut buffer,
            &mut second,
            &mut previous,
            &mut totals,
        ) {
            break Err(error);
        }
        let now_index = (Hns::now() - start).0 / HNS_PER_SECOND;
        if now_index != second_index {
            print_second(second_index, &second);
            second = Second::default();
            second_index = now_index;
        }
    };
    print_second(second_index, &second);
    print_totals(&totals);
    client.stop_stream().ok();
    result
}

fn drain(
    capture: &wasapi::AudioCaptureClient,
    buffer: &mut Vec<u8>,
    second: &mut Second,
    previous: &mut Option<Previous>,
    totals: &mut StreamTotals,
) -> Result<()> {
    while let Some(frames) = capture.get_next_packet_size()?.filter(|frames| *frames > 0) {
        buffer.resize(frames as usize * BYTES_PER_FRAME, 0);
        let (read, info) = capture.read_from_device(buffer)?;
        let received = Hns::now();
        let qpc = Hns(info.timestamp as i64);
        let read = u64::from(read);
        second.packets += 1;
        second.frames += read;
        second.silent += u64::from(info.flags.silent);
        second.discontinuities += u64::from(info.flags.data_discontinuity);
        second.timestamp_errors += u64::from(info.flags.timestamp_error);
        let samples = buffer[..read as usize * BYTES_PER_FRAME]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes).abs());
        let peak = samples.fold(0.0, f32::max);
        if !info.flags.silent && peak == 0.0 {
            second.zero_without_flag += 1;
        }
        second.peak = second.peak.max(peak);
        let latency = ms(received - qpc);
        second.max_latency_ms = second.max_latency_ms.max(latency);
        second.min_latency_ms = Some(
            second
                .min_latency_ms
                .map_or(latency, |min| min.min(latency)),
        );
        if let Some(previous) = previous.as_ref() {
            let expected = previous.qpc + duration_of(previous.frames);
            let gap = ms(qpc - expected);
            second.max_hole_ms = second.max_hole_ms.max(gap);
            second.max_overlap_ms = second.max_overlap_ms.max(-gap);
            if info.index != previous.index + previous.frames {
                totals.index_jumps += 1;
            }
        }
        totals.first_qpc.get_or_insert(qpc);
        totals.end_qpc = qpc + duration_of(read);
        totals.frames += read;
        *previous = Some(Previous {
            qpc,
            index: info.index,
            frames: read,
        });
    }
    Ok(())
}

fn print_second(index: i64, second: &Second) {
    println!(
        "[{index:>6} s] paquets {:>4}  images {:>6}  SILENT {:>4}  zéro sans drapeau {:>3}  discont. {}  err. horodatage {}  attentes vides {}  crête {:.3}  trou max {:.2} ms  chevauchement max {:.2} ms  latence {:.1} à {:.1} ms",
        second.packets,
        second.frames,
        second.silent,
        second.zero_without_flag,
        second.discontinuities,
        second.timestamp_errors,
        second.timeouts,
        second.peak,
        second.max_hole_ms,
        second.max_overlap_ms,
        second.min_latency_ms.unwrap_or_default(),
        second.max_latency_ms
    );
}

fn print_totals(totals: &StreamTotals) {
    let Some(first) = totals.first_qpc else {
        println!("bilan du flux : aucun paquet reçu");
        return;
    };
    let span = totals.end_qpc - first;
    let expected = span.0 as f64 * SAMPLE_RATE as f64 / HNS_PER_SECOND as f64;
    println!(
        "bilan du flux : {} images sur {:.3} s de QPC (attendu {expected:.0}, écart {:+.0} ppm), sauts d'index {}",
        totals.frames,
        span.as_secs_f64(),
        (totals.frames as f64 - expected) / expected.max(1.0) * 1e6,
        totals.index_jumps
    );
}

fn duration_of(frames: u64) -> Hns {
    Hns(frames as i64 * HNS_PER_SECOND / SAMPLE_RATE)
}

fn elapsed(start: Hns) -> f64 {
    (Hns::now() - start).as_secs_f64()
}

fn ms(value: Hns) -> f64 {
    value.0 as f64 / 10_000.0
}
