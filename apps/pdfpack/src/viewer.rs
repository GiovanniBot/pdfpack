//! O visualizador do PDF selecionado, com seus controles flutuantes, e o minimapa de páginas.

use egui::{Align2, Color32, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::core::human_size;
use crate::icons::Icon;
use crate::list::elided_name;
use crate::render::{Job, Key, Kind};
use crate::theme::{self, ACCENT, BORDER, CARD, GOOD, MUTED, PANEL, TEXT};
use crate::{App, ToastAction};

const PAD: f32 = 22.0;
const GAP: f32 = 14.0;
const THUMB: f32 = 104.0;
const UV: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));

/// A melhor imagem que já existe da página: a do tamanho pedido, outra ampliação, ou a miniatura.
fn best_texture(app: &App, exact: Key) -> (Option<egui::TextureId>, bool) {
    if let Some(t) = app.textures.get(&exact) {
        return (Some(t.id()), true);
    }
    let other = app.textures.iter().filter(|(k, _)| k.doc == exact.doc && k.version == exact.version && k.page == exact.page).max_by_key(|(k, _)| {
        match k.kind {
            Kind::Page(w) => w,
            Kind::Thumb => 0,
        }
    });
    (other.map(|(_, t)| t.id()), false)
}

pub fn pages(app: &mut App, ui: &mut Ui) {
    let Some(doc) = app.selected_doc().cloned() else { return };
    let ppp = ui.ctx().pixels_per_point();
    let zoom = ui.input(|i| i.zoom_delta());
    if zoom != 1.0 && ui.rect_contains_pointer(ui.max_rect()) {
        app.zoom = (app.zoom * zoom).clamp(0.3, 4.0);
    }
    // A faixa com o nome do arquivo e a página atual.
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    ui.painter().rect_filled(strip, 0, PANEL);
    ui.painter().hline(strip.x_range(), strip.max.y - 0.5, Stroke::new(1.0, BORDER));
    Icon::File.paint(ui, pos2(strip.min.x + 22.0, strip.center().y), 16.0, if doc.compressed { ACCENT } else { MUTED });
    let page = format!("Página {} de {}", app.current_page + 1, doc.pages.len());
    let page = ui.painter().text(pos2(strip.max.x - 16.0, strip.center().y), Align2::RIGHT_CENTER, page, theme::regular(12.5), MUTED);
    let size = if doc.compressed { format!("{} · −{}%", human_size(doc.size()), doc.saved_percent()) } else { human_size(doc.size()) };
    let (fg, bg) = if doc.compressed { (ACCENT, ACCENT.gamma_multiply(0.14)) } else { (MUTED, CARD) };
    let right = theme::pill(ui.painter(), pos2(page.min.x - 14.0, strip.center().y), &size, fg, bg).min.x - 10.0;
    let name = elided_name(ui, &doc.name, theme::semibold(13.5), TEXT, right - strip.min.x - 40.0);
    ui.painter().galley(pos2(strip.min.x + 38.0, strip.center().y - name.size().y / 2.0), name, TEXT);

    let avail = ui.available_rect_before_wrap();
    let widest = doc.pages.iter().map(|p| p[0]).fold(1.0, f32::max);
    // Com zoom 1 a página mais larga ocupa a largura do visualizador (até um tamanho confortável).
    let fit = (avail.width() - 2.0 * PAD - 10.0).clamp(120.0, 980.0);
    let scale = fit * app.zoom / widest;
    let mut tops = Vec::with_capacity(doc.pages.len());
    let mut y = PAD;
    for p in doc.pages.iter() {
        tops.push(y);
        y += p[1] * scale + GAP;
    }
    let total = vec2((widest * scale + 2.0 * PAD).max(avail.width() - 10.0), y - GAP + PAD);

    let mut area = egui::ScrollArea::both().id_salt(("viewer", doc.id)).auto_shrink(false);
    if let Some(page) = app.scroll_to.take() {
        area = area.vertical_scroll_offset(tops.get(page).map_or(0.0, |t| (t - PAD / 2.0).max(0.0)));
    }
    let mut visible: Vec<Job> = Vec::new();
    let mut nearby: Vec<Job> = Vec::new();
    let mut current = app.current_page;
    app.desired.clear();
    area.show_viewport(ui, |ui, viewport| {
        let (space, _) = ui.allocate_exact_size(total, Sense::hover());
        let focus = viewport.min.y + viewport.height() * 0.35;
        for (i, p) in doc.pages.iter().enumerate() {
            let (w, h) = (p[0] * scale, p[1] * scale);
            let top = tops[i];
            if top <= focus {
                current = i;
            }
            if top + h < viewport.min.y - 900.0 || top > viewport.max.y + 900.0 {
                continue;
            }
            let rect = Rect::from_min_size(pos2(space.min.x + (total.x - w) / 2.0, space.min.y + top), vec2(w, h));
            let on_screen = top + h >= viewport.min.y && top <= viewport.max.y;
            // Larguras em degraus de 32 px: redimensionar a janela não redesenha a cada pixel.
            let px = (((w * ppp) / 32.0).ceil() * 32.0).clamp(64.0, 6000.0) as u32;
            let key = Key { doc: doc.id, version: doc.version, page: i, kind: Kind::Page(px) };
            app.desired.insert(i, px);
            let (texture, exact) = best_texture(app, key);
            let painter = ui.painter();
            painter.rect_filled(rect.translate(vec2(0.0, 3.0)).expand(2.0), 3, Color32::from_black_alpha(90));
            painter.rect_filled(rect, 0, Color32::WHITE);
            if let Some(id) = texture {
                painter.image(id, rect, UV, Color32::WHITE);
            }
            if !exact && !app.failed.contains(&key) {
                let job = Job { key, bytes: doc.current.clone(), scale: px as f32 / p[0] };
                if on_screen { visible.push(job) } else { nearby.push(job) }
            }
        }
    });
    app.current_page = current.min(doc.pages.len().saturating_sub(1));
    controls(app, ui, avail);
    // As páginas na tela primeiro; as vizinhas depois das miniaturas pedidas antes.
    visible.append(&mut app.wanted);
    visible.append(&mut nearby);
    app.wanted = visible;
}

