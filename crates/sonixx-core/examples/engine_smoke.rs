use anyhow::Result;
use sonixx_core::audio::wasapi::{default_device, list_devices, DeviceFlow};
use sonixx_core::audio::{Engine, EngineConfig};
use std::thread::sleep;
use std::time::Duration;

fn main() -> Result<()> {
    let outputs = list_devices(DeviceFlow::Render)?;
    let loopback = default_device(DeviceFlow::Render)?;
    let output = outputs
        .iter()
        .find(|device| device.name.to_lowercase().contains("hdmi"))
        .or_else(|| outputs.iter().find(|device| device.id != loopback.id))
        .cloned()
        .unwrap_or_else(|| loopback.clone());

    println!("loopback: {}\noutput:   {}", loopback.name, output.name);

    let mut engine = Engine::new();
    engine.start(EngineConfig {
        output_device: output,
        loopback_device: loopback,
        mic_devices: Vec::new(),
        master_volume: 0.05,
        master_muted: false,
    })?;

    for step in 0..12 {
        sleep(Duration::from_millis(500));
        let peaks: Vec<String> = engine
            .sources()
            .iter()
            .map(|source| format!("{}={:.3}", source.name, source.peak()))
            .collect();
        println!(
            "{step:02} master={:.3}  {}",
            engine.master_peak(),
            peaks.join("  ")
        );
    }
    if let Some(error) = engine.error() {
        println!("ERROR: {error}");
    }
    engine.stop();
    Ok(())
}
