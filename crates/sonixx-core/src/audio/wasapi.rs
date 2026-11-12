use anyhow::{bail, Context, Result};
use std::mem::{size_of, ManuallyDrop};
use std::ops::Deref;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use windows::core::{implement, IUnknown, Interface, Ref, GUID};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, ActivateAudioInterfaceAsync, EDataFlow,
    IActivateAudioInterfaceAsyncOperation, IActivateAudioInterfaceCompletionHandler,
    IActivateAudioInterfaceCompletionHandler_Impl, IAudioCaptureClient, IAudioClient,
    IAudioRenderClient, IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2,
    IMMDeviceCollection, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK,
    AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, AUDIOCLIENT_ACTIVATION_PARAMS,
    AUDIOCLIENT_ACTIVATION_PARAMS_0, AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
    AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS, DEVICE_STATE_ACTIVE,
    PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0,
};
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantClear, PropVariantToStringAlloc, PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0,
    PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, BLOB, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Threading::{
    CreateEventW, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::System::Variant::VT_BLOB;

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const KSDATAFORMAT_SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
const KSDATAFORMAT_SUBTYPE_IEEE_FLOAT: GUID =
    GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceFlow {
    Render,
    Capture,
}

impl DeviceFlow {
    fn to_wasapi(self) -> EDataFlow {
        match self {
            Self::Render => eRender,
            Self::Capture => eCapture,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub flow: DeviceFlow,
}

/// COM is reference-counted per thread. If the thread already initialized COM
/// with another apartment model (common in GUI threads), we keep going and do
/// not uninitialize it on drop.
pub struct ComGuard {
    uninitialize: bool,
}

impl ComGuard {
    pub fn new() -> Self {
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Self {
            uninitialize: hr.is_ok(),
        }
    }
}

impl Default for ComGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

fn enumerator() -> Result<IMMDeviceEnumerator> {
    unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .context("failed to create the Windows audio device enumerator")
    }
}

fn device_name(device: &windows::Win32::Media::Audio::IMMDevice) -> Result<String> {
    unsafe {
        let store = device
            .OpenPropertyStore(STGM_READ)
            .context("failed to open the audio device property store")?;
        let mut value = store
            .GetValue(&PKEY_Device_FriendlyName)
            .context("failed to read the audio device friendly name")?;
        let text =
            PropVariantToStringAlloc(&value).context("failed to convert the audio device name")?;
        let name = text
            .to_string()
            .unwrap_or_else(|_| "Unknown device".to_string());
        CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
        let _ = PropVariantClear(&mut value);
        Ok(name)
    }
}

fn device_id(device: &windows::Win32::Media::Audio::IMMDevice) -> Result<String> {
    unsafe {
        let text = device
            .GetId()
            .context("failed to read the audio device id")?;
        let id = text.to_string().context("invalid audio device id")?;
        CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
        Ok(id)
    }
}

pub fn list_devices(flow: DeviceFlow) -> Result<Vec<AudioDevice>> {
    let _com = ComGuard::new();
    let enumerator = enumerator()?;
    let collection: IMMDeviceCollection = unsafe {
        enumerator
            .EnumAudioEndpoints(flow.to_wasapi(), DEVICE_STATE_ACTIVE)
            .context("failed to enumerate Windows audio endpoints")?
    };
    let count = unsafe {
        collection
            .GetCount()
            .context("failed to count audio endpoints")?
    };

    let mut devices = Vec::with_capacity(count as usize);
    for index in 0..count {
        let device = unsafe {
            collection
                .Item(index)
                .context("failed to read an audio endpoint")?
        };
        let name = device_name(&device)?;
        let id = device_id(&device)?;
        devices.push(AudioDevice { id, name, flow });
    }
    devices.sort_by_key(|device| device.name.to_lowercase());
    Ok(devices)
}

pub fn find_device(flow: DeviceFlow, id: &str) -> Result<AudioDevice> {
    list_devices(flow)?
        .into_iter()
        .find(|device| device.id == id)
        .with_context(|| format!("audio device not found: {id}"))
}

pub fn default_device(flow: DeviceFlow) -> Result<AudioDevice> {
    let _com = ComGuard::new();
    let enumerator = enumerator()?;
    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(flow.to_wasapi(), eConsole)
            .context("failed to read the default audio endpoint")?
    };
    let name = device_name(&device)?;
    let id = device_id(&device)?;
    Ok(AudioDevice { id, name, flow })
}

/// Peak level of the default playback endpoint (all sessions combined).
/// This does not open a capture stream and does not require any driver.
pub fn default_output_peak() -> Result<f32> {
    let _com = ComGuard::new();
    let enumerator = enumerator()?;
    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .context("failed to read the default output device")?
    };
    let meter: IAudioMeterInformation = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .context("failed to open the output peak meter")?
    };
    unsafe {
        meter
            .GetPeakValue()
            .context("failed to read the output peak")
    }
}

