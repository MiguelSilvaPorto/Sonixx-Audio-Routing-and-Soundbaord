use anyhow::Result;
use sonixx_core::audio::{list_audio_sessions, set_sessions_muted, set_sessions_volume};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<()> {
    let sessions = list_audio_sessions()?;
    let target = sessions
        .iter()
        .find(|session| session.name.to_lowercase().contains("chrome"))
        .or_else(|| sessions.first());
    let Some(target) = target else {
        println!("nenhuma sessão encontrada");
        return Ok(());
    };

    println!(
        "target: {} pid={} pids={:?} volume={:.2} muted={}",
        target.name, target.pid, target.pids, target.volume, target.muted
    );
    let original = target.volume;
    set_sessions_volume(&target.pids, 0.5)?;
    sleep(Duration::from_millis(400));
    let after = list_audio_sessions()?
        .into_iter()
        .find(|session| session.key == target.key);
    println!("após set 0.50: {:?}", after.as_ref().map(|s| s.volume));

    set_sessions_muted(&target.pids, true)?;
    sleep(Duration::from_millis(300));
    let muted = list_audio_sessions()?
        .into_iter()
        .find(|session| session.key == target.key);
    println!("após mute: {:?}", muted.as_ref().map(|s| s.muted));

    set_sessions_muted(&target.pids, false)?;
    set_sessions_volume(&target.pids, original)?;
    println!("restaurado para {:.2}", original);
    Ok(())
}