pub fn minimap(app: &mut App, ui: &mut Ui) {
    let Some(doc) = app.selected_doc().cloned() else { return };
    let ppp = ui.ctx().pixels_per_point();
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        let (slot, _) = ui.allocate_exact_size(vec2(16.0, 32.0), Sense::hover());
        Icon::Files.paint(ui, slot.center(), 15.0, MUTED);
        ui.label(RichText::new("Páginas").font(theme::semibold(14.5)));
        ui.label(RichText::new(doc.pages.len().to_string()).color(MUTED).size(12.5));
    });
    ui.add_space(2.0);
    let follow = ui.data_mut(|d| {
        let last = d.get_temp_mut_or::<(u64, usize)>(egui::Id::unique("minimap-follow"), (0, usize::MAX));
        let changed = *last != (doc.id, app.current_page);
        *last = (doc.id, app.current_page);
        changed
    });
    let mut jobs: Vec<Job> = Vec::new();
    egui::ScrollArea::vertical().id_salt(("minimap", doc.id)).auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let width = ui.available_width();
        for (i, p) in doc.pages.iter().enumerate() {
            let h = (THUMB * p[1] / p[0]).clamp(30.0, 260.0);
            let w = (h * p[0] / p[1]).min(THUMB);
            let (slot, resp) = ui.allocate_exact_size(vec2(width, h + 24.0), Sense::click());
            let current = i == app.current_page;
            if current && follow {
                ui.scroll_to_rect(slot.expand(40.0), None);
            }
            if resp.clicked() {
                app.scroll_to = Some(i);
            }
            if !ui.is_rect_visible(slot.expand(300.0)) {
                continue;
            }
            let rect = Rect::from_min_size(pos2(slot.center().x - w / 2.0, slot.min.y + 2.0), vec2(w, h));
            let key = Key { doc: doc.id, version: doc.version, page: i, kind: Kind::Thumb };
            let painter = ui.painter();
            painter.rect_filled(rect, 2, Color32::WHITE);
            match app.textures.get(&key) {
                Some(t) => {
                    painter.image(t.id(), rect, UV, Color32::WHITE);
                }
                None if !app.failed.contains(&key) => jobs.push(Job { key, bytes: doc.current.clone(), scale: w * ppp / p[0] }),
                None => {}
            }
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let stroke = if current {
                Stroke::new(2.0, ACCENT)
            } else if resp.hovered() {
                Stroke::new(1.5, MUTED)
            } else {
                Stroke::new(1.0, BORDER)
            };
            painter.rect_stroke(rect.expand(1.0), 3, stroke, StrokeKind::Outside);
            let font = if current { theme::semibold(12.0) } else { theme::regular(12.0) };
            painter.text(
                pos2(slot.center().x, rect.max.y + 11.0),
                Align2::CENTER_CENTER,
                (i + 1).to_string(),
                font,
                if current { ACCENT } else { MUTED },
            );
        }
    });
    app.wanted.append(&mut jobs);
}

