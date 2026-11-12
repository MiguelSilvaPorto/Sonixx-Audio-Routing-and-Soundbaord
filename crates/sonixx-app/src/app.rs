use crate::icons;
use crate::theme::{self, *};
use eframe::egui::{self, Color32, RichText};
use sonixx_core::audio::{
    default_device, default_output_peak, list_audio_sessions, list_devices, AppSource, AudioDevice,
    AudioSession, DeviceFlow, Engine, EngineConfig, SourceControl, SourceKind,
};
use sonixx_core::chrome_bridge::{ChromeBridge, ChromeTab};
use sonixx_core::{MicSelection, Settings, APP_VERSION};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    System,
    Apps,
    Chrome,
    Mics,
}

#[derive(Clone)]
struct SoundEntry {
    name: String,
    path: PathBuf,
}

pub struct SonixxApp {
    engine: Engine,
    outputs: Vec<AudioDevice>,
    inputs: Vec<AudioDevice>,
    output_id: String,
    loopback_id: String,
    mic_choice: String,
    mics: Vec<AudioDevice>,
    sessions: Vec<AudioSession>,
    selected_apps: Vec<AudioSession>,
    app_choice_session: String,
    sounds: Vec<SoundEntry>,
    tab: Tab,
    status: String,
    settings: Settings,
    vb_cable_installed: bool,
    monitor: bool,
    last_sessions_refresh: f64,
    system_peak: f32,
    icons: HashMap<String, egui::TextureHandle>,
    show_silent: bool,
    chrome_bridge: Option<ChromeBridge>,
    chrome_tabs: Vec<ChromeTab>,
    bridge_error: Option<String>,
}

