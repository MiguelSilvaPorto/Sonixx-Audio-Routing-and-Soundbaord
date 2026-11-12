use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const BG: Color32 = Color32::from_rgb(13, 14, 20);
pub const PANEL: Color32 = Color32::from_rgb(18, 19, 27);
pub const CARD: Color32 = Color32::from_rgb(24, 25, 36);
pub const CARD_HOVER: Color32 = Color32::from_rgb(31, 32, 46);
pub const BORDER: Color32 = Color32::from_rgb(38, 40, 56);
pub const TEXT: Color32 = Color32::from_rgb(232, 234, 245);
pub const MUTED: Color32 = Color32::from_rgb(138, 142, 162);
pub const ACCENT: Color32 = Color32::from_rgb(124, 92, 255);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(44, 35, 78);
pub const GREEN: Color32 = Color32::from_rgb(52, 199, 123);
pub const AMBER: Color32 = Color32::from_rgb(240, 180, 70);
pub const RED: Color32 = Color32::from_rgb(235, 87, 87);
pub const METER_BG: Color32 = Color32::from_rgb(9, 10, 15);
pub const SLIDER_TRACK: Color32 = Color32::from_rgb(43, 45, 64);
pub const SLIDER_HANDLE: Color32 = Color32::from_rgb(245, 246, 255);

pub fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = BG;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = METER_BG;
    style.visuals.faint_bg_color = CARD;
    style.visuals.widgets.noninteractive.bg_fill = PANEL;
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.inactive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, MUTED);
    style.visuals.widgets.hovered.bg_fill = CARD_HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = CARD_HOVER;
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    style.visuals.widgets.active.bg_fill = ACCENT;
    style.visuals.widgets.active.weak_bg_fill = ACCENT;
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
    style.visuals.selection.bg_fill = ACCENT;
    style.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    ctx.set_style_of(egui::Theme::Dark, style);
}

pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(egui::Margin::same(12))
        .corner_radius(CornerRadius::same(12))
}

/// Horizontal slider with a visible track, accent fill and a large handle.
pub fn accent_slider(ui: &mut egui::Ui, value: &mut f32, min: f32, max: f32, width: f32) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::click_and_drag());
    let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 6.0));
    let painter = ui.painter();
    painter.rect_filled(track, CornerRadius::same(3), SLIDER_TRACK);

    let range = (max - min).max(f32::EPSILON);
    let t = ((*value - min) / range).clamp(0.0, 1.0);
    if t > 0.0 {
        let fill =
            egui::Rect::from_min_size(track.min, egui::vec2(track.width() * t, track.height()));
        painter.rect_filled(fill, CornerRadius::same(3), ACCENT);
    }

    let center = egui::pos2(track.left() + track.width() * t, rect.center().y);
    let radius = if response.hovered() || response.dragged() {
        8.5
    } else {
        7.0
    };
    painter.circle_filled(
        center,
        radius + 3.0,
        Color32::from_rgba_unmultiplied(0, 0, 0, 90),
    );
    painter.circle_filled(center, radius, SLIDER_HANDLE);
    painter.circle_stroke(center, radius, Stroke::new(2.0, ACCENT));

    let mut changed = false;
    if response.dragged() || response.clicked() {
        if let Some(position) = response.interact_pointer_pos() {
            let new_t = ((position.x - track.left()) / track.width()).clamp(0.0, 1.0);
            let new_value = min + new_t * range;
            if (new_value - *value).abs() > 0.0001 {
                *value = new_value;
                changed = true;
            }
        }
    }
    changed
}

/// Filled sparkline of recent audio activity.
pub fn sparkline(ui: &mut egui::Ui, history: &[f32], height: f32) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(8), METER_BG);
    painter.line_segment(
        [
            egui::pos2(rect.left() + 8.0, rect.center().y),
            egui::pos2(rect.right() - 8.0, rect.center().y),
        ],
        Stroke::new(1.0, BORDER),
    );
    if history.len() < 2 {
        return;
    }
    let inner = rect.shrink2(egui::vec2(8.0, 6.0));
    let step = inner.width() / (history.len() - 1) as f32;
    let points: Vec<egui::Pos2> = history
        .iter()
        .enumerate()
        .map(|(index, value)| {
            egui::pos2(
                inner.left() + index as f32 * step,
                inner.bottom() - value.clamp(0.0, 1.0) * inner.height(),
            )
        })
        .collect();
    let mut area = points.clone();
    area.push(egui::pos2(inner.right(), inner.bottom()));
    area.push(egui::pos2(inner.left(), inner.bottom()));
    painter.add(egui::Shape::convex_polygon(
        area,
        Color32::from_rgba_unmultiplied(124, 92, 255, 45),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::line(points, Stroke::new(2.0, ACCENT)));
}

pub fn meter(ui: &mut egui::Ui, value: f32, height: f32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), METER_BG);

    let value = value.clamp(0.0, 1.0);
    if value > 0.002 {
        let fill =
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * value, rect.height()));
        let color = if value > 0.85 {
            RED
        } else if value > 0.6 {
            AMBER
        } else {
            GREEN
        };
        painter.rect_filled(fill, CornerRadius::same(4), color);
    }
}