/// Peak level of a specific playback endpoint.
pub fn output_peak(device_id: &str) -> Result<f32> {
    let _com = ComGuard::new();
    let enumerator = enumerator()?;
    let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
    let device = unsafe {
        enumerator
            .GetDevice(windows::core::PCWSTR(wide.as_ptr()))
            .context("failed to open the output device")?
    };
    let meter: IAudioMeterInformation = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .context("failed to open the output peak meter")?
    };
    unsafe {
        meter
            .GetPeakValue()
            .context("failed to read the output peak")
    }
}

pub fn is_vb_cable_installed() -> Result<bool> {
    let outputs = list_devices(DeviceFlow::Render)?;
    Ok(outputs
        .iter()
        .any(|device| device.name.to_lowercase().contains("cable input")))
}

pub fn vb_cable_output() -> Result<Option<AudioDevice>> {
    let outputs = list_devices(DeviceFlow::Render)?;
    Ok(outputs
        .into_iter()
        .find(|device| device.name.to_lowercase().contains("cable input")))
}

pub fn vb_cable_capture() -> Result<Option<AudioDevice>> {
    let inputs = list_devices(DeviceFlow::Capture)?;
    Ok(inputs.into_iter().find(|device| {
        let name = device.name.to_lowercase();
        name.contains("cable output") || (name.contains("sonixx") && name.contains("output"))
    }))
}

