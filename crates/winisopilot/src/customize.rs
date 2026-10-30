//! The Customize step in its two layouts:
//! Guided (stepper rail + list + explainer pane) and
//! Advanced (icon rail + category list + dense table + inspector).

use crate::app::{App, Step, View};
use crate::pages::{footer, Nav};
use crate::theme::{self, body, dim, eyebrow, h1, h2};
use eframe::egui::{self, Color32, CornerRadius, Frame, Margin, RichText, Sense, Stroke, Ui, Vec2};
use wip_core::plan::{availability, Availability};
use wip_core::settings::Mode;
use wip_core::{Category, Level, Risk};

#[derive(Clone)]
struct Row {
    id: String,
    title: String,
    tech: String,
    risk: Option<Risk>,
    status: Availability,
    checked: bool,
}

fn rows(app: &App, view: View, guided: bool) -> (Vec<Row>, usize) {
    let q = app.search.trim().to_lowercase();
    let matches = |s: &[&str]| q.is_empty() || s.iter().any(|t| t.to_lowercase().contains(&q));
    let mut hidden_by_level = 0;
    let mut out = Vec::new();
    let Some(ctx) = app.plan_ctx() else { return (out, 0) };
    match view {
        View::Cat(c) => {
            for o in app.catalog.in_category(c) {
                let status = availability(o, &ctx);
                if matches!(status, Availability::NotForThisWindows(_)) {
                    continue;
                }
                if guided && matches!(status, Availability::NotInImage(_)) {
                    continue;
                }
                if !guided && app.hide_missing && matches!(status, Availability::NotInImage(_)) {
                    continue;
                }
                if guided && !o.visible_at(app.settings.level) {
                    hidden_by_level += 1;
                    continue;
                }
                if !guided && app.risk_filter.is_some_and(|r| r != o.risk) {
                    continue;
                }
                let tech = o.actions.first().map(|a| a.target()).unwrap_or_default();
                if !matches(&[&o.title, &o.summary, &o.id, &tech]) {
                    continue;
                }
                out.push(Row {
                    id: o.id.clone(),
                    title: o.title.clone(),
                    tech,
                    risk: Some(o.risk),
                    status,
                    checked: app.selection.contains(&o.id),
                });
            }
        }
        View::Raw => {
            for r in &app.raw_items {
                if !matches(&[&r.name]) {
                    continue;
                }
                out.push(Row {
                    id: r.id.clone(),
                    title: wip_core::raw::title(&r.id),
                    tech: r.name.clone(),
                    risk: None,
                    status: Availability::Available,
                    checked: app.selection.contains(&r.id),
                });
            }
        }
    }
    (out, hidden_by_level)
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    if app.mode() == Mode::Advanced {
        advanced(app, ctx);
    } else {
        guided(app, ctx);
    }
}

fn status_note(s: &Availability) -> Option<&str> {
    match s {
        Availability::AlreadyDone(_) => Some("Already set"),
        Availability::NotInImage(_) => Some("Not in image"),
        _ => None,
    }
}

/// Explanation of the focused option (Guided explainer / Advanced inspector).
fn explainer(app: &App, ui: &mut Ui, compact: bool) {
    eyebrow(ui, if compact { "Inspector" } else { "About this option" });
    let Some(id) = app.focused.clone() else {
        dim(ui, "Select an option to see what it does, what could stop working, and how to undo it.");
        return;
    };
    if wip_core::raw::is_raw(&id) {
        h2(ui, &wip_core::raw::title(&id));
        theme::badge(ui, None);
        body(ui, "WinISOPilot has no description or safety rating for this item. It comes straight from the image's own list.");
        theme::warn_box(
            ui,
            "Be careful",
            &["Other parts of Windows may depend on it. Search for its name before removing it.".into()],
        );
        return;
    }
    let Some(o) = app.catalog.get(&id) else { return };
    h2(ui, &o.title);
    theme::badge(ui, Some(o.risk));
    body(ui, &o.summary);
    if !o.details.is_empty() {
        body(ui, &o.details);
    }
    if !o.breaks.is_empty() {
        theme::warn_box(ui, "What could stop working", &o.breaks);
    }
    if !o.undo.is_empty() {
        dim(ui, &format!("Undo later: {}", o.undo));
    }
    if let Some(c) = app.plan_ctx() {
        if let Some(r) = availability(o, &c).reason() {
            ui.label(RichText::new(r).color(theme::WARN_TEXT));
        }
    }
    let show_tech = |ui: &mut Ui| {
        for a in &o.actions {
            ui.label(RichText::new(a.technical()).monospace().size(12.0).color(theme::TEXT_DIM));
        }
    };
    // Guided never shows package names or registry paths; Advanced always does.
    if compact {
        ui.separator();
        show_tech(ui);
    }
}

