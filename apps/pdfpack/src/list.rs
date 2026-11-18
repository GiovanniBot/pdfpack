//! A barra de ações e a listagem dos PDFs (com os cartões "Email N" quando organizada).

use egui::{Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::core::human_size;
use crate::icons::Icon;
use crate::state::Sort;
use crate::theme::{self, ACCENT, BG, BORDER, CARD, CARD_HOVER, DANGER, Kind, MUTED, ON_ACCENT, PANEL, TEXT};
use crate::{App, Modal};

const ROW: f32 = 48.0;
const HEADER: f32 = 50.0;
const INSET: f32 = 6.0;

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn toolbar(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let has_docs = !app.state.docs.is_empty();
    let compressed = app.state.any_compressed();
    let organized = app.state.organized.is_some();
    let kind = |next: bool| if next { Kind::Primary } else { Kind::Outline };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        // Três ações independentes: quem só quer comprimir não precisa organizar. O amarelo cheio
        // vai de Comprimir para Salvar assim que há o que salvar; organizar fica sempre opcional.
        if theme::button(ui, Some(if compressed { Icon::Done } else { Icon::Compress }), "Comprimir", kind(!compressed && !organized), has_docs)
            .on_hover_text("Reduz o tamanho dos PDFs marcados")
            .clicked()
        {
            app.modal = Modal::CompressSetup;
        }
        if theme::button(ui, Some(if organized { Icon::Done } else { Icon::Mail }), "Organizar para email", Kind::Outline, has_docs)
            .on_hover_text("Distribui os PDFs em pastas que cabem em um email")
            .clicked()
        {
            app.limit_text = app.prefs.email_limit_mb;
            app.modal = Modal::OrganizeSetup;
        }
        if theme::button(ui, Some(Icon::Save), "Salvar", kind(compressed || organized), has_docs)
            .on_hover_text(format!("Salva tudo em uma pasta ({})", theme::shortcut("S")))
            .clicked()
        {
            app.start_save(&ctx);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let about = theme::icon_button(ui, Icon::Info, Kind::Ghost, true).on_hover_text(format!("PDFPack {}", crate::update::VERSION));
            egui::Popup::menu(&about).show(|ui| {
                ui.set_width(250.0);
                ui.label(RichText::new(format!("PDFPack {}", crate::update::VERSION)).font(theme::semibold(14.0)));
                ui.label(RichText::new("Feito sobre o motor do PDFCraft").color(MUTED).size(12.0));
                // Atualização e instalação próprias só existem no Windows.
                if !cfg!(windows) {
                    return;
                }
                ui.add_space(4.0);
                let auto = app.prefs.auto_update == Some(true);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 7, CARD_HOVER);
                }
                checkbox(ui, Rect::from_center_size(pos2(rect.min.x + 14.0, rect.center().y), vec2(16.0, 16.0)), auto, resp.hovered(), 1.0);
                ui.painter().text(pos2(rect.min.x + 30.0, rect.center().y), Align2::LEFT_CENTER, "Atualizar sozinho", theme::regular(13.5), TEXT);
                if resp.clicked() {
                    app.prefs.auto_update = Some(!auto);
                    app.prefs.save();
                }
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 7, CARD_HOVER);
                }
                Icon::Refresh.paint(ui, pos2(rect.min.x + 14.0, rect.center().y), 15.0, MUTED);
                ui.painter().text(pos2(rect.min.x + 30.0, rect.center().y), Align2::LEFT_CENTER, "Procurar atualizações", theme::regular(13.5), TEXT);
                if resp.clicked() {
                    app.check_updates(&ctx);
                    ui.close();
                }
                // Instalado, oferece desinstalar; rodando do arquivo baixado, oferece instalar.
                let installed = crate::install::is_installed();
                let (icon, text) = if installed { (Icon::Trash, "Desinstalar o PDFPack") } else { (Icon::Monitor, "Instalar neste computador") };
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 7, CARD_HOVER);
                }
                icon.paint(ui, pos2(rect.min.x + 14.0, rect.center().y), 15.0, MUTED);
                ui.painter().text(pos2(rect.min.x + 30.0, rect.center().y), Align2::LEFT_CENTER, text, theme::regular(13.5), TEXT);
                if resp.clicked() {
                    app.modal = if installed { Modal::UninstallConfirm } else { Modal::InstallOffer { desktop: true } };
                    ui.close();
                }
            });
            if theme::icon_button(ui, Icon::Redo, Kind::Ghost, app.can_redo())
                .on_hover_text(format!("Refazer ({})", theme::shortcut("Shift+Z")))
                .clicked()
            {
                app.redo();
            }
            if theme::icon_button(ui, Icon::Undo, Kind::Ghost, app.can_undo()).on_hover_text(format!("Desfazer ({})", theme::shortcut("Z"))).clicked()
            {
                app.undo();
            }
        });
    });
}

