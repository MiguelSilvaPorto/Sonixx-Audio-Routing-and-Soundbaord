use anyhow::Result;
use sonixx_core::audio::wasapi::{list_devices, CaptureStream, DeviceFlow};

fn main() -> Result<()> {
    println!("== loopback em cada saída ==");
    for device in list_devices(DeviceFlow::Render)? {
        match CaptureStream::open(&device.id, true) {
            Ok(stream) => println!(
                "OK   {}  ({} Hz, {} ch, {} bits)",
                device.name,
                stream.format.sample_rate,
                stream.format.channels,
                stream.format.bits_per_sample
            ),
            Err(error) => println!("FAIL {}  -> {error:#}", device.name),
        }
    }

    println!("== captura direta em cada entrada ==");
    for device in list_devices(DeviceFlow::Capture)? {
        match CaptureStream::open(&device.id, false) {
            Ok(stream) => println!(
                "OK   {}  ({} Hz, {} ch, {} bits)",
                device.name,
                stream.format.sample_rate,
                stream.format.channels,
                stream.format.bits_per_sample
            ),
            Err(error) => println!("FAIL {}  -> {error:#}", device.name),
        }
    }
    Ok(())
}