fn presets_menu(app: &mut App, ui: &mut Ui) {
    let presets = app.catalog.presets.clone();
    let label = if app.mode() == Mode::Guided { "Quick picks…" } else { "Presets…" };
    ui.menu_button(RichText::new(label).size(14.0), |ui| {
        for p in presets {
            let r = ui.add(egui::Button::new(RichText::new(&p.title).strong()).frame(false));
            let r = r.on_hover_text(&p.summary);
            ui.label(RichText::new(&p.summary).size(12.5).color(theme::TEXT_DIM));
            ui.separator();
            if r.clicked() {
                apply_preset(app, &p.id);
                ui.close();
            }
        }
    });
}

fn apply_preset(app: &mut App, id: &str) {
    let Some(p) = app.catalog.preset(id).cloned() else { return };
    let Some(ctx) = app.plan_ctx() else { return };
    let mut sel = app.selection.clone();
    let mut added = 0;
    let mut missing = 0;
    for oid in &p.options {
        let Some(o) = app.catalog.get(oid) else { continue };
        if availability(o, &ctx).is_available() {
            if sel.insert(oid.clone()) {
                added += 1;
            }
        } else {
            missing += 1;
        }
    }
    app.replace_selection(sel);
    let tail = if missing > 0 { format!(" {missing} don't apply to this image.") } else { String::new() };
    app.notify(format!("{}: added {added} changes.{tail} Ctrl+Z undoes.", p.title));
}

fn search_box(app: &mut App, ui: &mut Ui, width: f32) {
    let r = ui.add(
        egui::TextEdit::singleline(&mut app.search)
            .hint_text("Search (Ctrl+F)")
            .desired_width(width)
            .margin(Margin::symmetric(8, 6)),
    );
    if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
        r.request_focus();
    }
    if !app.search.is_empty() && ui.small_button("✖").on_hover_text("Clear search").clicked() {
        app.search.clear();
    }
}

// ------------------------------------------------------------------ Guided