/// O contorno de um retângulo arredondado, para a borda tracejada da área de soltar.
fn rounded_outline(rect: Rect, r: f32) -> Vec<Pos2> {
    let corners = [
        (rect.right_top() + vec2(-r, r), -90.0_f32),
        (rect.right_bottom() + vec2(-r, -r), 0.0),
        (rect.left_bottom() + vec2(r, -r), 90.0),
        (rect.left_top() + vec2(r, r), 180.0),
    ];
    let mut points: Vec<Pos2> =
        corners.iter().flat_map(|(c, start)| (0..=8).map(move |i| *c + r * egui::Vec2::angled((start + i as f32 * 11.25).to_radians()))).collect();
    points.push(points[0]);
    points
}

pub fn empty(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let rect = ui.available_rect_before_wrap();
    let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
    let glow = ctx.animate_bool_with_time(egui::Id::unique("drop-glow"), hovering, 0.15);
    let zone = Rect::from_center_size(rect.center(), vec2(540.0_f32.min(rect.width() - 40.0), 348.0));
    ui.painter().rect_filled(zone, 20, theme::mix(PANEL, ACCENT.gamma_multiply(0.10), glow));
    ui.painter().extend(Shape::dashed_line(
        &rounded_outline(zone, 20.0),
        Stroke::new(1.5, theme::mix(BORDER.gamma_multiply(1.5), ACCENT, glow)),
        7.0,
        6.0,
    ));

    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(zone.shrink(30.0)).layout(Layout::top_down(Align::Center)));
    child.add_space(14.0);
    let (slot, _) = child.allocate_exact_size(vec2(84.0, 84.0), Sense::hover());
    let logo = theme::logo(&ctx);
    child.painter().image(logo.id(), slot, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
    child.add_space(10.0);
    child.label(RichText::new(if hovering { "Solte para adicionar" } else { "Comprima e organize PDFs para email" }).font(theme::semibold(19.0)));
    child.label(RichText::new("Arraste os PDFs para cá ou selecione os arquivos.").color(MUTED).size(13.5));
    child.add_space(14.0);
    if app.loading > 0 {
        child.spinner();
        child.label(RichText::new("Abrindo PDFs…").color(MUTED));
    } else if theme::button(&mut child, Some(Icon::Plus), "Selecionar PDFs", Kind::Primary, true).clicked() {
        app.pick_files(&ctx);
    }

    crate::viewer::toast(app, &ctx, pos2(rect.center().x, rect.max.y - 24.0));

    // O que o aplicativo faz, para quem o abre pela primeira vez; ações soltas, não uma sequência.
    let steps = [(Icon::Compress, "Comprimir"), (Icon::Mail, "Organizar para email"), (Icon::Save, "Salvar")];
    let font = theme::regular(12.5);
    let widths: Vec<f32> = steps.iter().map(|(_, t)| ui.painter().layout_no_wrap((*t).to_owned(), font.clone(), MUTED).size().x + 22.0).collect();
    let total: f32 = widths.iter().sum::<f32>() + 34.0 * (steps.len() - 1) as f32;
    let mut x = zone.center().x - total / 2.0;
    let y = zone.max.y - 30.0;
    for (i, ((icon, text), w)) in steps.iter().zip(&widths).enumerate() {
        icon.paint(ui, pos2(x + 7.0, y), 14.0, MUTED);
        ui.painter().text(pos2(x + 22.0, y), Align2::LEFT_CENTER, *text, font.clone(), MUTED);
        x += w;
        if i + 1 < steps.len() {
            ui.painter().circle_filled(pos2(x + 17.0, y), 1.5, BORDER.gamma_multiply(1.8));
            x += 34.0;
        }
    }
}

