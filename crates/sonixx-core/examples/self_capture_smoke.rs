use anyhow::Result;
use sonixx_core::audio::wasapi::{default_device, list_devices, DeviceFlow};
use sonixx_core::audio::{AppSource, Engine, EngineConfig, SourceKind};
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<()> {
    let sound = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "sounds/test-tone.wav".to_string());
    let outputs = list_devices(DeviceFlow::Render)?;
    let loopback = default_device(DeviceFlow::Render)?;
    let output = outputs
        .iter()
        .find(|device| device.name.to_lowercase().contains("hdmi"))
        .or_else(|| outputs.iter().find(|device| device.id != loopback.id))
        .cloned()
        .unwrap_or_else(|| loopback.clone());

    let mut engine = Engine::new();
    engine.start(EngineConfig {
        output_device: output,
        loopback_device: loopback,
        app_sources: vec![AppSource {
            pid: std::process::id(),
            name: "Sonixx (eu mesmo)".to_string(),
        }],
        mic_devices: Vec::new(),
        master_volume: 0.5,
        master_muted: false,
    })?;
    sleep(Duration::from_millis(500));
    engine.play_sound(Path::new(&sound))?;

    for step in 0..10 {
        sleep(Duration::from_millis(400));
        let apps: Vec<String> = engine
            .sources()
            .iter()
            .filter(|source| source.kind == SourceKind::App)
            .map(|source| format!("peak={:.3}", source.peak()))
            .collect();
        println!(
            "{step:02} master={:.3}  {}",
            engine.master_peak(),
            apps.join(" ")
        );
    }
    if let Some(error) = engine.error() {
        println!("engine error: {error}");
    }
    engine.stop();
    Ok(())
}