impl SonixxApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::configure(&cc.egui_ctx);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));

        let settings = Settings::load();
        let engine = Engine::new();
        engine.set_master_volume(settings.master_volume.clamp(0.0, 2.0));
        let (chrome_bridge, bridge_error) = match ChromeBridge::start() {
            Ok(bridge) => (Some(bridge), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };

        let mut app = Self {
            engine,
            outputs: Vec::new(),
            inputs: Vec::new(),
            output_id: settings.output_device.clone().unwrap_or_default(),
            loopback_id: settings.loopback_device.clone().unwrap_or_default(),
            mic_choice: String::new(),
            mics: Vec::new(),
            sessions: Vec::new(),
            selected_apps: Vec::new(),
            app_choice_session: String::new(),
            sounds: Vec::new(),
            tab: Tab::Apps,
            status: "Selecione a saída e a fonte de áudio do sistema.".to_string(),
            monitor: settings.monitor,
            settings: settings.clone(),
            vb_cable_installed: false,
            last_sessions_refresh: 0.0,
            system_peak: 0.0,
            icons: HashMap::new(),
            show_silent: true,
            chrome_bridge,
            chrome_tabs: Vec::new(),
            bridge_error,
        };
        app.reload_devices();
        app.refresh_sounds();

        if !settings.mics.is_empty() {
            app.mics = settings
                .mics
                .iter()
                .filter_map(|saved| {
                    app.inputs
                        .iter()
                        .find(|device| device.id == saved.id)
                        .cloned()
                })
                .collect();
        } else if let Ok(default) = default_device(DeviceFlow::Capture) {
            app.mics.push(default);
        }
        if !app.inputs.iter().any(|device| device.id == app.mic_choice) {
            app.mic_choice = app
                .mics
                .first()
                .map(|device| device.id.clone())
                .or_else(|| app.inputs.first().map(|device| device.id.clone()))
                .unwrap_or_default();
        }
        app
    }

    fn reload_devices(&mut self) {
        self.outputs = list_devices(DeviceFlow::Render).unwrap_or_default();
        self.inputs = list_devices(DeviceFlow::Capture).unwrap_or_default();
        self.vb_cable_installed = self
            .outputs
            .iter()
            .any(|device| device.name.to_lowercase().contains("cable input"));

        let default_out = default_device(DeviceFlow::Render).ok();
        let default_in = default_device(DeviceFlow::Capture).ok();
        let cable = self
            .outputs
            .iter()
            .find(|device| device.name.to_lowercase().contains("cable input"))
            .cloned();

        if !self
            .outputs
            .iter()
            .any(|device| device.id == self.output_id)
        {
            self.output_id = cable
                .map(|device| device.id)
                .or_else(|| default_out.as_ref().map(|device| device.id.clone()))
                .or_else(|| self.outputs.first().map(|device| device.id.clone()))
                .unwrap_or_default();
        }
        if !self
            .outputs
            .iter()
            .any(|device| device.id == self.loopback_id)
        {
            self.loopback_id = default_out
                .as_ref()
                .map(|device| device.id.clone())
                .or_else(|| self.outputs.first().map(|device| device.id.clone()))
                .unwrap_or_default();
        }
        if !self
            .inputs
            .iter()
            .any(|device| device.id == self.mic_choice)
        {
            self.mic_choice = default_in
                .as_ref()
                .map(|device| device.id.clone())
                .or_else(|| self.inputs.first().map(|device| device.id.clone()))
                .unwrap_or_default();
        }
        self.mics
            .retain(|mic| self.inputs.iter().any(|device| device.id == mic.id));
        self.refresh_sessions();
    }

    fn refresh_sessions(&mut self) {
        self.sessions = list_audio_sessions().unwrap_or_default();
        self.system_peak = default_output_peak().unwrap_or(0.0);
        self.selected_apps.retain(|selected| {
            self.sessions
                .iter()
                .any(|session| session.pid == selected.pid)
        });
        if !self
            .sessions
            .iter()
            .any(|session| session.session_id == self.app_choice_session)
        {
            self.app_choice_session = self
                .sessions
                .first()
                .map(|session| session.session_id.clone())
                .unwrap_or_default();
        }
    }

    fn refresh_sounds(&mut self) {
        let dir = sounds_dir();
        let mut sounds = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default()
                    .to_lowercase();
                if matches!(
                    extension.as_str(),
                    "wav" | "mp3" | "flac" | "ogg" | "aac" | "m4a"
                ) {
                    sounds.push(SoundEntry {
                        name: path
                            .file_stem()
                            .map(|stem| stem.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Som".to_string()),
                        path,
                    });
                }
            }
        }
        sounds.sort_by_key(|sound| sound.name.to_lowercase());
        self.sounds = sounds;
    }

    fn start_engine(&mut self) {
        if self.engine.is_running() {
            self.engine.stop();
        }
        let output = self
            .outputs
            .iter()
            .find(|device| device.id == self.output_id)
            .cloned();
        let loopback = self
            .outputs
            .iter()
            .find(|device| device.id == self.loopback_id)
            .cloned();
        match (output, loopback) {
            (Some(output_device), Some(loopback_device)) => {
                let config = EngineConfig {
                    output_device,
                    loopback_device,
                    app_sources: self
                        .selected_apps
                        .iter()
                        .map(|session| AppSource {
                            pid: session.pid,
                            name: session.name.clone(),
                            exe: session.exe.clone(),
                        })
                        .collect(),
                    mic_devices: self.mics.clone(),
                    master_volume: self.engine.master_volume(),
                    master_muted: self.engine.master_muted(),
                };
                match self.engine.start(config) {
                    Ok(()) => {
                        self.status = "Roteando. Os medidores mostram o áudio real.".to_string();
                    }
                    Err(error) => {
                        self.status = format!("Falha ao iniciar: {error:#}");
                    }
                }
            }
            _ => {
                self.status = "Selecione um dispositivo de saída e uma fonte de áudio do sistema."
                    .to_string();
            }
        }
    }

    fn icon_for(&mut self, ctx: &egui::Context, exe: Option<&str>) -> Option<egui::TextureHandle> {
        let exe = exe?;
        if let Some(texture) = self.icons.get(exe) {
            return Some(texture.clone());
        }
        let image = icons::load_exe_icon(exe)?;
        let texture = ctx.load_texture(exe.to_string(), image, egui::TextureOptions::LINEAR);
        self.icons.insert(exe.to_string(), texture.clone());
        Some(texture)
    }

    fn save_settings(&mut self) {
        self.settings.master_volume = self.engine.master_volume();
        self.settings.output_device = Some(self.output_id.clone());
        self.settings.loopback_device = Some(self.loopback_id.clone());
        self.settings.output_name = device_name(&self.outputs, &self.output_id);
        self.settings.loopback_name = device_name(&self.outputs, &self.loopback_id);
        self.settings.monitor = self.monitor;
        self.settings.mics = self
            .mics
            .iter()
            .map(|device| MicSelection {
                id: device.id.clone(),
                name: device.name.clone(),
            })
            .collect();
        self.status = match self.settings.save() {
            Ok(()) => format!("Configurações salvas em {}", Settings::path().display()),
            Err(error) => format!("Falha ao salvar configurações: {error}"),
        };
    }
}

