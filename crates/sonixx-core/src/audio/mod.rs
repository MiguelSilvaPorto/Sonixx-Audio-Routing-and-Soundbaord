//! Windows audio core: WASAPI device enumeration, capture, mixing and render.

pub mod engine;
pub mod wasapi;

pub use engine::{decode_audio, AppSource, Engine, EngineConfig, Sound, SourceControl, SourceKind};
pub use wasapi::{
    default_device, list_audio_sessions, list_devices, vb_cable_output, AudioDevice, AudioSession,
    DeviceFlow,
};
