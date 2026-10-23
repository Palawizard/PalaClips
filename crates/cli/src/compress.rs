use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use anyhow::Result;
use clap::{Args, ValueEnum};
use palaclips_engine::encode::video_encoders;
use palaclips_engine::library::{
    CompressCodec, CompressRequest, compress, compressed_name, plan_compression, probe_source,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CodecArg {
    H264,
    Av1,
}

#[derive(Debug, Args)]
pub struct CompressArgs {
    #[arg(help = "Vidéo à compresser (mp4, mkv, mov…)")]
    pub input: PathBuf,
    #[arg(
        long,
        default_value_t = 20,
        help = "Taille cible en Mo (1 Mo = 1 000 000 octets)"
    )]
    pub target_mb: u64,
    #[arg(long, value_enum, default_value_t = CodecArg::H264, help = "Codec vidéo")]
    pub codec: CodecArg,
    #[arg(long, help = "Garder toutes les pistes audio (sinon la piste 1 seule)")]
    pub all_tracks: bool,
    #[arg(long, default_value_t = 0.0, help = "Début en secondes")]
    pub start: f64,
    #[arg(long, help = "Fin en secondes ; absente = fin du fichier")]
    pub end: Option<f64>,
    #[arg(
        long,
        help = "Fichier de sortie ; absent = à côté de l'original, suffixe _<N>MB"
    )]
    pub out: Option<PathBuf>,
    #[arg(long, help = "Affiche le plan sans encoder")]
    pub plan_only: bool,
}

pub fn run(args: &CompressArgs) -> Result<()> {
    let source = probe_source(&args.input)?;
    let end = args
        .end
        .unwrap_or(source.duration_secs)
        .min(source.duration_secs);
    let duration = end - args.start.max(0.0);
    let target = args.target_mb * 1_000_000;
    let codec = match args.codec {
        CodecArg::H264 => CompressCodec::H264,
        CodecArg::Av1 => CompressCodec::Av1,
    };
    let plan = plan_compression(&source, target, codec, args.all_tracks, duration)?;
    println!(
        "source : {}×{} à {:.2} i/s, {:.1} s, {} piste(s) audio, {:.1} Mo, HDR {}",
        source.width,
        source.height,
        source.fps,
        source.duration_secs,
        source.audio_tracks.len(),
        source.size_bytes as f64 / 1e6,
        if source.hdr { "oui" } else { "non" }
    );
    println!(
        "plan : {}×{} à {} i/s, vidéo {} kb/s, audio {} × {} kb/s, qualité {:?}, taille estimée {:.2} Mo pour {} Mo",
        plan.width,
        plan.height,
        plan.fps,
        plan.video_kbps,
        plan.audio_tracks,
        plan.audio_kbps,
        plan.quality(codec),
        plan.estimated_bytes(duration) as f64 / 1e6,
        args.target_mb
    );
    if args.plan_only {
        return Ok(());
    }
    let request = CompressRequest {
        output: args
            .out
            .clone()
            .unwrap_or_else(|| compressed_name(&args.input, target)),
        source: args.input.clone(),
        target_bytes: target,
        codec,
        all_tracks: args.all_tracks,
        start_secs: args.start,
        end_secs: args.end,
        plan,
    };
    let started = Instant::now();
    let mut shown = (0, -1);
    let result = compress(
        &request,
        &video_encoders()?,
        &mut |step| {
            let tenth = (step.fraction * 10.0).floor() as i32;
            if (step.attempt, tenth) != shown {
                shown = (step.attempt, tenth);
                println!("essai {} : {:>3} %", step.attempt, tenth * 10);
            }
        },
        &AtomicBool::new(false),
    )?;
    println!(
        "RÉSULTAT : {} octets ({:.2} Mo) pour une cible de {} octets, {} essai(s), {:.1} s, vidéo finale {} kb/s → {}",
        result.bytes,
        result.bytes as f64 / 1e6,
        target,
        result.attempts,
        started.elapsed().as_secs_f64(),
        result.plan.video_kbps,
        result.path
    );
    Ok(())
}