impl eframe::App for SonixxApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = ui.ctx().input(|input| input.time);
        if now - self.last_sessions_refresh >= 0.5 {
            self.last_sessions_refresh = now;
            self.refresh_sessions();
            if let Some(bridge) = &self.chrome_bridge {
                self.chrome_tabs = bridge.tabs();
            }
        }

        let running = self.engine.is_running();
        let mut toggle_running = false;

        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                    ui.painter().circle_filled(rect.center(), 20.0, ACCENT);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "S",
                        egui::FontId::proportional(22.0),
                        Color32::WHITE,
                    );
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Sonixx").size(23.0).strong().color(TEXT));
                        ui.label(
                            RichText::new("Audio Router & Soundboard")
                                .size(11.0)
                                .color(MUTED),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = if running { "Parar" } else { "Iniciar" };
                        let fill = if running { RED } else { ACCENT };
                        if ui
                            .add_sized(
                                [104.0, 36.0],
                                egui::Button::new(RichText::new(label).strong()).fill(fill),
                            )
                            .clicked()
                        {
                            toggle_running = true;
                        }
                        ui.add_space(10.0);

                        let (pill_fill, dot, text) = if running {
                            (Color32::from_rgb(22, 58, 42), GREEN, "Roteando")
                        } else {
                            (Color32::from_rgb(36, 37, 52), MUTED, "Parado")
                        };
                        egui::Frame::new()
                            .fill(pill_fill)
                            .corner_radius(egui::CornerRadius::same(20))
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(8.0, 8.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(rect.center(), 4.0, dot);
                                    ui.label(RichText::new(text).size(12.0).color(dot));
                                });
                            });
                    });
                });
            });

        if toggle_running {
            if self.engine.is_running() {
                self.engine.stop();
                self.status = "Parado.".to_string();
            } else {
                self.start_engine();
            }
        }

        let status = self.engine.error().unwrap_or_else(|| self.status.clone());
        let status_color = if self.engine.error().is_some() {
            RED
        } else {
            MUTED
        };
        let sounds = self.sounds.clone();
        let soundboard_running = self.engine.is_running();
        let mut play_sound = None;
        let mut refresh_sounds = false;
        let mut open_sounds = false;
        let mut soundboard_volume = self.engine.soundboard_volume();
        let mut soundboard_muted = self.engine.soundboard_muted();

        egui::Panel::bottom("soundboard")
            .exact_size(142.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("SOUNDBOARD").size(11.0).strong().color(MUTED));
                    if sounds.is_empty() {
                        ui.label(
                            RichText::new("Coloque arquivos .wav/.mp3 na pasta sounds/.")
                                .size(11.0)
                                .color(MUTED),
                        );
                    } else {
                        for (index, sound) in sounds.iter().take(8).enumerate() {
                            if ui
                                .add_enabled(
                                    soundboard_running,
                                    egui::Button::new(sound.name.as_str())
                                        .min_size(egui::vec2(104.0, 30.0)),
                                )
                                .on_disabled_hover_text("Inicie a engine para tocar o soundboard.")
                                .clicked()
                            {
                                play_sound = Some(index);
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized([140.0, 30.0], egui::Button::new("Abrir pasta"))
                            .clicked()
                        {
                            open_sounds = true;
                        }
                        if ui
                            .add_sized([100.0, 30.0], egui::Button::new("Atualizar"))
                            .clicked()
                        {
                            refresh_sounds = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Volume").size(11.0).color(MUTED));
                    ui.spacing_mut().slider_width = 140.0;
                    ui.add(egui::Slider::new(&mut soundboard_volume, 0.0..=2.0).show_value(false));
                    ui.label(
                        RichText::new(format!("{:.0}%", soundboard_volume * 100.0))
                            .size(11.0)
                            .color(TEXT),
                    );
                    if ui
                        .add_sized(
                            [64.0, 24.0],
                            egui::Button::new(if soundboard_muted { "Mudo" } else { "Som" }),
                        )
                        .clicked()
                    {
                        soundboard_muted = !soundboard_muted;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(status).size(11.0).color(status_color));
                    });
                });
            });

        self.engine.set_soundboard_volume(soundboard_volume);
        self.engine.set_soundboard_muted(soundboard_muted);
        if let Some(index) = play_sound {
            if let Some(sound) = self.sounds.get(index) {
                match self.engine.play_sound(&sound.path) {
                    Ok(()) => self.status = format!("Tocando: {}", sound.name),
                    Err(error) => self.status = format!("Falha ao tocar {}: {error:#}", sound.name),
                }
            }
        }
        if refresh_sounds {
            self.refresh_sounds();
        }
        if open_sounds {
            let _ = std::process::Command::new("explorer")
                .arg(sounds_dir())
                .spawn();
        }

        let outputs = &self.outputs;
        let inputs = &self.inputs;
        let mut output_id = self.output_id.clone();
        let mut loopback_id = self.loopback_id.clone();
        let mut mic_choice = self.mic_choice.clone();
        let mics = self.mics.clone();
        let sessions = self.sessions.clone();
        let selected_apps = self.selected_apps.clone();
        let mut app_choice_session = self.app_choice_session.clone();
        let mut add_app = false;
        let mut remove_app = None;
        let mut refresh_apps = false;
        let mut master_volume = self.engine.master_volume();
        let mut master_muted = self.engine.master_muted();
        let mut monitor = self.monitor;
        let mut add_mic = false;
        let mut remove_mic = None;
        let mut save = false;
        let mut reload = false;
        let running = self.engine.is_running();
        let output_rate = self.engine.output_rate();
        let source_count = self.engine.sources().len();
        let engine_error = self.engine.error();
        let master_peak = self.engine.master_peak();
        let system_peak = self.system_peak;
        let vb_installed = self.vb_cable_installed;

        egui::Panel::right("output")
            .exact_size(360.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label(RichText::new("SAÍDA").size(11.0).strong().color(MUTED));
                    ui.add_space(4.0);
                    theme::card_frame().show(ui, |ui| {
                        ui.label(RichText::new("Dispositivo de saída").size(11.0).color(MUTED));
                        device_combo(ui, "output-device", &mut output_id, outputs);
                        if !vb_installed {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(
                                    "VB-Cable não instalado. A saída será um dispositivo normal, não um microfone virtual.",
                                )
                                .size(10.0)
                                .color(AMBER),
                            );
                        }

                        ui.add_space(10.0);
                        ui.label(RichText::new("Capturar áudio de").size(11.0).color(MUTED));
                        device_combo(ui, "loopback-device", &mut loopback_id, outputs);
                        ui.label(
                            RichText::new("Loopback WASAPI do dispositivo selecionado.")
                                .size(10.0)
                                .color(MUTED),
                        );

                        ui.add_space(12.0);
                        ui.label(RichText::new("Volume máster").size(11.0).color(MUTED));
                        let slider_width = (ui.available_width() - 8.0).max(80.0);
                        ui.spacing_mut().slider_width = slider_width;
                        ui.add(egui::Slider::new(&mut master_volume, 0.0..=2.0).show_value(false));
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{:.0}%", master_volume * 100.0))
                                    .size(12.0)
                                    .color(TEXT),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_sized(
                                            [70.0, 26.0],
                                            egui::Button::new(if master_muted { "Mudo" } else { "Som" }),
                                        )
                                        .clicked()
                                    {
                                        master_muted = !master_muted;
                                    }
                                },
                            );
                        });
                        ui.add_space(6.0);
                        theme::meter(ui, if master_muted { 0.0 } else { master_peak }, 8.0);
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Atividade do PC (detector)")
                                .size(10.0)
                                .color(MUTED),
                        );
                        theme::meter(ui, system_peak, 6.0);
                        ui.add_space(8.0);
                        ui.add_enabled(false, egui::Checkbox::new(&mut monitor, "Monitor mix"))
                            .on_disabled_hover_text("Monitor mix entra na próxima fase.");
                    });

                    ui.add_space(12.0);
                    ui.label(RichText::new("APLICATIVOS").size(11.0).strong().color(MUTED));
                    ui.add_space(4.0);
                    theme::card_frame().show(ui, |ui| {
                        session_combo(ui, "app-choice", &mut app_choice_session, &sessions);
                        let add_width = ui.available_width();
                        if ui
                            .add_enabled(
                                !app_choice_session.is_empty(),
                                egui::Button::new("+ Adicionar aplicativo")
                                    .min_size(egui::vec2(add_width, 28.0)),
                            )
                            .clicked()
                        {
                            add_app = true;
                        }
                        if sessions.is_empty() {
                            ui.label(
                                RichText::new(
                                    "Nenhum app tocando áudio. Abra um player e clique em Atualizar.",
                                )
                                .size(10.0)
                                .color(MUTED),
                            );
                        }
                        for (index, app) in selected_apps.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(app.name.as_str()).size(11.0).color(TEXT));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_sized([64.0, 22.0], egui::Button::new("Remover"))
                                            .clicked()
                                        {
                                            remove_app = Some(index);
                                        }
                                    },
                                );
                            });
                        }
                        let refresh_width = ui.available_width();
                        if ui
                            .add_sized([refresh_width, 24.0], egui::Button::new("Atualizar aplicativos"))
                            .clicked()
                        {
                            refresh_apps = true;
                        }
                    });

                    ui.add_space(12.0);
                    ui.label(RichText::new("MICROFONES").size(11.0).strong().color(MUTED));
                    ui.add_space(4.0);
                    theme::card_frame().show(ui, |ui| {
                        device_combo(ui, "mic-choice", &mut mic_choice, inputs);
                        let add_width = ui.available_width();
                        if ui
                            .add_enabled(
                                !mic_choice.is_empty(),
                                egui::Button::new("+ Adicionar microfone")
                                    .min_size(egui::vec2(add_width, 28.0)),
                            )
                            .clicked()
                        {
                            add_mic = true;
                        }
                        ui.add_space(6.0);
                        if mics.is_empty() {
                            ui.label(
                                RichText::new("Nenhum microfone adicionado.")
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        }
                        for (index, mic) in mics.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(mic.name.as_str()).size(11.0).color(TEXT));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .add_sized([64.0, 22.0], egui::Button::new("Remover"))
                                            .clicked()
                                        {
                                            remove_mic = Some(index);
                                        }
                                    },
                                );
                            });
                        }
                    });

                    ui.add_space(12.0);
                    ui.label(RichText::new("STATUS").size(11.0).strong().color(MUTED));
                    ui.add_space(4.0);
                    theme::card_frame().show(ui, |ui| {
                        status_row(ui, "Engine", if running { "rodando" } else { "parada" });
                        status_row(
                            ui,
                            "Taxa de saída",
                            &if output_rate > 0 {
                                format!("{output_rate} Hz")
                            } else {
                                "—".to_string()
                            },
                        );
                        status_row(ui, "Fontes", &source_count.to_string());
                        status_row(
                            ui,
                            "VB-Cable",
                            if vb_installed {
                                "instalado"
                            } else {
                                "não instalado"
                            },
                        );
                        if let Some(error) = &engine_error {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("Erro: {error}"))
                                    .size(10.0)
                                    .color(RED),
                            );
                        }
                    });

                    ui.add_space(12.0);
                    let width = ui.available_width();
                    if ui
                        .add_sized([width, 30.0], egui::Button::new("Atualizar dispositivos"))
                        .clicked()
                    {
                        reload = true;
                    }
                    ui.add_space(4.0);
                    if ui
                        .add_sized([width, 30.0], egui::Button::new("Salvar configurações"))
                        .clicked()
                    {
                        save = true;
                    }
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("Sonixx v{APP_VERSION} · Rust"))
                            .size(10.0)
                            .color(MUTED),
                    );
                });
            });

        self.output_id = output_id;
        self.loopback_id = loopback_id;
        self.mic_choice = mic_choice;
        self.mics = mics;
        self.selected_apps = selected_apps;
        self.app_choice_session = app_choice_session;
        self.monitor = monitor;
        self.engine.set_master_volume(master_volume);
        self.engine.set_master_muted(master_muted);
        if add_app {
            if let Some(session) = sessions
                .iter()
                .find(|session| session.session_id == self.app_choice_session)
                .cloned()
            {
                if !self
                    .selected_apps
                    .iter()
                    .any(|selected| selected.session_id == session.session_id)
                {
                    self.selected_apps.push(session);
                }
            }
        }
        if let Some(index) = remove_app {
            if index < self.selected_apps.len() {
                self.selected_apps.remove(index);
            }
        }
        if add_mic {
            if let Some(device) = inputs
                .iter()
                .find(|device| device.id == self.mic_choice)
                .cloned()
            {
                if !self.mics.iter().any(|mic| mic.id == device.id) {
                    self.mics.push(device);
                }
            }
        }
        if let Some(index) = remove_mic {
            if index < self.mics.len() {
                self.mics.remove(index);
            }
        }
        if refresh_apps {
            self.refresh_sessions();
        }
        if save {
            self.save_settings();
        }
        if reload {
            self.reload_devices();
        }

        let running = self.engine.is_running();
        let mut tab = self.tab;
        let detected_sessions = self.sessions.clone();
        let chrome_tabs = self.chrome_tabs.clone();
        let bridge_error = self.bridge_error.clone();
        let bridge_port = self
            .chrome_bridge
            .as_ref()
            .map(|bridge| bridge.port())
            .unwrap_or(37_645);
        let mut open_extension = false;
        let has_chrome = detected_sessions.iter().any(|session| {
            session
                .exe
                .as_deref()
                .map(|exe| exe.to_lowercase().ends_with("chrome.exe"))
                .unwrap_or(false)
        });
        let mut show_silent = self.show_silent;
        let detected_app_count = if show_silent {
            detected_sessions.len()
        } else {
            detected_sessions
                .iter()
                .filter(|session| session.peak > 0.001)
                .count()
        };
        let sources: Vec<SourceControl> = self.engine.sources().to_vec();
        let chrome_exe = detected_sessions
            .iter()
            .find_map(|session| {
                session
                    .exe
                    .clone()
                    .filter(|exe| exe.to_lowercase().ends_with("chrome.exe"))
            })
            .or_else(|| {
                let path = "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe".to_string();
                std::path::Path::new(&path).exists().then_some(path)
            });
        let chrome_icon = self.icon_for(ui.ctx(), chrome_exe.as_deref());
        let detected_views: Vec<(AudioSession, Option<egui::TextureHandle>)> = detected_sessions
            .iter()
            .map(|session| {
                (
                    session.clone(),
                    self.icon_for(ui.ctx(), session.exe.as_deref()),
                )
            })
            .collect();
        let source_views: Vec<(SourceControl, Option<egui::TextureHandle>)> = sources
            .iter()
            .map(|source| {
                (
                    source.clone(),
                    self.icon_for(ui.ctx(), source.exe.as_deref()),
                )
            })
            .collect();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Fontes de áudio").size(20.0).strong());
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(if running {
                                    "Áudio real (WASAPI)"
                                } else {
                                    "Engine parada"
                                })
                                .size(11.0)
                                .color(if running { GREEN } else { MUTED }),
                            );
                        },
                    );
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut tab, Tab::System, "Áudio do sistema");
                    let app_count = if running {
                        sources
                            .iter()
                            .filter(|source| source.kind == SourceKind::App)
                            .count()
                    } else {
                        detected_app_count
                    };
                    ui.selectable_value(&mut tab, Tab::Apps, format!("Aplicativos ({app_count})"));
                    ui.selectable_value(
                        &mut tab,
                        Tab::Chrome,
                        format!("Chrome ({})", chrome_tabs.len()),
                    );
                    ui.selectable_value(
                        &mut tab,
                        Tab::Mics,
                        format!(
                            "Microfones ({})",
                            sources
                                .iter()
                                .filter(|source| source.kind == SourceKind::Mic)
                                .count()
                        ),
                    );
                    if tab == Tab::Apps {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut show_silent, "Mostrar silenciosos");
                        });
                    }
                });
                if tab == Tab::Apps && has_chrome {
                    ui.label(
                        RichText::new(
                            "O Windows expõe o Chrome como uma sessão única; abas não são separadas pelo WASAPI.",
                        )
                        .size(10.0)
                        .color(AMBER),
                    );
                }
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if tab == Tab::Chrome {
                            if chrome_tabs.is_empty() {
                                chrome_setup_card(
                                    ui,
                                    bridge_error.as_deref(),
                                    bridge_port,
                                    &mut open_extension,
                                );
                            } else {
                                for chrome_tab in &chrome_tabs {
                                    chrome_tab_card(ui, chrome_tab, chrome_icon.as_ref());
                                }
                                ui.add_space(4.0);
                                if ui.button("Abrir pasta da extensão").clicked() {
                                    open_extension = true;
                                }
                            }
                            return;
                        }
                        if !running {
                            if tab == Tab::Apps {
                                let visible: Vec<&(AudioSession, Option<egui::TextureHandle>)> =
                                    detected_views
                                        .iter()
                                        .filter(|(session, _)| {
                                            show_silent || session.peak > 0.001
                                        })
                                        .collect();
                                if visible.is_empty() {
                                    empty_state(
                                        ui,
                                        if show_silent {
                                            "Nenhuma sessão de áudio encontrada.\nAbra um player e toque algo — o detector lê o medidor de cada sessão."
                                        } else {
                                            "Nenhum app tocando agora.\nMarque “Mostrar silenciosos” para ver todas as sessões."
                                        },
                                    );
                                } else {
                                    for (session, icon) in visible {
                                        detected_session_card(ui, session, icon.as_ref());
                                    }
                                }
                            } else {
                                empty_state(
                                    ui,
                                    "A engine está parada.\nEscolha a saída e a fonte de áudio do sistema e clique em Iniciar.",
                                );
                            }
                            return;
                        }
                        let filtered: Vec<&(SourceControl, Option<egui::TextureHandle>)> =
                            source_views
                                .iter()
                                .filter(|(source, _)| match tab {
                                    Tab::System => source.kind == SourceKind::System,
                                    Tab::Apps => source.kind == SourceKind::App,
                                    Tab::Mics => source.kind == SourceKind::Mic,
                                    Tab::Chrome => false,
                                })
                                .collect();
                        if filtered.is_empty() {
                            empty_state(
                                ui,
                                match tab {
                                    Tab::Apps => {
                                        "Nenhum aplicativo na mixagem.\nAdicione um app no painel direito e clique em Iniciar."
                                    }
                                    Tab::Mics => {
                                        "Nenhum microfone na mixagem.\nAdicione um microfone no painel direito e clique em Iniciar."
                                    }
                                    Tab::System => "Nenhuma fonte nesta categoria.",
                                    Tab::Chrome => "",
                                },
                            );
                        } else {
                            for (source, icon) in filtered {
                                source_card(ui, source, icon.as_ref());
                            }
                        }
                    });
            });

        self.tab = tab;
        self.show_silent = show_silent;
        if open_extension {
            let _ = std::process::Command::new("explorer")
                .arg(chrome_extension_dir())
                .spawn();
        }
    }
}

