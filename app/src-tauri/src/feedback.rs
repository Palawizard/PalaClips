use std::sync::OnceLock;

use tauri_plugin_notification::NotificationExt;

const SAMPLE_RATE: u32 = 48_000;
const VOLUME: f32 = 0.22;
const NOTES: [(f32, f32, f32); 2] = [(880.0, 0.0, 0.11), (1318.5, 0.075, 0.2)];

fn chime_samples() -> Vec<i16> {
    let length = NOTES
        .iter()
        .map(|(_, start, duration)| start + duration)
        .fold(0.0, f32::max);
    let count = (length * SAMPLE_RATE as f32) as usize;
    (0..count)
        .map(|index| {
            let time = index as f32 / SAMPLE_RATE as f32;
            let value: f32 = NOTES
                .iter()
                .filter(|(_, start, duration)| time >= *start && time < start + duration)
                .map(|(frequency, start, duration)| {
                    let local = time - start;
                    let attack = (local / 0.004).min(1.0);
                    let release = ((duration - local) / 0.03).min(1.0);
                    let decay = (-local * 9.0).exp();
                    (local * frequency * std::f32::consts::TAU).sin() * attack * release * decay
                })
                .sum();
            (value * VOLUME * f32::from(i16::MAX)) as i16
        })
        .collect()
}

fn wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(windows)]
pub fn play_chime() {
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    use windows::core::PCWSTR;

    static CHIME: OnceLock<Vec<u8>> = OnceLock::new();
    let chime = CHIME.get_or_init(|| wav(&chime_samples()));
    // SAFETY: SND_MEMORY reads the WAV from `chime`, a static buffer that outlives the
    // asynchronous playback.
    let played = unsafe {
        PlaySoundW(
            PCWSTR(chime.as_ptr().cast()),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        )
    };
    if !played.as_bool() {
        tracing::warn!("son de confirmation non joué");
    }
}

pub fn toast(app: &tauri::AppHandle, title: &str, body: &str) {
    if let Err(error) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(%error, "notification non affichée");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chime_is_a_short_mono_wav_below_full_scale() {
        let samples = chime_samples();
        let seconds = samples.len() as f32 / SAMPLE_RATE as f32;
        assert!((0.2..0.35).contains(&seconds), "{seconds}");
        let peak = samples
            .iter()
            .map(|sample| sample.unsigned_abs())
            .max()
            .unwrap_or(0);
        assert!(peak > 1_000 && peak < i16::MAX as u16 / 2, "{peak}");
        let file = wav(&samples);
        assert_eq!(&file[..4], b"RIFF");
        assert_eq!(file.len(), 44 + samples.len() * 2);
    }
}
