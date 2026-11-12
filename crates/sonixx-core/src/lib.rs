//! Core types, settings and audio engine shared by the Sonixx app.

pub mod audio;
pub mod chrome_bridge;
pub mod settings;

pub use settings::{MicSelection, Settings};

pub const APP_NAME: &str = "Sonixx";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
