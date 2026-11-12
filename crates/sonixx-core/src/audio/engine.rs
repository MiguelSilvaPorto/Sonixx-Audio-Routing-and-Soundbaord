use crate::audio::wasapi::{AudioDevice, CaptureStream, RenderStream};
use anyhow::{bail, Context, Result};
use ringbuf::{traits::*, HeapRb};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    System,
    App,
    Mic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppSource {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
}

pub struct Sound {
    pub samples: Vec<f32>,
    pub name: String,
}

#[derive(Clone)]
pub struct SourceControl {
    pub id: u64,
    pub name: String,
    pub detail: String,
    pub kind: SourceKind,
    pub exe: Option<String>,
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
            exe: None,
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
    pub app_sources: Vec<AppSource>,
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
    soundboard_volume: Arc<AtomicU32>,
    soundboard_muted: Arc<AtomicBool>,
    sound_tx: Option<Sender<Arc<Sound>>>,
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
            soundboard_volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            soundboard_muted: Arc::new(AtomicBool::new(false)),
            sound_tx: None,
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

    pub fn soundboard_volume(&self) -> f32 {
        f32::from_bits(self.soundboard_volume.load(Ordering::Relaxed))
    }

    pub fn set_soundboard_volume(&self, value: f32) {
        self.soundboard_volume
            .store(value.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }

    pub fn soundboard_muted(&self) -> bool {
        self.soundboard_muted.load(Ordering::Relaxed)
    }

    pub fn set_soundboard_muted(&self, value: bool) {
        self.soundboard_muted.store(value, Ordering::Relaxed);
    }

    /// Decodes an audio file to the engine's output rate and queues it for
    /// playback through the mixer.
    pub fn play_sound(&self, path: &Path) -> Result<()> {
        let rate = self.output_rate();
        if rate == 0 {
            bail!("start the engine before playing soundboard files");
        }
        let samples = decode_audio(path, rate)?;
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "Som".to_string());
        let sender = self
            .sound_tx
            .as_ref()
            .context("the soundboard is not running")?;
        sender
            .send(Arc::new(Sound { samples, name }))
            .context("the soundboard thread stopped")?;
        Ok(())
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
                target: CaptureTarget::Device {
                    device_id: config.loopback_device.id.clone(),
                    loopback: true,
                },
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

        // Per-application process loopback sources.
        for app in &config.app_sources {
            let mut control = SourceControl::new(
                next_id,
                app.name.clone(),
                format!("Aplicativo · PID {}", app.pid),
                SourceKind::App,
            );
            control.exe = app.exe.clone();
            next_id += 1;
            let rb = HeapRb::<f32>::new(192_000 * 2);
            let (producer, consumer) = rb.split();
            capture_jobs.push(CaptureJob {
                target: CaptureTarget::Process(app.pid),
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
                target: CaptureTarget::Device {
                    device_id: mic.id.clone(),
                    loopback: false,
                },
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

        let (sound_tx, sound_rx) = mpsc::channel();
        let render_job = RenderJob {
            output_device_id: config.output_device.id.clone(),
            running: running.clone(),
            master_volume: master_volume.clone(),
            master_muted: master_muted.clone(),
            master_peak: master_peak.clone(),
            output_rate: output_rate.clone(),
            sources: render_sources,
            sound_rx,
            soundboard_volume: self.soundboard_volume.clone(),
            soundboard_muted: self.soundboard_muted.clone(),
            error: error.clone(),
        };
        handles.push(thread::spawn(move || render_loop(render_job)));

        self.running = running;
        self.master_volume = master_volume;
        self.master_muted = master_muted;
        self.master_peak = master_peak;
        self.output_rate = output_rate;
        self.sound_tx = Some(sound_tx);
        self.sources = controls;
        self.handles = handles;
        self.error = error;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        self.sound_tx = None;
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

enum CaptureTarget {
    Device { device_id: String, loopback: bool },
    Process(u32),
}

struct CaptureJob {
    target: CaptureTarget,
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
    let stream = match &job.target {
        CaptureTarget::Device {
            device_id,
            loopback,
        } => CaptureStream::open(device_id, *loopback),
        CaptureTarget::Process(process_id) => CaptureStream::open_process(*process_id),
    };
    let stream = match stream {
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
    sound_rx: Receiver<Arc<Sound>>,
    soundboard_volume: Arc<AtomicU32>,
    soundboard_muted: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

struct Voice {
    sound: Arc<Sound>,
    position: usize,
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
    let mut voices: Vec<Voice> = Vec::new();
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

        while let Ok(sound) = job.sound_rx.try_recv() {
            voices.push(Voice { sound, position: 0 });
        }

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

        if !job.soundboard_muted.load(Ordering::Relaxed) {
            let volume = f32::from_bits(job.soundboard_volume.load(Ordering::Relaxed));
            for voice in &mut voices {
                let start = voice.position * 2;
                let remaining = voice.sound.samples.len().saturating_sub(start);
                let count = remaining.min(frames_usize * 2);
                for (target, sample) in mix
                    .iter_mut()
                    .take(count)
                    .zip(&voice.sound.samples[start..start + count])
                {
                    *target += *sample * volume;
                }
                voice.position += count / 2;
            }
            voices.retain(|voice| voice.position * 2 < voice.sound.samples.len());
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

fn resample_stereo(stereo: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    if source_rate == target_rate || target_rate == 0 {
        return stereo.to_vec();
    }
    let frames = stereo.len() / 2;
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
    out
}

/// Decodes a sound file (WAV, MP3, FLAC, OGG, AAC) to interleaved stereo f32 at
/// the requested sample rate.
pub fn decode_audio(path: &Path, target_rate: u32) -> Result<Vec<f32>> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("failed to open sound {}", path.display()))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("failed to detect sound format {}", path.display()))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .context("the sound file has no default audio track")?;
    let track_id = track.id;
    let source_rate = track.codec_params.sample_rate.unwrap_or(48_000);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("failed to create the audio decoder")?;

    let mut stereo = Vec::new();
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(error) => return Err(error).context("failed to decode the sound file"),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count();
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks(channels.max(1)) {
            let left = frame.first().copied().unwrap_or(0.0);
            let right = frame.get(1).copied().unwrap_or(left);
            stereo.push(left);
            stereo.push(right);
        }
    }

    if stereo.is_empty() {
        bail!("no audio samples were decoded from {}", path.display());
    }
    Ok(resample_stereo(&stereo, source_rate, target_rate))
}