enum Action {
    Select(u64),
    Check(u64),
    CheckAll(bool),
    Remove(u64),
    Move(u64, usize),
    Fold(usize),
    Sort(Sort),
    Add,
}

/// Um texto de uma linha só, cortado com reticências quando não cabe.
pub fn elided(ui: &Ui, text: &str, font: egui::FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping { max_width: width.max(10.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    ui.painter().layout_job(job)
}

/// Um nome de arquivo cortado no meio quando não cabe: o fim ("(parte 2 de 3).pdf") continua à vista.
pub fn elided_name(ui: &Ui, text: &str, font: egui::FontId, color: Color32, width: f32) -> std::sync::Arc<egui::Galley> {
    const TAIL: usize = 18;
    let fits = |t: &str| ui.painter().layout_no_wrap(t.to_owned(), font.clone(), color);
    let whole = fits(text);
    let chars: Vec<char> = text.chars().collect();
    if whole.size().x <= width || chars.len() <= TAIL + 6 {
        return if whole.size().x <= width { whole } else { elided(ui, text, font, color, width) };
    }
    let tail: String = chars[chars.len() - TAIL..].iter().collect();
    let (mut lo, mut hi) = (1, chars.len() - TAIL);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        let head: String = chars[..mid].iter().collect();
        if fits(&format!("{}…{tail}", head.trim_end())).size().x <= width { lo = mid } else { hi = mid - 1 }
    }
    let head: String = chars[..lo].iter().collect();
    elided(ui, &format!("{}…{tail}", head.trim_end()), font, color, width)
}

pub(crate) fn checkbox(ui: &Ui, rect: Rect, checked: bool, hovered: bool, alpha: f32) {
    let p = ui.painter();
    if checked {
        p.rect_filled(rect, 5, ACCENT.gamma_multiply(alpha));
        Icon::Check.paint(ui, rect.center(), 13.0, ON_ACCENT.gamma_multiply(alpha));
    } else {
        p.rect_stroke(rect, 5, Stroke::new(1.4, if hovered { TEXT } else { MUTED.gamma_multiply(0.8) }.gamma_multiply(alpha)), StrokeKind::Inside);
    }
}

fn header(app: &App, ui: &mut Ui, actions: &mut Vec<Action>) {
    let n = app.state.docs.len();
    let checked = app.state.docs.iter().filter(|d| d.checked).count();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        ui.add_space(INSET + 3.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(17.0, 32.0), Sense::click());
        checkbox(ui, Rect::from_center_size(rect.center(), vec2(17.0, 17.0)), checked == n, resp.hovered(), 1.0);
        if resp.on_hover_text(if checked == n { "Desmarcar todos" } else { "Marcar todos" }).clicked() {
            actions.push(Action::CheckAll(checked != n));
        }
        ui.label(RichText::new("PDFs").font(theme::semibold(14.5)));
        let label = if checked == n { n.to_string() } else { format!("{checked} de {n}") };
        let (slot, _) = ui.allocate_exact_size(
            vec2(ui.painter().layout_no_wrap(label.clone(), theme::semibold(11.5), MUTED).size().x + 14.0, 20.0),
            Sense::hover(),
        );
        theme::pill(ui.painter(), slot.right_center(), &label, MUTED, CARD);
        if app.loading > 0 {
            ui.spinner();
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if theme::icon_button(ui, Icon::Plus, Kind::Ghost, true).on_hover_text(format!("Adicionar PDFs ({})", theme::shortcut("O"))).clicked() {
                actions.push(Action::Add);
            }
            let sort = theme::icon_button(ui, Icon::Sort, Kind::Ghost, true).on_hover_text(format!("Ordenar · {}", app.state.sort.name()));
            egui::Popup::menu(&sort).show(|ui| {
                // Largura fixa: as linhas ocupam a largura disponível, que num menu é a da janela.
                ui.set_width(190.0);
                ui.label(RichText::new("Ordenar por").color(MUTED).size(12.0));
                for (sort, icon) in Sort::MENU.into_iter().zip([Icon::Hash, Icon::Type, Icon::Gauge, Icon::Clock]) {
                    let on = app.state.sort == sort;
                    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 7, CARD_HOVER);
                    }
                    icon.paint(ui, pos2(rect.min.x + 14.0, rect.center().y), 15.0, if on { ACCENT } else { MUTED });
                    ui.painter().text(
                        pos2(rect.min.x + 30.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        sort.name(),
                        theme::regular(13.5),
                        if on { ACCENT } else { TEXT },
                    );
                    if on {
                        Icon::Check.paint(ui, pos2(rect.max.x - 12.0, rect.center().y), 14.0, ACCENT);
                    }
                    if resp.clicked() {
                        actions.push(Action::Sort(sort));
                        ui.close();
                    }
                }
            });
        });
    });
}