pub fn ensure_supported() -> Result<()> {
    if list_devices(DeviceFlow::Render)?.is_empty() {
        bail!("no active audio output device was found");
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub is_float: bool,
}

impl AudioFormat {
    unsafe fn from_ptr(ptr: *const WAVEFORMATEX) -> Result<Self> {
        if ptr.is_null() {
            bail!("WASAPI returned a null audio format");
        }
        let wave = unsafe { *ptr };
        let mut bits = wave.wBitsPerSample;
        let mut is_float = false;

        match wave.wFormatTag {
            WAVE_FORMAT_IEEE_FLOAT => is_float = true,
            WAVE_FORMAT_PCM => {}
            WAVE_FORMAT_EXTENSIBLE => {
                let ext = unsafe { *(ptr as *const WAVEFORMATEXTENSIBLE) };
                let subformat: GUID =
                    unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(ext.SubFormat)) };
                if subformat == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT {
                    is_float = true;
                } else if subformat != KSDATAFORMAT_SUBTYPE_PCM {
                    bail!("unsupported WASAPI audio subformat");
                }
                let valid = unsafe { ext.Samples.wValidBitsPerSample };
                if valid > 0 {
                    bits = valid;
                }
            }
            other => bail!("unsupported WASAPI audio format tag: {other}"),
        }

        if wave.nChannels == 0 || wave.nSamplesPerSec == 0 || wave.nBlockAlign == 0 {
            bail!("invalid WASAPI audio format");
        }
        Ok(Self {
            sample_rate: wave.nSamplesPerSec,
            channels: wave.nChannels,
            block_align: wave.nBlockAlign,
            bits_per_sample: bits,
            is_float,
        })
    }

    pub fn bytes_per_sample(&self) -> usize {
        (self.bits_per_sample as usize).div_ceil(8)
    }

    /// Builds a plain WAVEFORMATEX (no WAVEFORMATEXTENSIBLE). Some drivers
    /// reject the pointer returned by GetMixFormat directly, but accept this
    /// equivalent description of the same mix format.
    fn to_waveformat(self) -> WAVEFORMATEX {
        WAVEFORMATEX {
            wFormatTag: if self.is_float {
                WAVE_FORMAT_IEEE_FLOAT
            } else {
                WAVE_FORMAT_PCM
            },
            nChannels: self.channels,
            nSamplesPerSec: self.sample_rate,
            nAvgBytesPerSec: self.sample_rate * self.block_align as u32,
            nBlockAlign: self.block_align,
            wBitsPerSample: self.bits_per_sample,
            cbSize: 0,
        }
    }

    pub fn to_stereo_f32(&self, bytes: &[u8], frames: u32, out: &mut Vec<f32>) {
        let frames = frames as usize;
        let bps = self.bytes_per_sample();
        if bps == 0 || self.block_align == 0 {
            out.extend(std::iter::repeat_n(0.0, frames * 2));
            return;
        }
        out.reserve(frames * 2);
        for frame in 0..frames {
            let base = frame * self.block_align as usize;
            if base + bps > bytes.len() {
                out.push(0.0);
                out.push(0.0);
                continue;
            }
            let left = read_sample(bytes, base, self.bits_per_sample, self.is_float);
            let right = if self.channels >= 2 && base + bps * 2 <= bytes.len() {
                read_sample(bytes, base + bps, self.bits_per_sample, self.is_float)
            } else {
                left
            };
            out.push(left);
            out.push(right);
        }
    }

    pub fn from_stereo_f32(&self, mix: &[f32], frames: u32, out: &mut [u8]) {
        let frames = frames as usize;
        let bps = self.bytes_per_sample();
        let frame_bytes = self.block_align as usize;
        if bps == 0 || frame_bytes == 0 || out.len() < frames * frame_bytes {
            return;
        }
        out[..frames * frame_bytes].fill(0);
        for frame in 0..frames {
            let base = frame * frame_bytes;
            let left = mix.get(frame * 2).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
            let right = mix
                .get(frame * 2 + 1)
                .copied()
                .unwrap_or(left)
                .clamp(-1.0, 1.0);
            write_sample(out, base, left, self.bits_per_sample, self.is_float);
            if self.channels >= 2 && base + bps * 2 <= out.len() {
                write_sample(out, base + bps, right, self.bits_per_sample, self.is_float);
            }
            // Extra channels stay silent; the first two carry the mix.
        }
    }
}

fn read_sample(bytes: &[u8], offset: usize, bits: u16, is_float: bool) -> f32 {
    match (bits, is_float) {
        (32, true) => {
            let mut raw = [0u8; 4];
            raw.copy_from_slice(&bytes[offset..offset + 4]);
            f32::from_le_bytes(raw)
        }
        (32, false) => {
            let mut raw = [0u8; 4];
            raw.copy_from_slice(&bytes[offset..offset + 4]);
            i32::from_le_bytes(raw) as f32 / 2_147_483_648.0
        }
        (24, false) => {
            let b = &bytes[offset..offset + 3];
            let value = i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8;
            value as f32 / 8_388_608.0
        }
        (16, false) => {
            let mut raw = [0u8; 2];
            raw.copy_from_slice(&bytes[offset..offset + 2]);
            i16::from_le_bytes(raw) as f32 / 32_768.0
        }
        (8, false) => (bytes[offset] as f32 - 128.0) / 128.0,
        _ => 0.0,
    }
}

