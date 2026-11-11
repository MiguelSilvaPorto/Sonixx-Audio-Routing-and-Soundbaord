use crate::theme::{self, *};
use eframe::egui::{self, Color32, RichText};
use sonixx_core::{Settings, APP_VERSION};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Apps,
    Mics,
}

struct Source {
    name: String,
    detail: String,
    volume: f32,
    active: bool,
    muted: bool,
    peak: f32,
    phase: f32,
}

impl Source {
    fn new(name: impl Into<String>, detail: impl Into<String>, volume: f32, phase: f32) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            volume,
            active: true,
            muted: false,
            peak: 0.0,
            phase,
        }
    }
}

pub struct SonixxApp {
    running: bool,
    master_volume: f32,
    master_muted: bool,
    monitor: bool,
    tab: Tab,
    apps: Vec<Source>,
    mics: Vec<Source>,
    settings: Settings,
    status: String,
}

impl SonixxApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::configure(&cc.egui_ctx);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::SetTheme(egui::SystemTheme::Dark));
        let settings = Settings::load();
        let master_volume = settings.master_volume.clamp(0.0, 2.0);
        let monitor = settings.monitor;
        Self {
            running: false,
            master_volume,
            master_muted: false,
            monitor,
            tab: Tab::Apps,
            apps: vec![
                Source::new("Google Chrome", "Janela: YouTube", 0.82, 1.1),
                Source::new("Spotify", "Reproduzindo música", 0.68, 2.7),
                Source::new("Discord", "Chamada de voz", 0.55, 4.2),
            ],
            mics: vec![Source::new(
                "Headset Microfone",
                "WASAPI · 48 kHz",
                1.0,
                0.4,
            )],
            settings,
            status: "Interface Rust pronta. Engine de áudio na próxima fase.".to_string(),
        }
    }

    fn animate(&mut self, ctx: &egui::Context) {
        let time = ctx.input(|input| input.time) as f32;
        let running = self.running;
        for source in self.apps.iter_mut().chain(self.mics.iter_mut()) {
            if running && source.active && !source.muted {
                source.peak = 0.22 + 0.62 * ((time * 1.8 + source.phase).sin().abs());
            } else {
                source.peak *= 0.82;
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    fn master_peak(&self) -> f32 {
        if self.master_muted {
            return 0.0;
        }
        let peak = self
            .apps
            .iter()
            .chain(self.mics.iter())
            .filter(|source| source.active && !source.muted)
            .map(|source| source.peak)
            .fold(0.0, f32::max);
        (peak * self.master_volume).clamp(0.0, 1.0)
    }

    fn save_settings(&mut self) {
        self.settings.master_volume = self.master_volume;
        self.settings.monitor = self.monitor;
        self.status = match self.settings.save() {
            Ok(()) => format!("Configurações salvas em {}", Settings::path().display()),
            Err(error) => format!("Falha ao salvar configurações: {error}"),
        };
    }
}

impl eframe::App for SonixxApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.animate(ui.ctx());

        let running = self.running;
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
            self.running = !self.running;
            self.status = if self.running {
                "Roteando (simulação de interface). A engine de áudio entra na fase 2.".to_string()
            } else {
                "Parado.".to_string()
            };
        }

        let mut play = None;
        let mut open_folder = false;
        let status = self.status.clone();

        egui::Panel::bottom("soundboard")
            .exact_size(96.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("SOUNDBOARD").size(11.0).strong().color(MUTED));
                    for name in ["Airhorn", "Aplausos", "Risada", "Tambor"] {
                        if ui
                            .add_sized([96.0, 30.0], egui::Button::new(name))
                            .clicked()
                        {
                            play = Some(name);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized([150.0, 30.0], egui::Button::new("Abrir pasta de sons"))
                            .clicked()
                        {
                            open_folder = true;
                        }
                    });
                });
                ui.add_space(6.0);
                ui.label(RichText::new(status).size(11.0).color(MUTED));
            });

        if let Some(name) = play {
            self.status = format!("Tocando: {name} (engine de áudio na próxima fase).");
        }
        if open_folder {
            self.status = format!("Pasta de sons: {}", "sounds/");
        }

        let mut master_volume = self.master_volume;
        let mut master_muted = self.master_muted;
        let mut monitor = self.monitor;
        let mut save = false;
        let master_peak = self.master_peak();
        let app_count = self.apps.len();
        let mic_count = self.mics.len();

        egui::Panel::right("output")
            .exact_size(332.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(egui::Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ui, |ui| {
                ui.label(RichText::new("SAÍDA").size(11.0).strong().color(MUTED));
                ui.add_space(4.0);
                theme::card_frame().show(ui, |ui| {
                    ui.label(RichText::new("CABLE Input").size(15.0).strong());
                    ui.label(
                        RichText::new("VB-Audio Virtual Cable")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new("Volume máster").size(12.0).color(MUTED));
                    let slider_width = (ui.available_width() - 8.0).max(80.0);
                    ui.spacing_mut().slider_width = slider_width;
                    ui.add(egui::Slider::new(&mut master_volume, 0.0..=2.0).show_value(false));
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:.0}%", master_volume * 100.0))
                                .size(12.0)
                                .color(TEXT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add_sized(
                                    [70.0, 26.0],
                                    egui::Button::new(if master_muted { "Mudo" } else { "Som" }),
                                )
                                .clicked()
                            {
                                master_muted = !master_muted;
                            }
                        });
                    });
                    ui.add_space(6.0);
                    theme::meter(ui, if master_muted { 0.0 } else { master_peak }, 8.0);
                    ui.add_space(10.0);
                    ui.checkbox(&mut monitor, "Monitor mix (ouvir nos fones)");
                });

                ui.add_space(12.0);
                ui.label(RichText::new("STATUS").size(11.0).strong().color(MUTED));
                ui.add_space(4.0);
                theme::card_frame().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Aplicativos").size(12.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{app_count}"))
                                    .size(12.0)
                                    .strong()
                                    .color(TEXT),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Microfones").size(12.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{mic_count}"))
                                    .size(12.0)
                                    .strong()
                                    .color(TEXT),
                            );
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Taxa").size(12.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("48 kHz").size(12.0).color(TEXT));
                        });
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Engine").size(12.0).color(MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new("em construção").size(12.0).color(AMBER));
                        });
                    });
                });

                ui.add_space(12.0);
                let save_width = ui.available_width();
                if ui
                    .add_sized(
                        [save_width, 32.0],
                        egui::Button::new("Salvar configurações"),
                    )
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

        self.master_volume = master_volume;
        self.master_muted = master_muted;
        self.monitor = monitor;
        if save {
            self.save_settings();
        }

        let running = self.running;
        let mut tab = self.tab;
        let mut add_app = false;
        let mut add_mic = false;
        let mut remove_app = None;
        let mut remove_mic = None;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Fontes de áudio").size(20.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized([156.0, 30.0], egui::Button::new("+ Adicionar microfone"))
                            .clicked()
                        {
                            add_mic = true;
                        }
                        if ui
                            .add_sized([140.0, 30.0], egui::Button::new("+ Adicionar app"))
                            .clicked()
                        {
                            add_app = true;
                        }
                    });
                });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut tab,
                        Tab::Apps,
                        format!("Aplicativos ({})", self.apps.len()),
                    );
                    ui.selectable_value(
                        &mut tab,
                        Tab::Mics,
                        format!("Microfones ({})", self.mics.len()),
                    );
                });
                ui.add_space(8.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match tab {
                        Tab::Apps => {
                            if self.apps.is_empty() {
                                empty_state(ui, "Nenhum aplicativo na mixagem.");
                            }
                            for (index, source) in self.apps.iter_mut().enumerate() {
                                if source_card(ui, source, running) {
                                    remove_app = Some(index);
                                }
                            }
                        }
                        Tab::Mics => {
                            if self.mics.is_empty() {
                                empty_state(ui, "Nenhum microfone na mixagem.");
                            }
                            for (index, source) in self.mics.iter_mut().enumerate() {
                                if source_card(ui, source, running) {
                                    remove_mic = Some(index);
                                }
                            }
                        }
                    });
            });

        self.tab = tab;
        if add_app {
            let index = self.apps.len() + 1;
            self.apps.push(Source::new(
                format!("Aplicativo {index}"),
                "Detectado na próxima varredura",
                0.7,
                index as f32,
            ));
        }
        if add_mic {
            let index = self.mics.len() + 1;
            self.mics.push(Source::new(
                format!("Microfone {index}"),
                "WASAPI · 48 kHz",
                1.0,
                index as f32,
            ));
        }
        if let Some(index) = remove_app {
            self.apps.remove(index);
        }
        if let Some(index) = remove_mic {
            self.mics.remove(index);
        }
    }
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

fn source_card(ui: &mut egui::Ui, source: &mut Source, running: bool) -> bool {
    let mut remove = false;
    theme::card_frame().show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(34.0, 34.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 17.0, ACCENT_SOFT);
            let initial = source.name.chars().next().unwrap_or('?').to_string();
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                initial,
                egui::FontId::proportional(15.0),
                ACCENT,
            );
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
                if ui
                    .add_sized([74.0, 26.0], egui::Button::new("Remover"))
                    .clicked()
                {
                    remove = true;
                }
                if ui
                    .add_sized(
                        [58.0, 26.0],
                        egui::Button::new(if source.muted { "Mudo" } else { "Som" }),
                    )
                    .clicked()
                {
                    source.muted = !source.muted;
                }
            });
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = 190.0;
            ui.add(egui::Slider::new(&mut source.volume, 0.0..=2.0).show_value(false));
            ui.label(
                RichText::new(format!("{:.0}%", source.volume * 100.0))
                    .size(11.0)
                    .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.checkbox(&mut source.active, "Ativo");
            });
        });
        ui.add_space(4.0);
        theme::meter(
            ui,
            if running && source.active && !source.muted {
                source.peak
            } else {
                0.0
            },
            6.0,
        );
    });
    remove
}
