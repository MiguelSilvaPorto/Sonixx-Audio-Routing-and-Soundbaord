use anyhow::Result;
use sonixx_core::audio::{decode_audio, default_output_peak, list_audio_sessions};
use std::path::Path;

fn main() -> Result<()> {
    println!("endpoint peak: {:.3}", default_output_peak()?);
    println!("== sessões de áudio ==");
    let sessions = list_audio_sessions()?;
    if sessions.is_empty() {
        println!(" (nenhuma)");
    }
    for session in sessions {
        let exe = session
            .exe
            .as_deref()
            .and_then(|path| std::path::Path::new(path).file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "?".to_string());
        println!(
            " pid={:<6} peak={:.3} sessions={} exe={:<24} key={}  {}",
            session.pid, session.peak, session.sessions, exe, session.key, session.name
        );
    }

    if let Some(path) = std::env::args().nth(1) {
        let samples = decode_audio(Path::new(&path), 48_000)?;
        let peak = samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        println!(
            "\ndecodificado: {} amostras ({} frames stereo), pico {:.3}",
            samples.len(),
            samples.len() / 2,
            peak
        );
    }
    Ok(())
}
