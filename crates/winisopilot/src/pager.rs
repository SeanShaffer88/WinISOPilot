//! Paging for lists, so every page fits the window without a scrollbar.
//!
//! `layout` works out how many fixed-height rows fit in the space left and
//! which ones the current page shows; `Page::controls` draws Previous/Next.
//! The mouse wheel and PageUp/PageDown also turn pages while the pointer is
//! over the list.

use eframe::egui::{self, Id, RichText, Ui};
use std::ops::Range;

/// Height of the Previous/Next bar.
pub const BAR_H: f32 = 40.0;

pub struct Page {
    pub range: Range<usize>,
    page: usize,
    pages: usize,
    total: usize,
    id: Id,
}

/// Splits `total` rows of height `row_h` into pages that fit the height
/// left in `ui` after reserving `reserve` pixels below the list.
pub fn layout(ui: &Ui, salt: impl std::hash::Hash, total: usize, row_h: f32, reserve: f32) -> Page {
    let id = Id::new(("pager", salt));
    let avail = (ui.available_height() - reserve).max(row_h);
    let per =
        if total as f32 * row_h <= avail { total.max(1) } else { (((avail - BAR_H) / row_h).floor() as usize).max(1) };
    let pages = total.div_ceil(per).max(1);
    let page = ui.data(|d| d.get_temp::<usize>(id)).unwrap_or(0).min(pages - 1);
    let start = page * per;
    Page { range: start..(start + per).min(total), page, pages, total, id }
}

impl Page {
    /// Draws the page bar (only when there is more than one page) and
    /// handles wheel / PageUp / PageDown over `area`.
    pub fn controls(&self, ui: &mut Ui, area: egui::Rect) {
        if self.pages <= 1 {
            return;
        }
        let mut page = self.page;
        let hovered = ui.input(|i| i.pointer.hover_pos().is_some_and(|p| area.contains(p)));
        if hovered {
            let now = ui.input(|i| i.time);
            let last = ui.data(|d| d.get_temp::<f64>(self.id.with("t"))).unwrap_or(0.0);
            let (wheel, down, up) = ui.input_mut(|i| {
                (
                    i.raw_scroll_delta.y,
                    i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
                )
            });
            // One page per wheel gesture: ignore further wheel events for a moment.
            let wheel_ok = wheel.abs() > 0.5 && now - last > 0.35;
            if (down || (wheel_ok && wheel < 0.0)) && page + 1 < self.pages {
                page += 1;
            } else if (up || (wheel_ok && wheel > 0.0)) && page > 0 {
                page -= 1;
            }
            if wheel_ok {
                ui.data_mut(|d| d.insert_temp(self.id.with("t"), now));
            }
        }
        ui.horizontal(|ui| {
            ui.set_min_height(BAR_H - 8.0);
            if ui
                .add_enabled(page > 0, egui::Button::new("Previous page"))
                .on_hover_text("PageUp, or scroll over the list")
                .clicked()
            {
                page -= 1;
            }
            let shown = format!(
                "Page {} of {} · {}–{} of {}",
                page + 1,
                self.pages,
                self.range.start + 1,
                self.range.end,
                self.total
            );
            ui.label(RichText::new(shown).size(12.0).color(crate::theme::TEXT_DIM));
            if ui
                .add_enabled(page + 1 < self.pages, egui::Button::new("Next page"))
                .on_hover_text("PageDown, or scroll over the list")
                .clicked()
            {
                page += 1;
            }
        });
        if page != self.page {
            ui.data_mut(|d| d.insert_temp(self.id, page));
            ui.ctx().request_repaint();
        }
    }
}
