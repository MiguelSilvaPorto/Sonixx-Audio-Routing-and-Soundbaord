//! Core types and settings shared by the Sonixx app and the future audio engine.

pub mod settings;

pub use settings::Settings;

pub const APP_NAME: &str = "Sonixx";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