/// O resumo no pé da lista: quanto pesa tudo e quanto já foi economizado.
fn summary(app: &App, ui: &mut Ui) {
    let before: u64 = app.state.docs.iter().map(|d| d.base_size).sum();
    let total = app.state.total();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::hover());
    let p = ui.painter();
    p.rect(rect, 12, BG, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    p.text(pos2(rect.min.x + 14.0, rect.min.y + 17.0), Align2::LEFT_CENTER, "Total", theme::regular(11.5), MUTED);
    let size = p.text(pos2(rect.min.x + 14.0, rect.min.y + 36.0), Align2::LEFT_CENTER, human_size(total), theme::semibold(16.0), TEXT);
    if total < before {
        p.text(pos2(size.max.x + 8.0, rect.min.y + 37.0), Align2::LEFT_CENTER, format!("antes {}", human_size(before)), theme::regular(11.5), MUTED);
        let percent = (100.0 * (1.0 - total as f64 / before.max(1) as f64)).round();
        theme::pill(p, pos2(rect.max.x - 12.0, rect.min.y + 17.0), &format!("−{percent}%"), ACCENT, ACCENT.gamma_multiply(0.14));
    }
    if let Some(emails) = app.state.emails() {
        let text = plural(emails.len(), "email", "emails");
        let t = p.text(pos2(rect.max.x - 14.0, rect.min.y + 37.0), Align2::RIGHT_CENTER, text, theme::semibold(12.5), TEXT);
        Icon::Mail.paint(ui, pos2(t.min.x - 12.0, rect.min.y + 37.0), 14.0, ACCENT);
    }
}