fn guided(app: &mut App, ctx: &egui::Context) {
    let cats = app.visible_categories();
    let View::Cat(cat) = app.view else {
        app.view = View::Cat(cats.first().copied().unwrap_or(Category::Apps));
        return;
    };
    let pos = cats.iter().position(|c| *c == cat).unwrap_or(0);
    let next_label = match cats.get(pos + 1) {
        Some(n) => format!("Next: {}", n.label()),
        None => "Review".into(),
    };
    let status = format!("{} changes selected", app.selection.len());
    match footer(ctx, &status, true, Some((&next_label, !app.busy()))) {
        Nav::Back => match pos.checked_sub(1).and_then(|p| cats.get(p)) {
            Some(prev) => {
                app.view = View::Cat(*prev);
                app.focused = None;
            }
            None => app.goto(Step::Edition),
        },
        Nav::Next => match cats.get(pos + 1) {
            Some(n) => {
                app.view = View::Cat(*n);
                app.focused = None;
                app.search.clear();
            }
            None => app.goto(Step::Review),
        },
        Nav::None => {}
    }

    egui::SidePanel::right("explainer")
        .exact_width(320.0)
        .resizable(false)
        .frame(
            Frame::new().fill(theme::CARD).inner_margin(Margin::same(16)).stroke(Stroke::new(1.0_f32, theme::DIVIDER)),
        )
        .show(ctx, |ui| {
            explainer(app, ui, false);
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::PAGE).inner_margin(Margin { left: 20, right: 16, top: 14, bottom: 10 }))
        .show(ctx, |ui| {
            eyebrow(ui, "Step 4 of 6 · Customize");
            h1(ui, cat.label());
            body(ui, cat.blurb());
            crate::help::tip(app, ui);
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                presets_menu(app, ui);
                search_box(app, ui, 200.0);
                let n = app.selected_in(cat);
                if ui
                    .add_enabled(n > 0, egui::Button::new("Untick all here"))
                    .on_hover_text("Untick every option on this page (Ctrl+Z undoes)")
                    .clicked()
                {
                    let keep: wip_core::plan::Selection = app
                        .selection
                        .iter()
                        .filter(|id| app.catalog.get(id).is_none_or(|o| o.category != cat))
                        .cloned()
                        .collect();
                    app.replace_selection(keep);
                    app.notify(format!("Cleared {n} changes in {}. Ctrl+Z undoes.", cat.label()));
                }
            });
            ui.add_space(6.0);
            let (list, hidden) = rows(app, View::Cat(cat), true);
            if list.is_empty() {
                dim(ui, "Nothing here matches this image or your search.");
            }
            let reserve = if hidden > 0 { 36.0 } else { 0.0 };
            let page = crate::pager::layout(ui, ("guided", cat.label()), list.len(), GUIDED_ROW, reserve);
            let shown = &list[page.range.clone()];
            let area = Frame::new()
                .stroke(Stroke::new(1.0_f32, theme::DIVIDER))
                .corner_radius(CornerRadius::same(10))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, r) in shown.iter().enumerate() {
                        guided_row(app, ui, r, i + 1 == shown.len());
                    }
                })
                .response
                .rect;
            ui.add_space(4.0);
            page.controls(ui, area);
            if hidden > 0 {
                ui.horizontal(|ui| {
                    dim(ui, &format!("{hidden} riskier options are hidden to keep things safe."));
                    if ui.link("Show them").on_hover_text("Also changeable in Settings").clicked() {
                        app.settings.level = match app.settings.level {
                            Level::Beginner => Level::Intermediate,
                            _ => Level::Expert,
                        };
                        app.save_settings();
                    }
                });
            }
        });
}

/// Height of one Guided option row (frame margins included).
const GUIDED_ROW: f32 = 34.0;

fn guided_row(app: &mut App, ui: &mut Ui, r: &Row, last: bool) {
    let focused = app.focused.as_deref() == Some(r.id.as_str());
    let enabled = r.status.is_available() && !app.busy();
    let fill = if focused { theme::ROW_SELECTED } else { theme::CARD };
    let resp = Frame::new()
        .fill(fill)
        .inner_margin(Margin::symmetric(12, 3))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(GUIDED_ROW - 6.0);
            ui.horizontal_centered(|ui| {
                let mut on = r.checked;
                let cb = ui.add_enabled(enabled, egui::Checkbox::without_text(&mut on));
                let cb = cb.on_hover_text(&r.title);
                if cb.changed() {
                    app.set_selected(&r.id, on);
                    app.focused = Some(r.id.clone());
                }
                let title =
                    RichText::new(&r.title).size(15.0).color(if enabled { theme::TEXT } else { theme::TEXT_DIM });
                let t = ui.add(egui::Label::new(if focused { title.strong() } else { title }).sense(Sense::click()));
                if t.clicked() {
                    app.focused = Some(r.id.clone());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    theme::badge(ui, r.risk);
                    if let Some(n) = status_note(&r.status) {
                        dim(ui, n);
                    }
                });
            });
        })
        .response;
    if focused {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(resp.rect.min, Vec2::new(3.0, resp.rect.height())),
            CornerRadius::ZERO,
            theme::ACCENT,
        );
    }
    if !last {
        ui.painter().hline(resp.rect.x_range(), resp.rect.bottom(), Stroke::new(1.0_f32, theme::DIVIDER));
    }
}

