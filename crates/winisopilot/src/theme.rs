//! Crimson & charcoal design tokens (see docs/design-tokens.md) and small
//! shared widgets.

use eframe::egui::{self, Color32, CornerRadius, FontId, Frame, Margin, Response, RichText, Stroke, Ui, Vec2, Visuals};
use wip_core::Risk;

pub const PAGE: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x22);
pub const CARD: Color32 = Color32::from_rgb(0x25, 0x25, 0x2B);
pub const FIELD: Color32 = Color32::from_rgb(0x2D, 0x2D, 0x33);
pub const RAIL: Color32 = Color32::from_rgb(0x65, 0x0F, 0x23);
pub const RAIL_SELECTED: Color32 = Color32::from_rgb(0x8A, 0x1A, 0x35);
pub const RAIL_MUTED: Color32 = Color32::from_rgb(0xE8, 0xC4, 0xCD);
pub const CHROME: Color32 = Color32::from_rgb(0x18, 0x18, 0x1C);
pub const PRIMARY: Color32 = Color32::from_rgb(0xA5, 0x22, 0x40);
pub const ACCENT: Color32 = Color32::from_rgb(0xEF, 0x78, 0x94);
pub const ROW_SELECTED: Color32 = Color32::from_rgb(0x3A, 0x15, 0x20);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF0, 0xF2);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xB9, 0xB3, 0xBC);
pub const TEXT_BODY: Color32 = Color32::from_rgb(0xD9, 0xD4, 0xDB);
pub const BOUNDARY: Color32 = Color32::from_rgb(0x85, 0x81, 0x8B);
pub const DIVIDER: Color32 = Color32::from_rgb(0x3B, 0x3B, 0x43);
pub const WARN_BG: Color32 = Color32::from_rgb(0x2F, 0x26, 0x14);
pub const WARN_BORDER: Color32 = Color32::from_rgb(0x6B, 0x55, 0x24);
pub const WARN_TEXT: Color32 = Color32::from_rgb(0xF5, 0xD3, 0x8A);
pub const WARN_BODY: Color32 = Color32::from_rgb(0xEF, 0xE3, 0xC6);
pub const OK_TEXT: Color32 = Color32::from_rgb(0x9E, 0xE6, 0xCB);
pub const ERR_TEXT: Color32 = Color32::from_rgb(0xFF, 0x9D, 0xB0);

pub fn risk_colors(r: Option<Risk>) -> (Color32, Color32) {
    match r {
        Some(Risk::Safe) => (Color32::from_rgb(0x1F, 0x3D, 0x35), OK_TEXT),
        Some(Risk::Low) => (Color32::from_rgb(0x2E, 0x2A, 0x35), Color32::from_rgb(0xD8, 0xCF, 0xE6)),
        Some(Risk::Moderate) => (Color32::from_rgb(0x4A, 0x3A, 0x16), WARN_TEXT),
        Some(Risk::High) => (Color32::from_rgb(0x4D, 0x16, 0x24), ERR_TEXT),
        None => (Color32::from_rgb(0x4D, 0x16, 0x24), ERR_TEXT),
    }
}

pub const FOCUS_HC: Color32 = Color32::from_rgb(0xFF, 0xD4, 0x00);

pub fn apply(ctx: &egui::Context, settings: &wip_core::settings::Settings) {
    let hc = settings.high_contrast;
    let mut v = Visuals::dark();
    v.panel_fill = PAGE;
    v.window_fill = CARD;
    v.window_stroke = Stroke::new(if hc { 2.0_f32 } else { 1.0 }, if hc { Color32::WHITE } else { DIVIDER });
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = CARD;
    v.hyperlink_color = if hc { FOCUS_HC } else { ACCENT };
    v.selection.bg_fill = PRIMARY;
    v.override_text_color = Some(if hc { Color32::WHITE } else { TEXT });
    // 3 px: checkboxes stay visibly square (6 px made them look like radio buttons).
    let r = CornerRadius::same(3);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = r;
    }
    let border = if hc { Stroke::new(2.0_f32, Color32::WHITE) } else { Stroke::new(1.0_f32, BOUNDARY) };
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, if hc { BOUNDARY } else { DIVIDER });
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, if hc { Color32::WHITE } else { TEXT_BODY });
    v.widgets.inactive.bg_fill = FIELD;
    v.widgets.inactive.weak_bg_fill = FIELD;
    v.widgets.inactive.bg_stroke = border;
    v.widgets.inactive.fg_stroke = Stroke::new(if hc { 2.0_f32 } else { 1.0 }, if hc { Color32::WHITE } else { TEXT });
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x38, 0x38, 0x40);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x38, 0x38, 0x40);
    // Hovered and keyboard-focused widgets share this outline.
    v.widgets.hovered.bg_stroke = Stroke::new(if hc { 3.0_f32 } else { 1.5 }, if hc { FOCUS_HC } else { ACCENT });
    v.widgets.hovered.fg_stroke = Stroke::new(if hc { 2.0_f32 } else { 1.0 }, if hc { Color32::WHITE } else { TEXT });
    v.widgets.active.bg_fill = PRIMARY;
    v.widgets.active.weak_bg_fill = PRIMARY;
    v.widgets.hovered.expansion = 0.0;
    v.selection.stroke = Stroke::new(if hc { 3.0_f32 } else { 2.0 }, if hc { FOCUS_HC } else { ACCENT });
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        // Data-dense: 13 px text, tight 6/4 px gaps, 22 px controls.
        s.spacing.item_spacing = Vec2::new(6.0, 4.0);
        s.spacing.button_padding = Vec2::new(8.0, 3.0);
        s.spacing.interact_size.y = 22.0;
        s.spacing.indent = 12.0;
        use egui::{FontFamily, TextStyle};
        s.text_styles = [
            (TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
        ]
        .into();
        s.animation_time = if settings.reduce_motion { 0.0 } else { 1.0 / 12.0 };
    });
    ctx.set_zoom_factor(settings.text_scale.clamp(0.75, 2.0));
}