fn device_name(devices: &[AudioDevice], id: &str) -> String {
    devices
        .iter()
        .find(|device| device.id == id)
        .map(|device| device.name.clone())
        .unwrap_or_else(|| "—".to_string())
}

fn device_combo(ui: &mut egui::Ui, salt: &str, selected: &mut String, devices: &[AudioDevice]) {
    let current = devices
        .iter()
        .find(|device| &device.id == selected)
        .map(|device| device.name.clone())
        .unwrap_or_else(|| "Selecionar...".to_string());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for device in devices {
                ui.selectable_value(selected, device.id.clone(), &device.name);
            }
        });
}

fn session_label(session: &AudioSession) -> String {
    let activity = if session.peak > 0.005 {
        " • tocando"
    } else {
        ""
    };
    format!("{} (PID {}){activity}", session.name, session.pid)
}

fn session_combo(ui: &mut egui::Ui, salt: &str, selected: &mut String, sessions: &[AudioSession]) {
    let current = sessions
        .iter()
        .find(|session| session.session_id == *selected)
        .map(session_label)
        .unwrap_or_else(|| "Selecionar...".to_string());
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for session in sessions {
                ui.selectable_value(selected, session.session_id.clone(), session_label(session));
            }
        });
}

fn app_icon(ui: &mut egui::Ui, icon: Option<&egui::TextureHandle>, name: &str) {
    if let Some(texture) = icon {
        ui.add(egui::Image::from_texture((texture.id(), egui::vec2(34.0, 34.0))).corner_radius(8));
    } else {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 17.0, ACCENT_SOFT);
        let initial = name.chars().next().unwrap_or('?').to_string();
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            initial,
            egui::FontId::proportional(15.0),
            ACCENT,
        );
    }
}

