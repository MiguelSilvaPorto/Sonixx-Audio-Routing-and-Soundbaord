//! Windows audio core: WASAPI device enumeration, capture, mixing and render.

pub mod engine;
pub mod wasapi;

pub use engine::{decode_wav, Engine, EngineConfig, SourceControl, SourceKind};
pub use wasapi::{default_device, list_devices, vb_cable_output, AudioDevice, DeviceFlow};