pub fn eyebrow(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).font(FontId::monospace(10.5)).color(TEXT_DIM));
}

pub fn h1(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(18.0).strong().color(TEXT));
}

pub fn h2(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(14.0).strong().color(TEXT));
}

pub fn body(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).color(TEXT_BODY));
}

pub fn dim(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(TEXT_DIM));
}

pub fn badge(ui: &mut Ui, risk: Option<Risk>) -> Response {
    let (bg, fg) = risk_colors(risk);
    let label = risk.map_or("UNRATED", |r| match r {
        Risk::Safe => "SAFE",
        Risk::Low => "LOW",
        Risk::Moderate => "MODERATE",
        Risk::High => "HIGH",
    });
    Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| ui.label(RichText::new(label).font(FontId::monospace(10.0)).color(fg)))
        .response
}

pub fn primary(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let b = egui::Button::new(RichText::new(text).size(13.0).strong().color(TEXT))
        .fill(if enabled { PRIMARY } else { Color32::from_rgb(0x55, 0x2A, 0x34) })
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(72.0, 26.0));
    ui.add_enabled(enabled, b)
}

pub fn secondary(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let b = egui::Button::new(RichText::new(text).size(13.0).color(TEXT))
        .fill(Color32::TRANSPARENT)
        .stroke(Stroke::new(1.0_f32, BOUNDARY))
        .min_size(Vec2::new(60.0, 26.0));
    ui.add_enabled(enabled, b)
}

pub fn quiet(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(13.0).color(TEXT_DIM)).frame(false).min_size(Vec2::new(48.0, 26.0)),
    )
}

pub fn card() -> Frame {
    Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(1.0_f32, DIVIDER))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::same(8))
}

/// Asks the next card to stretch down to `reserve` pixels above the bottom
/// of the space left in `ui`, so pages have no empty band at the bottom.
pub fn fill_next(ui: &Ui, reserve: f32) {
    let h = (ui.available_height() - reserve - 2.0 * 8.0 - 2.0).max(0.0);
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("card-fill-next"), (pass, h)));
}

/// First call inside every card: full width, plus any pending stretch.
pub fn card_body(ui: &mut Ui) {
    ui.set_width(ui.available_width());
    let id = egui::Id::new("card-fill-next");
    // Only a request made earlier in this same frame counts.
    if let Some((pass, h)) = ui.ctx().data_mut(|d| d.remove_temp::<(u64, f32)>(id)) {
        if pass == ui.ctx().cumulative_pass_nr() {
            ui.set_min_height(h);
        }
    }
}

pub fn warn_box(ui: &mut Ui, title: &str, lines: &[String]) {
    Frame::new()
        .fill(WARN_BG)
        .stroke(Stroke::new(1.0_f32, WARN_BORDER))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(WARN_TEXT));
            for l in lines {
                ui.label(RichText::new(format!("• {l}")).color(WARN_BODY));
            }
        });
}

pub fn error_box(ui: &mut Ui, title: &str, text: &str) {
    Frame::new()
        .fill(Color32::from_rgb(0x3A, 0x15, 0x20))
        .stroke(Stroke::new(1.0_f32, PRIMARY))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().color(ERR_TEXT));
            ui.label(RichText::new(text).color(TEXT_BODY));
        });
}

/// ✔ / ✖ status line used by host checks.
pub fn check_line(ui: &mut Ui, ok: bool, text: &str) {
    ui.horizontal(|ui| {
        let (sym, c) = if ok { ("✔", OK_TEXT) } else { ("✖", ERR_TEXT) };
        ui.label(RichText::new(sym).strong().color(c));
        ui.add(egui::Label::new(RichText::new(text).color(TEXT_BODY)).wrap());
    });
}
