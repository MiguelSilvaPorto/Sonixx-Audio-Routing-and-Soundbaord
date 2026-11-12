use anyhow::Result;
use sonixx_core::audio::wasapi::{default_device, list_devices, DeviceFlow};
use sonixx_core::audio::{AppSource, Engine, EngineConfig, SourceKind};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<()> {
    let pid: u32 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0".to_string())
        .parse()
        .unwrap_or(0);
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
            pid,
            name: format!("Processo {pid}"),
            exe: None,
        }],
        mic_devices: Vec::new(),
        master_volume: 0.5,
        master_muted: false,
    })?;

    for step in 0..10 {
        sleep(Duration::from_millis(500));
        let apps: Vec<String> = engine
            .sources()
            .iter()
            .filter(|source| source.kind == SourceKind::App)
            .map(|source| format!("{} peak={:.3}", source.name, source.peak()))
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
