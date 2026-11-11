use crate::audio::wasapi::{AudioDevice, CaptureStream, RenderStream};
use anyhow::{bail, Context, Result};
use ringbuf::{traits::*, HeapRb};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    System,
    Mic,
}

#[derive(Clone)]
pub struct SourceControl {
    pub id: u64,
    pub name: String,
    pub detail: String,
    pub kind: SourceKind,
    volume: Arc<AtomicU32>,
    active: Arc<AtomicBool>,
    muted: Arc<AtomicBool>,
    peak: Arc<AtomicU32>,
    sample_rate: Arc<AtomicU32>,
}

impl SourceControl {
    fn new(id: u64, name: String, detail: String, kind: SourceKind) -> Self {
        Self {
            id,
            name,
            detail,
            kind,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            active: Arc::new(AtomicBool::new(true)),
            muted: Arc::new(AtomicBool::new(false)),
            peak: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            sample_rate: Arc::new(AtomicU32::new(0)),
        }
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, value: f32) {
        self.volume
            .store(value.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }

    pub fn active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }

    pub fn set_active(&self, value: bool) {
        self.active.store(value, Ordering::Relaxed);
    }

    pub fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    pub fn set_muted(&self, value: bool) {
        self.muted.store(value, Ordering::Relaxed);
    }

    pub fn peak(&self) -> f32 {
        if !self.active() || self.muted() {
            0.0
        } else {
            f32::from_bits(self.peak.load(Ordering::Relaxed))
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }

    fn set_peak(&self, value: f32) {
        self.peak.store(value.to_bits(), Ordering::Relaxed);
    }
}

pub struct EngineConfig {
    pub output_device: AudioDevice,
    pub loopback_device: AudioDevice,
    pub mic_devices: Vec<AudioDevice>,
    pub master_volume: f32,
    pub master_muted: bool,
}

pub struct Engine {
    running: Arc<AtomicBool>,
    master_volume: Arc<AtomicU32>,
    master_muted: Arc<AtomicBool>,
    master_peak: Arc<AtomicU32>,
    output_rate: Arc<AtomicU32>,
    sources: Vec<SourceControl>,
    handles: Vec<JoinHandle<()>>,
    error: Arc<Mutex<Option<String>>>,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            master_volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            master_muted: Arc::new(AtomicBool::new(false)),
            master_peak: Arc::new(AtomicU32::new(0.0f32.to_bits())),
            output_rate: Arc::new(AtomicU32::new(0)),
            sources: Vec::new(),
            handles: Vec::new(),
            error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn sources(&self) -> &[SourceControl] {
        &self.sources
    }

    pub fn master_volume(&self) -> f32 {
        f32::from_bits(self.master_volume.load(Ordering::Relaxed))
    }

    pub fn set_master_volume(&self, value: f32) {
        self.master_volume
            .store(value.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }

    pub fn master_muted(&self) -> bool {
        self.master_muted.load(Ordering::Relaxed)
    }

    pub fn set_master_muted(&self, value: bool) {
        self.master_muted.store(value, Ordering::Relaxed);
    }

    pub fn master_peak(&self) -> f32 {
        if self.master_muted() {
            0.0
        } else {
            f32::from_bits(self.master_peak.load(Ordering::Relaxed))
        }
    }

    pub fn output_rate(&self) -> u32 {
        self.output_rate.load(Ordering::Relaxed)
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|error| error.clone())
    }