fn write_sample(out: &mut [u8], offset: usize, value: f32, bits: u16, is_float: bool) {
    let value = value.clamp(-1.0, 1.0);
    match (bits, is_float) {
        (32, true) => {
            out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        (32, false) => {
            let sample = (value * 2_147_483_647.0) as i32;
            out[offset..offset + 4].copy_from_slice(&sample.to_le_bytes());
        }
        (24, false) => {
            let sample = (value * 8_388_607.0) as i32;
            let bytes = sample.to_le_bytes();
            out[offset..offset + 3].copy_from_slice(&bytes[..3]);
        }
        (16, false) => {
            let sample = (value * 32_767.0) as i16;
            out[offset..offset + 2].copy_from_slice(&sample.to_le_bytes());
        }
        (8, false) => {
            out[offset] = ((value * 127.0) + 128.0) as u8;
        }
        _ => {}
    }
}

#[derive(Clone, Debug)]
pub struct AudioSession {
    pub pid: u32,
    pub name: String,
    pub peak: f32,
    pub session_id: String,
    pub exe: Option<String>,
}

fn process_exe(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(handle);
        if result.is_ok() {
            Some(String::from_utf16_lossy(&buffer[..size as usize]))
        } else {
            None
        }
    }
}

fn is_input_like_name(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "microphone",
        "microfone",
        "headset",
        "line in",
        "stereo mix",
        "mixagem",
        "alto-falantes",
        "speakers",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

fn is_system_process(exe: &str) -> bool {
    let exe = exe.to_lowercase();
    ["audiodg.exe", "svchost.exe", "system", "idle", "registry"]
        .iter()
        .any(|needle| exe.ends_with(needle))
}

/// Lists applications that currently have an audio session on the default
/// playback device. This is what the UI shows in the "Aplicativos" tab.
pub fn list_audio_sessions() -> Result<Vec<AudioSession>> {
    let _com = ComGuard::new();
    let enumerator = enumerator()?;
    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .context("failed to open the default playback device")?
    };
    let manager: IAudioSessionManager2 = unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .context("failed to open the audio session manager")?
    };
    let sessions: IAudioSessionEnumerator = unsafe {
        manager
            .GetSessionEnumerator()
            .context("failed to enumerate audio sessions")?
    };
    let count = unsafe {
        sessions
            .GetCount()
            .context("failed to count audio sessions")?
    };

    let mut system: Option<sysinfo::System> = None;
    let mut found = Vec::new();
    for index in 0..count {
        let Ok(control) = (unsafe { sessions.GetSession(index) }) else {
            continue;
        };
        let Ok(control2) = control.cast::<IAudioSessionControl2>() else {
            continue;
        };
        if unsafe { control2.IsSystemSoundsSession().0 } == 0 {
            continue;
        }
        let Ok(pid) = (unsafe { control2.GetProcessId() }) else {
            continue;
        };
        if pid == 0 || pid == std::process::id() {
            continue;
        }
        let display = unsafe {
            control
                .GetDisplayName()
                .ok()
                .map(|text| {
                    let value = text.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
                    value
                })
                .unwrap_or_default()
        };
        if is_input_like_name(&display) {
            continue;
        }
        let exe = process_exe(pid);
        if let Some(exe) = &exe {
            if is_system_process(exe) {
                continue;
            }
        }
        let name = if display.trim().is_empty() {
            if system.is_none() {
                system = sysinfo::System::new_all().ok();
            }
            system
                .as_ref()
                .and_then(|system| system.process(sysinfo::Pid::from_u32(pid)))
                .map(|process| process.name().to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            display
        };
        if name.trim().is_empty() || is_input_like_name(&name) {
            continue;
        }
        let session_id = unsafe {
            control2
                .GetSessionIdentifier()
                .ok()
                .map(|text| {
                    let value = text.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(text.0 as *const core::ffi::c_void));
                    value
                })
                .unwrap_or_default()
        };
        // Peak meter of this specific session. This reads the samples that
        // already left the application, without opening a capture stream.
        let peak = control
            .cast::<IAudioMeterInformation>()
            .ok()
            .and_then(|meter| unsafe { meter.GetPeakValue().ok() })
            .unwrap_or(0.0);
        found.push(AudioSession {
            pid,
            name,
            peak,
            session_id,
            exe,
        });
    }
    found.sort_by(|a, b| a.session_id.cmp(&b.session_id));
    found.dedup_by(|a, b| !a.session_id.is_empty() && a.session_id == b.session_id);
    found.sort_by_key(|session| session.name.to_lowercase());
    Ok(found)
}