fn detected_session_card(
    ui: &mut egui::Ui,
    session: &AudioSession,
    icon: Option<&egui::TextureHandle>,
) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            app_icon(ui, icon, &session.name);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(session.name.as_str()).strong().size(14.0));
                ui.label(
                    RichText::new(format!("Sessão de áudio · PID {}", session.pid))
                        .size(11.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, color) = if session.peak > 0.005 {
                    ("tocando", GREEN)
                } else {
                    ("silencioso", MUTED)
                };
                ui.label(RichText::new(text).size(11.0).color(color));
            });
        });
        ui.add_space(4.0);
        theme::meter(ui, session.peak, 6.0);
    });
}

fn chrome_extension_dir() -> PathBuf {
    if let Ok(current) = std::env::current_dir() {
        let dir = current.join("chrome-extension");
        if dir.is_dir() {
            return dir;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dir = parent.join("chrome-extension");
            if dir.is_dir() {
                return dir;
            }
        }
    }
    PathBuf::from("chrome-extension")
}

fn chrome_setup_card(ui: &mut egui::Ui, error: Option<&str>, port: u16, open_folder: &mut bool) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(
            RichText::new("Detectar abas do Chrome")
                .size(16.0)
                .strong(),
        );
        ui.add_space(4.0);
        ui.label(
            "O Windows expõe o Chrome como uma sessão única. Para ver as abas que estão tocando, instale a extensão Sonixx Tab Bridge:",
        );
        ui.label("1. Abra chrome://extensions");
        ui.label("2. Ative o Modo do desenvolvedor");
        ui.label("3. Clique em Carregar sem compactação e escolha a pasta chrome-extension");
        ui.label(format!(
            "A ponte local escuta em 127.0.0.1:{port}. O Sonixx precisa estar aberto."
        ));
        ui.add_space(6.0);
        if ui.button("Abrir pasta da extensão").clicked() {
            *open_folder = true;
        }
        if let Some(error) = error {
            ui.add_space(4.0);
            ui.label(RichText::new(error).size(10.0).color(RED));
        }
    });
}