pub fn panel(app: &mut App, ui: &mut Ui) {
    let mut actions: Vec<Action> = Vec::new();
    header(app, ui, &mut actions);
    ui.add_space(2.0);
    egui::Panel::bottom("summary")
        .frame(egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 8, bottom: 0 }))
        .show_separator_line(false)
        .resizable(false)
        .show(ui, |ui| summary(app, ui));

    let n = app.state.docs.len();
    let now = ui.input(|i| i.time);
    let limit = app.state.organized;
    let emails = app.state.emails();
    let groups: Vec<(Option<usize>, Vec<usize>)> = match &emails {
        Some(emails) => emails.iter().enumerate().map(|(i, docs)| (Some(i), docs.clone())).collect(),
        None => vec![(None, (0..n).collect())],
    };
    // A animação de entrada: cada cartão de email aparece e recebe seus PDFs, um depois do outro.
    let organized_at = app.organized_at;
    let enter = |order: usize| -> f32 {
        let Some(start) = organized_at else { return 1.0 };
        let t = (((now - start) as f32 - order as f32 * 0.06) / 0.34).clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    };
    let mut animating = false;
    // Onde cada linha ficou neste quadro, para saber onde soltar um PDF arrastado.
    let mut rows: Vec<(usize, Rect)> = Vec::new();

    egui::ScrollArea::vertical().id_salt("docs").auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        let left = ui.cursor().min.x;
        let mut order = 0;
        for (email, docs) in &groups {
            let mut open = 1.0;
            let mut appear = 1.0;
            let card = ui.painter().add(Shape::Noop);
            let top = ui.cursor().min.y;
            let grouped = email.is_some();
            if let (Some(e), Some(limit)) = (email, limit) {
                appear = enter(order);
                order += 1;
                animating |= appear < 1.0;
                let folded = app.collapsed.contains(e);
                open = ui.ctx().animate_bool_with_time(ui.scope_id().with(("fold", *e)), !folded, 0.16);
                let size: u64 = docs.iter().map(|i| app.state.docs[*i].size()).sum();
                let over = size > limit;
                let (rect, resp) = ui.allocate_exact_size(vec2(width, HEADER * appear), Sense::click());
                if appear > 0.05 {
                    let p = ui.painter().with_clip_rect(rect);
                    let a = |c: Color32| c.gamma_multiply(appear);
                    let mid = rect.min.y + HEADER / 2.0;
                    theme::chevron(&p, pos2(rect.min.x + 14.0, mid), open, a(if resp.hovered() { TEXT } else { MUTED }));
                    p.rect_filled(
                        Rect::from_center_size(pos2(rect.min.x + 40.0, mid), vec2(28.0, 28.0)),
                        8,
                        a(if over { DANGER } else { ACCENT }).gamma_multiply(0.16),
                    );
                    if appear > 0.9 {
                        Icon::Mail.paint(ui, pos2(rect.min.x + 40.0, mid), 15.0, if over { DANGER } else { ACCENT });
                    }
                    let title =
                        p.text(pos2(rect.min.x + 62.0, mid - 8.0), Align2::LEFT_CENTER, format!("Email {}", e + 1), theme::semibold(14.0), a(TEXT));
                    p.text(
                        pos2(title.max.x + 7.0, mid - 7.5),
                        Align2::LEFT_CENTER,
                        plural(docs.len(), "PDF", "PDFs"),
                        theme::regular(11.5),
                        a(MUTED),
                    );
                    let total = format!("{} de {}", human_size(size), human_size(limit));
                    p.text(
                        pos2(rect.max.x - 12.0, mid - 8.0),
                        Align2::RIGHT_CENTER,
                        total,
                        theme::regular(11.5),
                        a(if over { DANGER } else { MUTED }),
                    );
                    // Quanto do limite o email ocupa.
                    let bar = Rect::from_min_max(pos2(rect.min.x + 62.0, mid + 7.0), pos2(rect.max.x - 12.0, mid + 11.0));
                    p.rect_filled(bar, 2, a(BORDER));
                    let fill = (size as f32 / limit.max(1) as f32).clamp(0.02, 1.0) * appear;
                    p.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * fill, bar.height())), 2, a(if over { DANGER } else { ACCENT }));
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp
                    .on_hover_text(if over { "Passa do limite: uma única página é maior que o email" } else { "Clique para abrir ou fechar" })
                    .clicked()
                {
                    actions.push(Action::Fold(*e));
                }
            }
            let inset = if grouped { INSET } else { 0.0 };
            for &i in docs {
                if open <= 0.01 {
                    break;
                }
                let doc = &app.state.docs[i];
                let t = if grouped { enter(order) } else { 1.0 };
                if grouped {
                    order += 1;
                    animating |= t < 1.0;
                }
                let step = (ROW + 5.0) * open;
                let (slot, _) = ui.allocate_exact_size(vec2(width, step), Sense::hover());
                // Na entrada, a linha desliza para dentro do cartão.
                let rect = Rect::from_min_size(slot.min + vec2(inset - (1.0 - t) * 18.0, 0.0), vec2(width - 2.0 * inset, ROW));
                rows.push((i, rect));
                if !ui.is_rect_visible(slot) {
                    continue;
                }
                let whole = open > 0.99 && t > 0.99;
                let clip = slot.expand2(vec2(0.0, 0.5));
                let resp = ui.interact(rect.intersect(slot), ui.scope_id().with(("row", doc.id)), Sense::click_and_drag());
                let selected = app.selected == Some(doc.id);
                let dragging = app.drag == Some(doc.id);
                let alpha = if dragging { 0.3 } else { t };
                let fade = |c: Color32| c.gamma_multiply(alpha);
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered() || selected, 0.08);
                let p = ui.painter().with_clip_rect(clip);
                p.rect_filled(rect, 10, fade(theme::mix(if grouped { CARD } else { CARD.gamma_multiply(0.75) }, CARD_HOVER, hover)));
                // Comprimidos ficam com a borda amarela, mais forte no selecionado.
                if doc.compressed {
                    p.rect_stroke(rect, 10, Stroke::new(1.2, fade(ACCENT).gamma_multiply(if selected { 1.0 } else { 0.55 })), StrokeKind::Inside);
                } else if selected {
                    p.rect_stroke(rect, 10, Stroke::new(1.0, fade(MUTED).gamma_multiply(0.7)), StrokeKind::Inside);
                }
                if selected {
                    p.rect_filled(
                        Rect::from_min_max(pos2(rect.min.x, rect.min.y + 13.0), pos2(rect.min.x + 3.0, rect.max.y - 13.0)),
                        CornerRadius { nw: 0, sw: 0, ne: 3, se: 3 },
                        fade(ACCENT),
                    );
                }

                let check = Rect::from_center_size(pos2(rect.min.x + 20.0, rect.center().y), vec2(17.0, 17.0));
                if whole {
                    let check_resp = ui.interact(check.expand(6.0), ui.scope_id().with(("check", doc.id)), Sense::click());
                    checkbox(ui, check, doc.checked, check_resp.hovered(), alpha);
                    if check_resp.clicked() {
                        actions.push(Action::Check(doc.id));
                    }
                }

                // À direita: a economia; com o ponteiro em cima, o botão de remover no lugar dela.
                let hovered = whole && ui.rect_contains_pointer(rect) && app.drag.is_none();
                let mut right = rect.max.x - 34.0;
                if doc.compressed && !hovered {
                    let badge = theme::pill(
                        &p,
                        pos2(rect.max.x - 10.0, rect.center().y),
                        &format!("−{}%", doc.saved_percent()),
                        fade(ACCENT),
                        fade(ACCENT).gamma_multiply(0.14),
                    );
                    right = badge.min.x - 8.0;
                } else if doc.compressed {
                    right = rect.max.x - 56.0;
                }
                if hovered {
                    let close = Rect::from_center_size(pos2(rect.max.x - 20.0, rect.center().y), vec2(24.0, 24.0));
                    let close_resp = ui.interact(close, ui.scope_id().with(("close", doc.id)), Sense::click());
                    if close_resp.hovered() {
                        p.rect_filled(close, 7, DANGER.gamma_multiply(0.18));
                    }
                    Icon::Close.paint(ui, close.center(), 14.0, if close_resp.hovered() { DANGER } else { MUTED });
                    if close_resp.on_hover_text("Remover da lista").clicked() {
                        actions.push(Action::Remove(doc.id));
                    }
                }

                let text_left = rect.min.x + 40.0;
                if open > 0.6 {
                    let name = elided_name(ui, &doc.name, theme::regular(13.5), fade(TEXT), right - text_left);
                    p.galley(pos2(text_left, rect.min.y + 8.0), name, TEXT);
                    let detail = match &doc.error {
                        Some(e) => format!("{} · {e}", human_size(doc.size())),
                        None => format!("{} · {}", human_size(doc.size()), plural(doc.pages.len(), "página", "páginas")),
                    };
                    let detail = elided(ui, &detail, theme::regular(11.5), fade(if doc.error.is_some() { DANGER } else { MUTED }), right - text_left);
                    p.galley(pos2(text_left, rect.min.y + 27.0), detail, MUTED);
                }

                if resp.clicked() {
                    actions.push(Action::Select(doc.id));
                }
                if resp.drag_started() {
                    app.drag = Some(doc.id);
                }
            }
            if grouped {
                // O cartão do email, pintado por trás do que acabou de ser desenhado.
                let (pad, _) = ui.allocate_exact_size(vec2(width, 1.0 * appear), Sense::hover());
                let frame = Rect::from_min_max(pos2(left, top), pos2(left + width, pad.max.y));
                ui.painter().set(
                    card,
                    egui::epaint::RectShape::new(
                        frame,
                        14,
                        BG.gamma_multiply(appear),
                        Stroke::new(1.0, BORDER.gamma_multiply(appear)),
                        StrokeKind::Inside,
                    ),
                );
                ui.add_space(8.0 * appear);
            }
        }
        // Um respiro no fim, que também é onde se solta para mover ao final da lista.
        ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());

        if let Some(id) = app.drag {
            let pointer = ui.ctx().pointer_latest_pos();
            let released = ui.input(|i| i.pointer.any_released() || !i.pointer.any_down());
            if let (Some(pos), Some(first)) = (pointer, rows.first().copied()) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                // Antes da primeira linha cujo meio está abaixo do ponteiro; senão, no fim.
                let target = rows.iter().find(|(_, r)| pos.y < r.center().y).copied();
                let (to, y, x) = match target {
                    Some((i, r)) => (i, r.min.y - 3.0, r.min.x),
                    None => {
                        let (i, r) = rows.last().copied().unwrap_or(first);
                        (i + 1, r.max.y + 2.5, r.min.x)
                    }
                };
                let layer = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, ui.scope_id().with("drop")));
                layer.hline(x + 4.0..=first.1.max.x - 4.0, y, Stroke::new(2.0, ACCENT));
                layer.circle_filled(pos2(x + 4.0, y), 3.5, ACCENT);
                if let Some(doc) = app.state.docs.iter().find(|d| d.id == id) {
                    let ghost = Rect::from_min_size(pos + vec2(14.0, 6.0), vec2(240.0, 34.0));
                    layer.rect_filled(ghost.translate(vec2(0.0, 3.0)), 10, Color32::from_black_alpha(90));
                    layer.rect(ghost, 10, CARD_HOVER, Stroke::new(1.2, ACCENT), StrokeKind::Inside);
                    let name = elided_name(ui, &doc.name, theme::regular(12.5), TEXT, ghost.width() - 24.0);
                    layer.galley(ghost.min + vec2(12.0, 9.0), name, TEXT);
                }
                if released {
                    actions.push(Action::Move(id, to));
                }
            }
            if released {
                app.drag = None;
            }
        }
    });

    if animating {
        ui.ctx().request_repaint();
    } else if app.organized_at.is_some_and(|start| now - start > 0.6) {
        app.organized_at = None;
    }

    let ctx = ui.ctx().clone();
    for action in actions {
        match action {
            Action::Select(id) => app.select(Some(id)),
            Action::Check(id) => {
                if let Some(doc) = app.state.docs.iter_mut().find(|d| d.id == id) {
                    doc.checked = !doc.checked;
                }
            }
            Action::CheckAll(on) => app.state.docs.iter_mut().for_each(|d| d.checked = on),
            Action::Remove(id) => app.remove(id),
            Action::Move(id, to) => app.move_doc(id, to),
            Action::Fold(e) => {
                if !app.collapsed.remove(&e) {
                    app.collapsed.insert(e);
                }
            }
            Action::Sort(sort) => app.sort_by(sort),
            Action::Add => app.pick_files(&ctx),
        }
    }
}
