use anyhow::Result;
use sonixx_core::audio::wasapi::CaptureStream;

fn main() -> Result<()> {
    let pid: u32 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0".to_string())
        .parse()
        .unwrap_or(0);
    println!("abrindo process loopback para PID {pid}...");
    match CaptureStream::open_process(pid) {
        Ok(stream) => println!(
            "OK: {} Hz, {} canais, {} bits",
            stream.format.sample_rate, stream.format.channels, stream.format.bits_per_sample
        ),
        Err(error) => println!("FAIL: {error:#}"),
    }
    Ok(())
}
