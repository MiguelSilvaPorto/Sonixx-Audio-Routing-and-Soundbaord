//! Optional bridge that receives the audible Chrome tabs from the companion
//! extension. Windows exposes Chrome as one audio session, so this is the only
//! way to know which tabs are playing without a driver.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChromeTab {
    pub id: i64,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub audible: bool,
}

#[derive(Deserialize)]
struct TabsPayload {
    tabs: Vec<ChromeTab>,
}

pub struct ChromeBridge {
    tabs: Arc<Mutex<Vec<ChromeTab>>>,
    port: u16,
}

impl ChromeBridge {
    /// Starts the local bridge on 127.0.0.1:37645.
    pub fn start() -> Result<Self> {
        let port = 37_645u16;
        let server = tiny_http::Server::http(("127.0.0.1", port))
            .map_err(|error| anyhow::anyhow!("failed to start Chrome bridge: {error}"))?;
        let tabs = Arc::new(Mutex::new(Vec::new()));
        let shared = tabs.clone();
        thread::spawn(move || {
            for mut request in server.incoming_requests() {
                if request.method() == &tiny_http::Method::Options {
                    let _ = request.respond(cors_response(204, ""));
                    continue;
                }
                if request.url() == "/tabs" && request.method() == &tiny_http::Method::Post {
                    let mut body = String::new();
                    if request.as_reader().read_to_string(&mut body).is_ok() {
                        if let Ok(payload) = serde_json::from_str::<TabsPayload>(&body) {
                            if let Ok(mut current) = shared.lock() {
                                *current = payload.tabs;
                            }
                        }
                    }
                    let _ = request.respond(cors_response(200, "ok"));
                } else if request.url() == "/health" {
                    let _ = request.respond(cors_response(200, "sonixx"));
                } else {
                    let _ = request.respond(cors_response(404, "not found"));
                }
            }
        });
        Ok(Self { tabs, port })
    }

    pub fn tabs(&self) -> Vec<ChromeTab> {
        self.tabs
            .lock()
            .map(|tabs| tabs.clone())
            .unwrap_or_default()
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

fn cors_response(status: u16, body: &str) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let mut response = tiny_http::Response::from_string(body.to_string()).with_status_code(status);
    for (name, value) in [
        ("Access-Control-Allow-Origin", "*"),
        ("Access-Control-Allow-Methods", "POST, GET, OPTIONS"),
        (
            "Access-Control-Allow-Headers",
            "Content-Type, X-Sonixx-Bridge",
        ),
    ] {
        if let Ok(header) = tiny_http::Header::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    response
}

/// Sanity check used by the integration tests.
pub fn bridge_port() -> u16 {
    37_645
}

pub fn ensure_bridge_available() -> Result<()> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", bridge_port()))
        .context("port 37645 is already in use")?;
    drop(listener);
    Ok(())
}
