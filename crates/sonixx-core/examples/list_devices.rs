use anyhow::Result;
use sonixx_core::audio::{list_devices, DeviceFlow};

fn main() -> Result<()> {
    for (label, flow) in [
        ("SAÍDA", DeviceFlow::Render),
        ("ENTRADA", DeviceFlow::Capture),
    ] {
        println!("== {label} ==");
        for device in list_devices(flow)? {
            println!("  {}\n    {}", device.name, device.id);
        }
    }
    println!(
        "\nVB-Cable instalado: {}",
        sonixx_core::audio::wasapi::is_vb_cable_installed()?
    );
    Ok(())
}
