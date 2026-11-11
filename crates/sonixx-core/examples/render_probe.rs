use anyhow::Result;
use sonixx_core::audio::wasapi::{list_devices, DeviceFlow, RenderStream};

fn main() -> Result<()> {
    for device in list_devices(DeviceFlow::Render)? {
        match RenderStream::open(&device.id) {
            Ok(stream) => println!(
                "OK   {}  ({} Hz, {} ch, buffer={})",
                device.name, stream.format.sample_rate, stream.format.channels, stream.buffer_size
            ),
            Err(error) => println!("FAIL {} -> {error:#}", device.name),
        }
    }
    Ok(())
}