// ---------------------------------------------------------------- Advanced

fn advanced(app: &mut App, ctx: &egui::Context) {
    let total = app.selection.len();
    let (list, _) = rows(app, app.view, false);
    let shown_selected = list.iter().filter(|r| r.checked).count();
    let status = format!("{shown_selected} of {} selected here · {total} changes total", list.len());
    match footer(ctx, &status, true, Some(("Review", !app.busy()))) {
        Nav::Back => app.goto(Step::Edition),
        Nav::Next => app.goto(Step::Review),
        Nav::None => {}
    }

    egui::SidePanel::left("categories")
        .exact_width(210.0)
        .resizable(false)
        .frame(
            Frame::new()
                .fill(theme::CHROME)
                .inner_margin(Margin::symmetric(10, 16))
                .stroke(Stroke::new(1.0_f32, theme::DIVIDER)),
        )
        .show(ctx, |ui| {
            eyebrow(ui, "Image contents");
            ui.add_space(4.0);
            let cats = app.visible_categories();
            let mut views: Vec<(View, String, usize)> = cats
                .iter()
                .map(|c| {
                    let n = app.plan_ctx().map_or(0, |cx| {
                        app.catalog.in_category(*c).filter(|o| availability(o, &cx).is_available()).count()
                    });
                    (View::Cat(*c), c.label().to_string(), n)
                })
                .collect();
            views.push((View::Raw, "Unrated items".into(), app.raw_items.len()));
            for (v, label, n) in views {
                let sel = app.view == v;
                let b = egui::Button::new(RichText::new(label).size(13.5).color(if sel {
                    theme::TEXT
                } else {
                    theme::TEXT_BODY
                }))
                .fill(if sel { theme::ROW_SELECTED } else { Color32::TRANSPARENT })
                .stroke(Stroke::NONE)
                .right_text(RichText::new(n.to_string()).color(theme::TEXT_DIM))
                .min_size(Vec2::new(ui.available_width(), 32.0));
                if ui.add(b).clicked() {
                    app.view = v;
                    app.focused = None;
                }
            }
        });

    egui::SidePanel::right("inspector")
        .exact_width(290.0)
        .resizable(false)
        .frame(
            Frame::new().fill(theme::CARD).inner_margin(Margin::same(18)).stroke(Stroke::new(1.0_f32, theme::DIVIDER)),
        )
        .show(ctx, |ui| {
            explainer(app, ui, true);
        });

    egui::CentralPanel::default()
        .frame(Frame::new().fill(theme::PAGE).inner_margin(Margin::same(0)))
        .show(ctx, |ui| {
            Frame::new().inner_margin(Margin::symmetric(14, 10)).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    search_box(app, ui, 240.0);
                    let label = app.risk_filter.map_or("Risk: all".to_string(), |r| format!("Risk: {}", r.label()));
                    egui::ComboBox::from_id_salt("risk").selected_text(label).height(400.0).show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.risk_filter, None, "All");
                        for r in [Risk::Safe, Risk::Low, Risk::Moderate, Risk::High] {
                            ui.selectable_value(&mut app.risk_filter, Some(r), r.label());
                        }
                    });
                    ui.checkbox(&mut app.hide_missing, "Hide items not in image");
                    presets_menu(app, ui);
                    if ui.button("Load…").clicked() {
                        crate::pages::load_recipe(app);
                    }
                    if ui.add_enabled(app.history.can_undo(), egui::Button::new("Undo")).on_hover_text("Ctrl+Z").clicked() {
                        app.undo();
                    }
                });
            });
            ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0_f32, theme::DIVIDER));
            if app.settings.show_tips {
                Frame::new().inner_margin(Margin::symmetric(14, 0)).show(ui, |ui| crate::help::tip(app, ui));
            }
            if app.view == View::Raw {
                Frame::new().inner_margin(Margin::symmetric(14, 6)).show(ui, |ui| {
                    theme::warn_box(ui, "Unrated items", &["These come straight from the image and have no description or safety rating. Each one needs confirmation on Review.".into()]);
                });
            }
            let widths = Columns::new(ui.available_width() - 28.0);
            Frame::new().inner_margin(Margin::symmetric(14, 4)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    for (h, w) in [("", widths.check), ("NAME", widths.name), ("ID", widths.id), ("RISK", widths.risk), ("STATUS", widths.status)] {
                        cell(ui, w, |ui| ui.label(RichText::new(h).monospace().size(11.0).color(theme::TEXT_DIM)));
                    }
                });
            });
            Frame::new().inner_margin(Margin { left: 14, right: 14, top: 0, bottom: 8 }).show(ui, |ui| {
                let page = crate::pager::layout(ui, ("table", format!("{:?}", app.view)), list.len(), TABLE_ROW, 0.0);
                let area = ui
                    .scope(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, r) in list[page.range.clone()].iter().enumerate() {
                            table_row(app, ui, r, &widths, i % 2 == 1);
                        }
                    })
                    .response
                    .rect;
                if list.is_empty() {
                    dim(ui, "Nothing matches.");
                }
                ui.add_space(4.0);
                page.controls(ui, area);
            });
        });
}