fn canonical_capture_format() -> (AudioFormat, WAVEFORMATEXTENSIBLE) {
    let sample_rate = 48_000u32;
    let channels = 2u16;
    let block_align = 8u16;
    let format = AudioFormat {
        sample_rate,
        channels,
        block_align,
        bits_per_sample: 32,
        is_float: true,
    };
    let ext = WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE,
            nChannels: channels,
            nSamplesPerSec: sample_rate,
            nAvgBytesPerSec: sample_rate * block_align as u32,
            nBlockAlign: block_align,
            wBitsPerSample: 32,
            cbSize: 22,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 {
            wValidBitsPerSample: 32,
        },
        dwChannelMask: 0x3,
        SubFormat: KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
    };
    (format, ext)
}

fn initialize_capture_client(
    client: &IAudioClient,
    loopback: bool,
) -> Result<(AudioFormat, HANDLE)> {
    let (format, ext) = canonical_capture_format();
    let mut default_period = 0i64;
    let mut min_period = 0i64;
    unsafe {
        let _ = client.GetDevicePeriod(Some(&mut default_period), Some(&mut min_period));
    }
    let buffer_duration = if min_period > 0 { min_period } else { 0 };
    let flags = (if loopback {
        AUDCLNT_STREAMFLAGS_LOOPBACK
    } else {
        0
    }) | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM
        | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
    let event = unsafe { CreateEventW(None, false, false, windows::core::PCWSTR::null()) }
        .context("failed to create the capture event")?;
    let ext_ptr = (&ext as *const WAVEFORMATEXTENSIBLE).cast::<WAVEFORMATEX>();
    let init = unsafe {
        client.Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            flags,
            buffer_duration,
            0,
            ext_ptr,
            None,
        )
    };
    if let Err(error) = init {
        unsafe {
            let _ = CloseHandle(event);
        }
        return Err(error).context("failed to initialize the WASAPI capture stream");
    }
    if let Err(error) = unsafe { client.SetEventHandle(event) } {
        unsafe {
            let _ = CloseHandle(event);
        }
        return Err(error).context("failed to attach the capture event");
    }
    Ok((format, event))
}

#[implement(IActivateAudioInterfaceCompletionHandler)]
struct ActivationHandler(Arc<(Mutex<bool>, Condvar)>);

impl ActivationHandler {
    fn new(setup: Arc<(Mutex<bool>, Condvar)>) -> Self {
        Self(setup)
    }
}

impl IActivateAudioInterfaceCompletionHandler_Impl for ActivationHandler_Impl {
    fn ActivateCompleted(
        &self,
        _activateoperation: Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        let (lock, cvar) = &*self.0;
        let mut completed = lock
            .lock()
            .map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;
        *completed = true;
        drop(completed);
        cvar.notify_one();
        Ok(())
    }
}

pub struct CaptureStream {
    client: IAudioClient,
    capture: IAudioCaptureClient,
    pub format: AudioFormat,
    event: HANDLE,
    _com: ComGuard,
}