fn chrome_tab_card(ui: &mut egui::Ui, tab: &ChromeTab, icon: Option<&egui::TextureHandle>) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            app_icon(ui, icon, "C");
            ui.add_space(4.0);
            ui.vertical(|ui| {
                let title = if tab.title.trim().is_empty() {
                    "Aba do Chrome"
                } else {
                    tab.title.as_str()
                };
                ui.label(RichText::new(title).strong().size(14.0));
                let domain = tab
                    .url
                    .split("://")
                    .nth(1)
                    .and_then(|rest| rest.split('/').next())
                    .unwrap_or(tab.url.as_str());
                ui.label(RichText::new(domain).size(11.0).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, color) = if tab.audible {
                    ("tocando", GREEN)
                } else {
                    ("silenciosa", MUTED)
                };
                ui.label(RichText::new(text).size(11.0).color(color));
            });
        });
    });
}

fn sounds_dir() -> PathBuf {
    if let Ok(current) = std::env::current_dir() {
        let dir = current.join("sounds");
        if dir.is_dir() {
            return dir;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dir = parent.join("sounds");
            if dir.is_dir() {
                return dir;
            }
        }
    }
    PathBuf::from("sounds")
}

fn status_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(11.0).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(11.0).strong().color(TEXT));
        });
    });
}