/// Height of one Advanced table row.
const TABLE_ROW: f32 = 28.0;

/// A fixed-width, left-aligned table cell.
fn cell<R>(ui: &mut Ui, w: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(Vec2::new(w, TABLE_ROW), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_width(w);
        add(ui)
    })
    .inner
}

struct Columns {
    check: f32,
    name: f32,
    id: f32,
    risk: f32,
    status: f32,
}

impl Columns {
    fn new(total: f32) -> Columns {
        let fixed = 28.0 + 84.0 + 92.0 + 4.0 * 10.0;
        let flex = (total - fixed).max(200.0);
        Columns { check: 28.0, name: flex * 0.48, id: flex * 0.52, risk: 84.0, status: 92.0 }
    }
}

fn table_row(app: &mut App, ui: &mut Ui, r: &Row, w: &Columns, stripe: bool) {
    let enabled = r.status.is_available() && !app.busy();
    let focused = app.focused.as_deref() == Some(r.id.as_str());
    let fill = if focused {
        theme::ROW_SELECTED
    } else if stripe {
        Color32::from_rgb(0x23, 0x23, 0x28)
    } else {
        Color32::TRANSPARENT
    };
    Frame::new().fill(fill).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let mut on = r.checked;
            let cb = cell(ui, w.check, |ui| {
                ui.add_enabled(enabled, egui::Checkbox::without_text(&mut on)).on_hover_text(&r.title)
            });
            if cb.changed() {
                app.set_selected(&r.id, on);
                app.focused = Some(r.id.clone());
            }
            let name = RichText::new(&r.title).size(13.5).color(if enabled { theme::TEXT } else { theme::TEXT_DIM });
            let name = if focused { name.strong() } else { name };
            let l = cell(ui, w.name, |ui| ui.add(egui::Label::new(name).sense(Sense::click()).truncate()));
            if l.on_hover_text(&r.title).clicked() {
                app.focused = Some(r.id.clone());
            }
            cell(ui, w.id, |ui| {
                ui.add(
                    egui::Label::new(RichText::new(&r.tech).monospace().size(11.5).color(theme::TEXT_DIM)).truncate(),
                )
            })
            .on_hover_text(&r.tech);
            let text = r.risk.map_or("Unrated", |x| x.label());
            cell(ui, w.risk, |ui| ui.label(RichText::new(text).strong().color(theme::risk_colors(r.risk).1)));
            cell(ui, w.status, |ui| {
                ui.label(RichText::new(status_note(&r.status).unwrap_or("")).size(12.5).color(theme::TEXT_DIM))
            });
        });
    });
}