impl CaptureStream {
    pub fn open(device_id: &str, loopback: bool) -> Result<Self> {
        let com = ComGuard::new();
        let enumerator = enumerator()?;
        let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let device = unsafe {
            enumerator
                .GetDevice(windows::core::PCWSTR(wide.as_ptr()))
                .context("failed to open the capture device")?
        };
        let client: IAudioClient = unsafe {
            device
                .Activate(CLSCTX_ALL, None)
                .context("failed to activate the WASAPI capture client")?
        };
        let (format, event) = initialize_capture_client(&client, loopback)?;

        let capture: IAudioCaptureClient = unsafe {
            client
                .GetService()
                .context("failed to get the WASAPI capture client")?
        };
        unsafe { client.Start().context("failed to start audio capture")? };

        Ok(Self {
            client,
            capture,
            format,
            event,
            _com: com,
        })
    }

    /// Captures only the audio of one process (and its children) using the
    /// Windows process-loopback API.
    pub fn open_process(process_id: u32) -> Result<Self> {
        let com = ComGuard::new();
        let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
            ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
            Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
                ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                    TargetProcessId: process_id,
                    ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
                },
            },
        };
        let pinned_params = Pin::new(&mut params);
        let raw_prop = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_BLOB,
                    wReserved1: 0,
                    wReserved2: 0,
                    wReserved3: 0,
                    Anonymous: PROPVARIANT_0_0_0 {
                        blob: BLOB {
                            cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                            pBlobData: std::ptr::from_mut(pinned_params.get_mut()).cast(),
                        },
                    },
                }),
            },
        };
        let activation_prop = ManuallyDrop::new(raw_prop);
        let pinned_prop = Pin::new(activation_prop.deref());
        let activation_params = Some(std::ptr::from_ref(pinned_prop.get_ref()));

        let setup = Arc::new((Mutex::new(false), Condvar::new()));
        let callback: IActivateAudioInterfaceCompletionHandler =
            ActivationHandler::new(setup.clone()).into();
        let operation = unsafe {
            ActivateAudioInterfaceAsync(
                VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
                &IAudioClient::IID,
                activation_params,
                &callback,
            )
        }
        .context("failed to activate process loopback")?;

        let (lock, cvar) = &*setup;
        let mut completed = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("process loopback activation lock poisoned"))?;
        while !*completed {
            completed = cvar
                .wait(completed)
                .map_err(|_| anyhow::anyhow!("process loopback activation wait failed"))?;
        }
        drop(completed);

        let mut unknown: Option<IUnknown> = None;
        let mut result = windows::core::HRESULT(0);
        unsafe {
            operation
                .GetActivateResult(&mut result, &mut unknown)
                .context("failed to read the process loopback activation result")?;
        }
        result.ok().context("process loopback activation failed")?;
        let client: IAudioClient = unknown
            .context("process loopback returned no audio client")?
            .cast()
            .context("failed to cast the process loopback client")?;
        let (format, event) = initialize_capture_client(&client, true)?;
        let capture: IAudioCaptureClient = unsafe {
            client
                .GetService()
                .context("failed to get the process loopback capture client")?
        };
        unsafe {
            client
                .Start()
                .context("failed to start process loopback capture")?
        };

        Ok(Self {
            client,
            capture,
            format,
            event,
            _com: com,
        })
    }

    /// Drains all packets available right now into `out` (interleaved stereo f32).
    pub fn read_into(&self, out: &mut Vec<f32>) -> Result<()> {
        out.clear();
        loop {
            let packet = unsafe {
                self.capture
                    .GetNextPacketSize()
                    .context("failed to read the capture packet size")?
            };
            if packet == 0 {
                break;
            }
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames = 0u32;
            let mut flags = 0u32;
            unsafe {
                self.capture
                    .GetBuffer(&mut data, &mut frames, &mut flags, None, None)
                    .context("failed to read the capture buffer")?;
            }
            if frames > 0 && !data.is_null() {
                let silent = flags & (AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0;
                let byte_len = frames as usize * self.format.block_align as usize;
                if silent {
                    out.extend(std::iter::repeat_n(0.0, frames as usize * 2));
                } else {
                    let bytes = unsafe { std::slice::from_raw_parts(data, byte_len) };
                    self.format.to_stereo_f32(bytes, frames, out);
                }
            }
            unsafe {
                self.capture
                    .ReleaseBuffer(frames)
                    .context("failed to release the capture buffer")?;
            }
        }
        Ok(())
    }

    pub fn stop(&self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        self.stop();
        unsafe {
            let _ = CloseHandle(self.event);
        };
    }
}