fn empty_state(ui: &mut egui::Ui, text: &str) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.vertical_centered(|ui| {
            ui.add_space(18.0);
            ui.label(RichText::new(text).size(13.0).color(MUTED));
            ui.add_space(18.0);
        });
    });
}

fn source_card(ui: &mut egui::Ui, source: &SourceControl, icon: Option<&egui::TextureHandle>) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            app_icon(ui, icon, &source.name);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(RichText::new(source.name.as_str()).strong().size(14.0));
                ui.label(
                    RichText::new(source.detail.as_str())
                        .size(11.0)
                        .color(MUTED),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut active = source.active();
                if ui.checkbox(&mut active, "Ativo").changed() {
                    source.set_active(active);
                }
                if ui
                    .add_sized(
                        [58.0, 26.0],
                        egui::Button::new(if source.muted() { "Mudo" } else { "Som" }),
                    )
                    .clicked()
                {
                    source.set_muted(!source.muted());
                }
            });
        });

        ui.add_space(6.0);
        let mut volume = source.volume();
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = 190.0;
            if ui
                .add(egui::Slider::new(&mut volume, 0.0..=2.0).show_value(false))
                .changed()
            {
                source.set_volume(volume);
            }
            ui.label(
                RichText::new(format!("{:.0}%", volume * 100.0))
                    .size(11.0)
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{} Hz", source.sample_rate()))
                        .size(10.0)
                        .color(MUTED),
                );
            });
        });
        ui.add_space(4.0);
        theme::meter(ui, source.peak(), 6.0);
    });
}
