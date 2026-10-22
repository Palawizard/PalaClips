use std::mem::ManuallyDrop;

use anyhow::Result;
use windows::Win32::Media::MediaFoundation::{
    CMSH264EncoderMFT, CODECAPI_AVEncMPVDefaultBPictureCount, CODECAPI_AVEncNumWorkerThreads,
    CODECAPI_AVEncVideoMaxNumRefFrame, CODECAPI_AVLowLatencyMode, ICodecAPI, IMFTransform,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_VERSION, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFMediaType_Video, MFSTARTUP_FULL, MFStartup,
    MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_OUTPUT_DATA_BUFFER,
    MFVideoFormat_H264, MFVideoFormat_NV12, MFVideoInterlace_Progressive,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_UI4};
use windows::core::{GUID, Interface};

#[derive(Debug, clap::Args)]
pub struct ProbeMfArgs {
    #[arg(long, default_value_t = 1920)]
    width: u32,
    #[arg(long, default_value_t = 1080)]
    height: u32,
    #[arg(long, default_value_t = 90)]
    frames: i64,
    #[arg(long, help = "CODECAPI_AVEncNumWorkerThreads")]
    workers: Option<u32>,
    #[arg(long, help = "CODECAPI_AVEncVideoMaxNumRefFrame")]
    ref_frames: Option<u32>,
    #[arg(long, help = "CODECAPI_AVLowLatencyMode")]
    low_latency: bool,
    #[arg(
        long,
        help = "Nombre de fils posé après les types, avant la première image"
    )]
    workers_late: Option<u32>,
    #[arg(long, help = "Images NV12 brutes à encoder (au lieu d'images vides)")]
    input: Option<std::path::PathBuf>,
    #[arg(long, help = "Flux H.264 (Annex B) écrit ici")]
    out: Option<std::path::PathBuf>,
}

fn private_mb() -> f64 {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    // SAFETY: the pseudo-handle of the current process is valid and `counters` is sized by `cb`.
    let _ = unsafe {
        GetProcessMemoryInfo(GetCurrentProcess(), (&raw mut counters).cast(), counters.cb)
    };
    counters.PrivateUsage as f64 / 1_048_576.0
}

fn uint(value: u32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_UI4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { ulVal: value },
            }),
        },
    }
}

fn set(api: &ICodecAPI, name: &str, key: &GUID, value: u32) {
    // SAFETY: the GUID and VARIANT outlive the call.
    let outcome = unsafe { api.SetValue(key, &uint(value)) };
    println!(
        "  {name} = {value} : {}",
        if outcome.is_ok() {
            "accepté"
        } else {
            "refusé"
        }
    );
}