/// Solta as imagens que não servem mais: de outro PDF, de outra versão ou de páginas distantes.
pub fn evict(app: &mut App) {
    let Some((id, version, count)) = app.selected_doc().map(|d| (d.id, d.version, d.pages.len())) else {
        app.textures.clear();
        app.failed.clear();
        return;
    };
    let current = app.current_page;
    // As páginas que já têm a imagem do tamanho pedido: delas, as outras ampliações podem sair.
    let ready: std::collections::HashSet<usize> = app
        .desired
        .iter()
        .filter(|(page, px)| app.textures.contains_key(&Key { doc: id, version, page: **page, kind: Kind::Page(**px) }))
        .map(|(page, _)| *page)
        .collect();
    let desired = &app.desired;
    app.textures.retain(|k, _| {
        if k.doc != id || k.version != version || k.page >= count {
            return false;
        }
        match k.kind {
            Kind::Thumb => k.page.abs_diff(current) < 200,
            Kind::Page(w) => desired.get(&k.page).is_some_and(|px| *px == w || !ready.contains(&k.page)),
        }
    });
    app.failed.retain(|k| k.doc == id && k.version == version);
}

/// Os controles que flutuam sobre o visualizador: zoom embaixo, e o aviso de "salvo" acima dele.
fn controls(app: &mut App, ui: &mut Ui, area: Rect) {
    let ctx = ui.ctx().clone();
    let shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(110) };
    let frame =
        egui::Frame::new().fill(CARD).stroke(Stroke::new(1.0, BORDER)).corner_radius(20).inner_margin(egui::Margin::symmetric(6, 4)).shadow(shadow);
    egui::Area::new(egui::Id::unique("zoom"))
        .order(egui::Order::Middle)
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(pos2(area.center().x, area.max.y - 16.0))
        .show(&ctx, |ui| {
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if theme::icon_button(ui, Icon::ZoomOut, theme::Kind::Ghost, app.zoom > 0.31).on_hover_text("Reduzir").clicked() {
                        app.zoom = (app.zoom / 1.2).max(0.3);
                    }
                    let (rect, resp) = ui.allocate_exact_size(vec2(52.0, 32.0), Sense::click());
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        format!("{:.0}%", app.zoom * 100.0),
                        theme::semibold(12.5),
                        if resp.hovered() { ACCENT } else { TEXT },
                    );
                    if resp.on_hover_text("Voltar a 100%").clicked() {
                        app.zoom = 1.0;
                    }
                    if theme::icon_button(ui, Icon::ZoomIn, theme::Kind::Ghost, app.zoom < 3.99)
                        .on_hover_text(format!("Ampliar ({})", theme::shortcut("roda do mouse")))
                        .clicked()
                    {
                        app.zoom = (app.zoom * 1.2).min(4.0);
                    }
                    let (line, _) = ui.allocate_exact_size(vec2(9.0, 32.0), Sense::hover());
                    ui.painter().vline(line.center().x, line.y_range().shrink(8.0), Stroke::new(1.0, BORDER));
                    if theme::icon_button(ui, Icon::Fit, theme::Kind::Ghost, true).on_hover_text("Ajustar à largura").clicked() {
                        app.zoom = 1.0;
                    }
                });
            });
        });

    toast(app, &ctx, pos2(area.center().x, area.max.y - 72.0));
}

/// O aviso que flutua no pé da janela ("salvo", "atualizado"), com sua ação.
pub fn toast(app: &mut App, ctx: &egui::Context, anchor: egui::Pos2) {
    let now = ctx.input(|i| i.time);
    let Some(toast) = &app.toast else { return };
    if now > toast.until {
        app.toast = None;
        return;
    }
    let shadow = egui::Shadow { offset: [0, 4], blur: 16, spread: 0, color: Color32::from_black_alpha(110) };
    let frame =
        egui::Frame::new().fill(CARD).stroke(Stroke::new(1.0, BORDER)).corner_radius(20).inner_margin(egui::Margin::symmetric(14, 6)).shadow(shadow);
    // Some aos poucos no último meio segundo.
    let alpha = ((toast.until - now) as f32 / 0.5).clamp(0.0, 1.0);
    let mut clicked = false;
    egui::Area::new(egui::Id::unique("toast")).order(egui::Order::Middle).pivot(Align2::CENTER_BOTTOM).fixed_pos(anchor).show(ctx, |ui| {
        ui.set_opacity(alpha);
        frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                let (slot, _) = ui.allocate_exact_size(vec2(18.0, 34.0), Sense::hover());
                Icon::Done.paint(ui, slot.center(), 17.0, GOOD);
                ui.label(RichText::new(&toast.text).font(theme::semibold(13.0)));
                clicked = match &toast.action {
                    ToastAction::None => false,
                    ToastAction::Folder(_) => theme::button(ui, Some(Icon::FolderOpen), "Abrir pasta", theme::Kind::Outline, true).clicked(),
                    ToastAction::Restart => theme::button(ui, Some(Icon::Refresh), "Reiniciar agora", theme::Kind::Outline, true).clicked(),
                };
            });
        });
    });
    ctx.request_repaint_after_secs(0.05);
    if clicked {
        match app.toast.take().map(|t| t.action) {
            Some(ToastAction::Folder(folder)) => {
                let _ = std::process::Command::new("explorer").arg(folder).spawn();
            }
            Some(ToastAction::Restart) => app.restart(ctx),
            _ => {}
        }
    }
}