    pub fn start(&mut self, config: EngineConfig) -> Result<()> {
        self.stop();
        if config.output_device.id.is_empty() {
            bail!("select an output device before starting");
        }
        if config.loopback_device.id.is_empty() {
            bail!("select a system audio device before starting");
        }

        let running = Arc::new(AtomicBool::new(true));
        let master_volume = Arc::new(AtomicU32::new(
            config.master_volume.clamp(0.0, 2.0).to_bits(),
        ));
        let master_muted = Arc::new(AtomicBool::new(config.master_muted));
        let master_peak = Arc::new(AtomicU32::new(0.0f32.to_bits()));
        let output_rate = Arc::new(AtomicU32::new(0));
        let error = Arc::new(Mutex::new(None));

        let mut controls = Vec::new();
        let mut capture_jobs = Vec::new();
        let mut render_sources = Vec::new();
        let mut next_id = 1u64;

        // System audio source.
        {
            let control = SourceControl::new(
                next_id,
                config.loopback_device.name.clone(),
                "Loopback do sistema".to_string(),
                SourceKind::System,
            );
            next_id += 1;
            let rb = HeapRb::<f32>::new(192_000 * 2);
            let (producer, consumer) = rb.split();
            capture_jobs.push(CaptureJob {
                device_id: config.loopback_device.id.clone(),
                loopback: true,
                producer,
                control: control.clone(),
                running: running.clone(),
                error: error.clone(),
            });
            render_sources.push(RenderSource {
                control: control.clone(),
                consumer,
                scratch_in: Vec::new(),
                scratch_out: Vec::new(),
                resampler: LinearResampler::default(),
                rate: 0,
            });
            controls.push(control);
        }

        // Microphone sources.
        for mic in &config.mic_devices {
            let control = SourceControl::new(
                next_id,
                mic.name.clone(),
                "Microfone WASAPI".to_string(),
                SourceKind::Mic,
            );
            next_id += 1;
            let rb = HeapRb::<f32>::new(192_000 * 2);
            let (producer, consumer) = rb.split();
            capture_jobs.push(CaptureJob {
                device_id: mic.id.clone(),
                loopback: false,
                producer,
                control: control.clone(),
                running: running.clone(),
                error: error.clone(),
            });
            render_sources.push(RenderSource {
                control: control.clone(),
                consumer,
                scratch_in: Vec::new(),
                scratch_out: Vec::new(),
                resampler: LinearResampler::default(),
                rate: 0,
            });
            controls.push(control);
        }

        let mut handles = Vec::new();
        for job in capture_jobs {
            handles.push(thread::spawn(move || capture_loop(job)));
        }

        let render_job = RenderJob {
            output_device_id: config.output_device.id.clone(),
            running: running.clone(),
            master_volume: master_volume.clone(),
            master_muted: master_muted.clone(),
            master_peak: master_peak.clone(),
            output_rate: output_rate.clone(),
            sources: render_sources,
            error: error.clone(),
        };
        handles.push(thread::spawn(move || render_loop(render_job)));

        self.running = running;
        self.master_volume = master_volume;
        self.master_muted = master_muted;
        self.master_peak = master_peak;
        self.output_rate = output_rate;
        self.sources = controls;
        self.handles = handles;
        self.error = error;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
        self.sources.clear();
        self.master_peak.store(0.0f32.to_bits(), Ordering::Relaxed);
        self.output_rate.store(0, Ordering::Relaxed);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

struct CaptureJob {
    device_id: String,
    loopback: bool,
    producer: ringbuf::HeapProd<f32>,
    control: SourceControl,
    running: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

fn set_error(error: &Arc<Mutex<Option<String>>>, message: impl Into<String>) {
    if let Ok(mut slot) = error.lock() {
        if slot.is_none() {
            *slot = Some(message.into());
        }
    }
}

fn capture_loop(mut job: CaptureJob) {
    let stream = match CaptureStream::open(&job.device_id, job.loopback) {
        Ok(stream) => stream,
        Err(error) => {
            set_error(&job.error, format!("capture failed: {error:#}"));
            return;
        }
    };
    job.control
        .sample_rate
        .store(stream.format.sample_rate, Ordering::Relaxed);

    let mut buffer = Vec::new();
    while job.running.load(Ordering::Relaxed) {
        if let Err(error) = stream.read_into(&mut buffer) {
            set_error(&job.error, format!("capture read failed: {error:#}"));
            break;
        }
        if buffer.is_empty() {
            job.control.set_peak(0.0);
            thread::sleep(Duration::from_millis(2));
            continue;
        }

        let peak = buffer
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        job.control.set_peak(peak);

        let mut offset = 0;
        while offset < buffer.len() {
            let written = job.producer.push_slice(&buffer[offset..]);
            if written == 0 {
                break;
            }
            offset += written;
        }
        thread::sleep(Duration::from_millis(2));
    }
    stream.stop();
}

struct RenderSource {
    control: SourceControl,
    consumer: ringbuf::HeapCons<f32>,
    scratch_in: Vec<f32>,
    scratch_out: Vec<f32>,
    resampler: LinearResampler,
    rate: u32,
}

struct RenderJob {
    output_device_id: String,
    running: Arc<AtomicBool>,
    master_volume: Arc<AtomicU32>,
    master_muted: Arc<AtomicBool>,
    master_peak: Arc<AtomicU32>,
    output_rate: Arc<AtomicU32>,
    sources: Vec<RenderSource>,
    error: Arc<Mutex<Option<String>>>,
}

fn render_loop(mut job: RenderJob) {
    let stream = match RenderStream::open(&job.output_device_id) {
        Ok(stream) => stream,
        Err(error) => {
            set_error(&job.error, format!("render failed: {error:#}"));
            return;
        }
    };
    let out_rate = stream.format.sample_rate;
    job.output_rate.store(out_rate, Ordering::Relaxed);

    let max_frames = stream.buffer_size.max(1) as usize;
    let mut mix = vec![0.0f32; max_frames * 2];
    let mut out_bytes = vec![0u8; max_frames * stream.format.block_align as usize];
    for source in &mut job.sources {
        source.scratch_in.reserve(max_frames * 8 * 2);
        source.scratch_out.reserve(max_frames * 2);
    }

    while job.running.load(Ordering::Relaxed) {
        let frames = match stream.available_frames() {
            Ok(frames) => frames,
            Err(error) => {
                set_error(&job.error, format!("render padding failed: {error:#}"));
                break;
            }
        };
        if frames == 0 {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        let frames_usize = frames as usize;
        mix[..frames_usize * 2].fill(0.0);

        for source in &mut job.sources {
            let src_rate = source.control.sample_rate();
            if src_rate == 0 {
                continue;
            }
            let active = source.control.active() && !source.control.muted();
            if source.scratch_out.len() < frames_usize * 2 {
                source.scratch_out.resize(frames_usize * 2, 0.0);
            }

            if src_rate == out_rate {
                // Common case: no resampling, consume exactly one frame per output frame.
                let got = source
                    .consumer
                    .pop_slice(&mut source.scratch_out[..frames_usize * 2]);
                if got < frames_usize * 2 {
                    source.scratch_out[got..frames_usize * 2].fill(0.0);
                }
            } else {
                if source.rate != src_rate {
                    source.rate = src_rate;
                    source
                        .resampler
                        .set_ratio(src_rate as f64 / out_rate as f64);
                }
                let needed_in =
                    ((frames_usize as f64 * src_rate as f64 / out_rate as f64).ceil() as usize) + 1;
                let needed_samples = needed_in * 2;
                if source.scratch_in.len() < needed_samples {
                    source.scratch_in.resize(needed_samples, 0.0);
                }
                let got = source
                    .consumer
                    .pop_slice(&mut source.scratch_in[..needed_samples]);
                if got < needed_samples {
                    source.scratch_in[got..needed_samples].fill(0.0);
                }
                source.resampler.process(
                    &source.scratch_in[..needed_samples],
                    frames_usize,
                    &mut source.scratch_out,
                );
            }

            if active {
                let volume = source.control.volume();
                for (target, sample) in mix
                    .iter_mut()
                    .take(frames_usize * 2)
                    .zip(source.scratch_out.iter().take(frames_usize * 2))
                {
                    *target += *sample * volume;
                }
            }
        }

        let master = if job.master_muted.load(Ordering::Relaxed) {
            0.0
        } else {
            f32::from_bits(job.master_volume.load(Ordering::Relaxed))
        };
        let mut peak = 0.0f32;
        for sample in &mut mix[..frames_usize * 2] {
            let value = (*sample * master).clamp(-1.0, 1.0);
            *sample = value;
            peak = peak.max(value.abs());
        }
        job.master_peak.store(peak.to_bits(), Ordering::Relaxed);

        if let Err(error) = stream.write(&mix[..frames_usize * 2], frames, &mut out_bytes) {
            set_error(&job.error, format!("render write failed: {error:#}"));
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    stream.stop();
}

struct LinearResampler {
    ratio: f64,
    position: f64,
    last: [f32; 2],
}

impl Default for LinearResampler {
    fn default() -> Self {
        Self {
            ratio: 1.0,
            position: 0.0,
            last: [0.0, 0.0],
        }
    }
}

impl LinearResampler {
    fn set_ratio(&mut self, ratio: f64) {
        if ratio.is_finite() && ratio > 0.0 {
            self.ratio = ratio;
        }
    }

    fn process(&mut self, input: &[f32], out_frames: usize, output: &mut Vec<f32>) {
        output.clear();
        output.resize(out_frames * 2, 0.0);
        let in_frames = input.len() / 2;
        if in_frames == 0 {
            return;
        }

        let mut position = self.position;
        let mut last = self.last;
        for frame in 0..out_frames {
            let index = position.floor() as isize;
            let fraction = (position - index as f64) as f32;
            let first = sample_at(input, in_frames, index, last);
            let second = sample_at(input, in_frames, index + 1, first);
            output[frame * 2] = first[0] + (second[0] - first[0]) * fraction;
            output[frame * 2 + 1] = first[1] + (second[1] - first[1]) * fraction;
            position += self.ratio;
        }

        last = [input[(in_frames - 1) * 2], input[(in_frames - 1) * 2 + 1]];
        self.position = position - in_frames as f64;
        self.last = last;
    }
}

fn sample_at(input: &[f32], in_frames: usize, index: isize, fallback: [f32; 2]) -> [f32; 2] {
    if index < 0 || index as usize >= in_frames {
        fallback
    } else {
        let base = index as usize * 2;
        [input[base], input[base + 1]]
    }
}

/// Decode a WAV file to interleaved stereo f32 at the given sample rate.
pub fn decode_wav(path: &std::path::Path, target_rate: u32) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)
        .with_context(|| format!("failed to open sound {}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let source_rate = spec.sample_rate;
    let mut samples: Vec<f32> = Vec::new();

    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                samples.push(sample.context("failed to read a WAV sample")?);
            }
        }
        hound::SampleFormat::Int => {
            let max = (1i64 << (spec.bits_per_sample.saturating_sub(1) as u32)) as f32;
            for sample in reader.samples::<i32>() {
                samples.push(sample.context("failed to read a WAV sample")? as f32 / max);
            }
        }
    }

    if channels == 0 {
        bail!("WAV file has no channels");
    }
    let frames = samples.len() / channels;
    let mut stereo = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let base = frame * channels;
        let left = samples[base];
        let right = if channels >= 2 {
            samples[base + 1]
        } else {
            left
        };
        stereo.push(left);
        stereo.push(right);
    }

    if source_rate == target_rate || target_rate == 0 {
        return Ok(stereo);
    }
    // Offline linear resampling (fine for short soundboard clips).
    let ratio = source_rate as f64 / target_rate as f64;
    let out_frames = ((frames as f64) / ratio).round() as usize;
    let mut out = Vec::with_capacity(out_frames * 2);
    for frame in 0..out_frames {
        let position = frame as f64 * ratio;
        let index = position.floor() as usize;
        let fraction = (position - index as f64) as f32;
        let first = if index < frames {
            [stereo[index * 2], stereo[index * 2 + 1]]
        } else {
            [0.0, 0.0]
        };
        let second = if index + 1 < frames {
            [stereo[(index + 1) * 2], stereo[(index + 1) * 2 + 1]]
        } else {
            first
        };
        out.push(first[0] + (second[0] - first[0]) * fraction);
        out.push(first[1] + (second[1] - first[1]) * fraction);
    }
    Ok(out)
}