pub fn probe_mf(args: &ProbeMfArgs) -> Result<()> {
    let start = private_mb();
    // SAFETY: plain COM and Media Foundation initialisation for this thread and process.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
    }
    // SAFETY: CMSH264EncoderMFT is the in-process Microsoft H.264 encoder.
    let mft: IMFTransform =
        unsafe { CoCreateInstance(&CMSH264EncoderMFT, None, CLSCTX_INPROC_SERVER)? };
    let api: ICodecAPI = mft.cast()?;
    println!(
        "MFT créé : privée {:.0} Mo (+{:.0})",
        private_mb(),
        private_mb() - start
    );
    set(
        &api,
        "AVEncMPVDefaultBPictureCount",
        &CODECAPI_AVEncMPVDefaultBPictureCount,
        0,
    );
    if let Some(workers) = args.workers {
        set(
            &api,
            "AVEncNumWorkerThreads",
            &CODECAPI_AVEncNumWorkerThreads,
            workers,
        );
    }
    if let Some(refs) = args.ref_frames {
        set(
            &api,
            "AVEncVideoMaxNumRefFrame",
            &CODECAPI_AVEncVideoMaxNumRefFrame,
            refs,
        );
    }
    if args.low_latency {
        set(&api, "AVLowLatencyMode", &CODECAPI_AVLowLatencyMode, 1);
    }
    let size = (u64::from(args.width) << 32) | u64::from(args.height);
    let rate = (30u64 << 32) | 1;
    // SAFETY: media types are fresh COM objects configured before being handed to the MFT.
    unsafe {
        let output = MFCreateMediaType()?;
        output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        output.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
        output.SetUINT32(&MF_MT_AVG_BITRATE, 11_200_000)?;
        output.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
        output.SetUINT64(&MF_MT_FRAME_RATE, rate)?;
        output.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        mft.SetOutputType(0, &output, 0)?;
        let input = MFCreateMediaType()?;
        input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
        input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_NV12)?;
        input.SetUINT64(&MF_MT_FRAME_SIZE, size)?;
        input.SetUINT64(&MF_MT_FRAME_RATE, rate)?;
        input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
        mft.SetInputType(0, &input, 0)?;
        if let Some(workers) = args.workers_late {
            set(
                &api,
                "AVEncNumWorkerThreads (tard)",
                &CODECAPI_AVEncNumWorkerThreads,
                workers,
            );
            let output = mft.GetOutputCurrentType(0)?;
            let input = mft.GetInputCurrentType(0)?;
            println!(
                "  types reposés : sortie {:?}, entrée {:?}",
                mft.SetOutputType(0, &output, 0).is_ok(),
                mft.SetInputType(0, &input, 0).is_ok()
            );
        }
        mft.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
        mft.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
    }
    println!(
        "types posés : privée {:.0} Mo (+{:.0})",
        private_mb(),
        private_mb() - start
    );
    let frame_bytes = args.width * args.height * 3 / 2;
    // SAFETY: stream 0 exists once the types are set.
    let output_bytes = unsafe { mft.GetOutputStreamInfo(0)? }
        .cbSize
        .max(frame_bytes);
    let mut packets = 0;
    let raw = args.input.as_ref().map(std::fs::read).transpose()?;
    let mut stream = Vec::new();
    let clock = std::time::Instant::now();
    let cpu_start = process_cpu_secs();
    for index in 0..args.frames {
        // SAFETY: samples and buffers are fresh COM objects sized for one NV12 frame.
        unsafe {
            let buffer = MFCreateMemoryBuffer(frame_bytes)?;
            if let Some(raw) = &raw {
                let count = raw.len() / frame_bytes as usize;
                let offset = (index as usize % count) * frame_bytes as usize;
                let mut data = std::ptr::null_mut();
                buffer.Lock(&mut data, None, None)?;
                std::ptr::copy_nonoverlapping(raw[offset..].as_ptr(), data, frame_bytes as usize);
                buffer.Unlock()?;
            }
            buffer.SetCurrentLength(frame_bytes)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            sample.SetSampleTime(index * 333_333)?;
            sample.SetSampleDuration(333_333)?;
            mft.ProcessInput(0, &sample, 0)?;
            loop {
                let out = MFCreateSample()?;
                out.AddBuffer(&MFCreateMemoryBuffer(output_bytes)?)?;
                let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
                    dwStreamID: 0,
                    pSample: ManuallyDrop::new(Some(out)),
                    dwStatus: 0,
                    pEvents: ManuallyDrop::new(None),
                }];
                let mut status = 0;
                let result = mft.ProcessOutput(0, &mut buffers, &mut status);
                if result.is_ok()
                    && let Some(sample) = buffers[0].pSample.as_ref()
                {
                    let buffer = sample.ConvertToContiguousBuffer()?;
                    let mut data = std::ptr::null_mut();
                    let mut length = 0;
                    buffer.Lock(&mut data, None, Some(&mut length))?;
                    stream.extend_from_slice(std::slice::from_raw_parts(data, length as usize));
                    buffer.Unlock()?;
                }
                ManuallyDrop::drop(&mut buffers[0].pSample);
                ManuallyDrop::drop(&mut buffers[0].pEvents);
                match result {
                    Ok(()) => packets += 1,
                    Err(error) if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => break,
                    Err(error) => return Err(error.into()),
                }
            }
        }
        if index == 0 || index + 1 == args.frames {
            println!(
                "{} image(s) : privée {:.0} Mo (+{:.0}), {packets} paquets",
                index + 1,
                private_mb(),
                private_mb() - start
            );
        }
    }
    let seconds = clock.elapsed().as_secs_f64();
    let video = args.frames as f64 / 30.0;
    println!(
        "{} images en {seconds:.2} s : vitesse {:.2}x, {:.2} cœur(s) au temps réel",
        args.frames,
        video / seconds,
        (process_cpu_secs() - cpu_start) / video
    );
    if let Some(out) = &args.out {
        std::fs::write(out, &stream)?;
    }
    Ok(())
}

fn process_cpu_secs() -> f64 {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::GetProcessTimes;
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: the pseudo-handle is valid and all out-pointers are local.
    let _ = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    };
    let ticks = |time: FILETIME| {
        (u64::from(time.dwHighDateTime) << 32 | u64::from(time.dwLowDateTime)) as f64
    };
    (ticks(kernel) + ticks(user)) / 10_000_000.0
}