pub struct RenderStream {
    client: IAudioClient,
    render: IAudioRenderClient,
    pub format: AudioFormat,
    pub buffer_size: u32,
    _com: ComGuard,
}

impl RenderStream {
    pub fn open(device_id: &str) -> Result<Self> {
        let com = ComGuard::new();
        let enumerator = enumerator()?;
        let wide: Vec<u16> = device_id.encode_utf16().chain(std::iter::once(0)).collect();
        let device = unsafe {
            enumerator
                .GetDevice(windows::core::PCWSTR(wide.as_ptr()))
                .context("failed to open the render device")?
        };
        let client: IAudioClient = unsafe {
            device
                .Activate(CLSCTX_ALL, None)
                .context("failed to activate the WASAPI render client")?
        };
        let format_ptr = unsafe {
            client
                .GetMixFormat()
                .context("failed to read the render mix format")?
        };
        let format = unsafe { AudioFormat::from_ptr(format_ptr)? };
        let wave = format.to_waveformat();
        let init = unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 0, 0, &wave, None) };
        unsafe { CoTaskMemFree(Some(format_ptr as *const core::ffi::c_void)) };
        init.context("failed to initialize the WASAPI render stream")?;

        let render: IAudioRenderClient = unsafe {
            client
                .GetService()
                .context("failed to get the WASAPI render client")?
        };
        let buffer_size = unsafe {
            client
                .GetBufferSize()
                .context("failed to read the render buffer size")?
        };

        // Pre-fill the device buffer with silence before starting.
        unsafe {
            let data = render
                .GetBuffer(buffer_size)
                .context("failed to prefill the render buffer")?;
            std::ptr::write_bytes(data, 0, buffer_size as usize * format.block_align as usize);
            render
                .ReleaseBuffer(buffer_size, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)
                .context("failed to release the prefilled render buffer")?;
            client.Start().context("failed to start audio rendering")?;
        }

        Ok(Self {
            client,
            render,
            format,
            buffer_size,
            _com: com,
        })
    }

    pub fn available_frames(&self) -> Result<u32> {
        unsafe {
            let padding = self
                .client
                .GetCurrentPadding()
                .context("failed to read the render padding")?;
            Ok(self.buffer_size.saturating_sub(padding))
        }
    }

    pub fn write(&self, mix: &[f32], frames: u32, out: &mut [u8]) -> Result<()> {
        if frames == 0 {
            return Ok(());
        }
        let byte_len = frames as usize * self.format.block_align as usize;
        if out.len() < byte_len {
            bail!("render scratch buffer is too small");
        }
        self.format
            .from_stereo_f32(mix, frames, &mut out[..byte_len]);
        unsafe {
            let data = self
                .render
                .GetBuffer(frames)
                .context("failed to get the render buffer")?;
            std::ptr::copy_nonoverlapping(out.as_ptr(), data, byte_len);
            self.render
                .ReleaseBuffer(frames, 0)
                .context("failed to release the render buffer")?;
        }
        Ok(())
    }

    pub fn stop(&self) {
        unsafe {
            let _ = self.client.Stop();
        }
    }
}

impl Drop for RenderStream {
    fn drop(&mut self) {
        self.stop();
    }
}
