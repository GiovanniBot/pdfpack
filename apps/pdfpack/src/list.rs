//! A barra de ações e a listagem dos PDFs (com os cartões "Email N" quando organizada).

use std::collections::{HashMap, HashSet};

use egui::{Align, Align2, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};

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
    /// Um clique numa linha, com Ctrl e com Shift.
    Pick(u64, bool, bool),
    /// Um clique no fundo: a seleção volta a ser só o PDF aberto.
    PickShown,
    /// O retângulo de seleção terminou: o visualizador acompanha.
    ShowPicked,
    Check(u64),
    CheckAll(bool),
    Remove(u64),
    Move(HashSet<u64>, usize),
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

/// Um arrasto em andamento: as linhas que vão junto e o cartão que segue o ponteiro.
pub struct Drag {
    pub ids: HashSet<u64>,
    /// A linha que foi agarrada: é ela que o cartão mostra.
    lead: u64,
    /// Onde o cartão foi agarrado, a partir do canto de cima à esquerda.
    grab: Vec2,
    size: Vec2,
    /// O canto do cartão na tela.
    pos: Pos2,
    /// Antes de qual posição da lista as linhas caem se forem soltas agora.
    to: usize,
    /// A inclinação, em radianos, e o quanto o cartão está erguido (0 a 1).
    tilt: f32,
    lift: f32,
    /// Solto: quando, e de onde o cartão voa até o lugar em que pousa.
    landing: Option<(f64, Pos2)>,
}

/// Um retângulo de seleção em andamento.
pub struct Marquee {
    /// Onde começou, medido a partir do topo do conteúdo da lista (acompanha a rolagem).
    start: Vec2,
    /// O que já estava selecionado, quando o retângulo soma (Ctrl ou Shift).
    base: HashSet<u64>,
}

const PRESENT: u8 = 0;
const GAP: u8 = 1;
const LIFT: u8 = 2;

/// Leva o valor guardado em `key` na direção de `target`, um pouco a cada quadro.
fn approach(map: &mut HashMap<(u8, u64), f32>, key: (u8, u64), start: f32, target: f32, dt: f32, moving: &mut bool) -> f32 {
    let v = map.entry(key).or_insert(start);
    let next = *v + (target - *v) * (1.0 - (-20.0 * dt).exp());
    *v = if (target - next).abs() < 0.004 { target } else { next };
    *moving |= *v != target;
    *v
}

/// O espaço que a lista abre onde as linhas arrastadas vão cair. Devolve o lugar da linha nele.
fn gap(ui: &mut Ui, size: f32, width: f32, inset: f32) -> Option<Rect> {
    let height = (ROW + 5.0) * size;
    if height < 0.5 {
        return None;
    }
    let (slot, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let place = Rect::from_min_size(slot.min + vec2(inset, 0.0), vec2(width - 2.0 * inset, ROW));
    let shown = Rect::from_min_max(place.min, pos2(place.max.x, slot.max.y - 5.0 * size));
    if shown.height() > 6.0 {
        let p = ui.painter().with_clip_rect(slot);
        p.rect_filled(shown, 10, ACCENT.gamma_multiply(0.07 * size));
        p.extend(Shape::dashed_line(&rounded_outline(shown.shrink(0.5), 10.0), Stroke::new(1.3, ACCENT.gamma_multiply(0.75 * size)), 6.0, 5.0));
    }
    Some(place)
}

/// O cartão que segue o ponteiro durante o arrasto: inclinado em torno do ponto em que foi
/// agarrado, com sombra, e empilhado quando leva mais de um PDF.
fn floating_card(ui: &Ui, painter: &egui::Painter, drag: &Drag, doc: &crate::state::Doc) {
    let lift = drag.lift;
    let rect = Rect::from_min_size(drag.pos, drag.size).expand(1.5 * lift);
    let pivot = drag.pos + drag.grab;
    let turned = |points: Vec<Pos2>, angle: f32, shift: Vec2| -> Vec<Pos2> {
        let rot = egui::emath::Rot2::from_angle(angle);
        points.into_iter().map(|p| pivot + rot * (p - pivot) + shift).collect()
    };
    let outline = |r: Rect, radius: f32| {
        let mut points = rounded_outline(r, radius);
        points.pop();
        points
    };
    // A sombra: camadas largas e fracas por baixo, mais forte quanto mais erguido.
    for k in 0..5 {
        let grow = k as f32 * 2.6 * lift;
        let shadow = turned(outline(rect.expand(grow), 10.0 + grow), drag.tilt, vec2(0.0, 5.0 + 7.0 * lift));
        painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha((34.0 * lift) as u8), Stroke::NONE));
    }
    // Os outros PDFs do grupo, espiando por trás.
    let behind = drag.ids.len().saturating_sub(1).min(2);
    for k in (1..=behind).rev() {
        let k = k as f32;
        let points = turned(outline(rect, 10.0), drag.tilt + 0.035 * k * lift, vec2(3.0 * k, 5.0 * k) * lift);
        painter.add(Shape::convex_polygon(points.clone(), theme::mix(CARD, BG, 0.25 * k), Stroke::NONE));
        painter.add(Shape::closed_line(points, Stroke::new(1.0, BORDER.gamma_multiply(1.6))));
    }
    let points = turned(outline(rect, 10.0), drag.tilt, egui::Vec2::ZERO);
    painter.add(Shape::convex_polygon(points.clone(), CARD_HOVER, Stroke::NONE));
    painter.add(Shape::closed_line(points, Stroke::new(1.4, ACCENT)));

    let at = |p: Pos2| turned(vec![p], drag.tilt, egui::Vec2::ZERO)[0];
    let check = Rect::from_center_size(pos2(rect.min.x + 20.0, rect.center().y), vec2(17.0, 17.0));
    let square = turned(outline(check, 5.0), drag.tilt, egui::Vec2::ZERO);
    if doc.checked {
        painter.add(Shape::convex_polygon(square, ACCENT, Stroke::NONE));
        let c = check.center();
        let tick = vec![at(c + vec2(-4.0, 0.2)), at(c + vec2(-1.2, 3.0)), at(c + vec2(4.2, -3.2))];
        painter.add(Shape::line(tick, Stroke::new(1.8, ON_ACCENT)));
    } else {
        painter.add(Shape::closed_line(square, Stroke::new(1.4, MUTED)));
    }
    let width = rect.width() - 40.0 - 26.0;
    let name = elided_name(ui, &doc.name, theme::regular(13.5), TEXT, width);
    painter.add(egui::epaint::TextShape::new(at(pos2(rect.min.x + 40.0, rect.min.y + 9.0)), name, TEXT).with_angle(drag.tilt));
    let detail = format!("{} · {}", human_size(doc.size()), plural(doc.pages.len(), "página", "páginas"));
    let detail = elided(ui, &detail, theme::regular(11.5), MUTED, width);
    painter.add(egui::epaint::TextShape::new(at(pos2(rect.min.x + 40.0, rect.min.y + 28.0)), detail, MUTED).with_angle(drag.tilt));
    // Quantos PDFs vão junto.
    if drag.ids.len() > 1 {
        let c = at(rect.right_top() + vec2(-8.0, 4.0));
        painter.circle_filled(c, 12.5, ACCENT);
        painter.circle_stroke(c, 12.5, Stroke::new(2.0, BG));
        painter.text(c, Align2::CENTER_CENTER, drag.ids.len().to_string(), theme::semibold(12.0), ON_ACCENT);
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
    let (now, dt) = ui.input(|i| (i.time, i.stable_dt.min(0.05)));
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
    // Onde cada linha ficou neste quadro: é onde se solta um PDF e o que o retângulo de seleção toca.
    let mut rows: Vec<(usize, Rect)> = Vec::new();
    // Durante um arrasto as linhas levadas somem da lista e ela abre espaço onde vão cair.
    let carried: HashSet<u64> = app.drag.as_ref().map(|d| d.ids.clone()).unwrap_or_default();
    let drop_at = app.drag.as_ref().map(|d| d.to);
    let live = app.drag.is_some();
    let mut landing: Option<Rect> = None;
    let mut finished = false;

    egui::ScrollArea::vertical().id_salt("docs").auto_shrink(false).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let width = ui.available_width();
        let left = ui.cursor().min.x;
        let origin = ui.cursor().min;
        let view = ui.clip_rect();
        // O fundo da lista, por baixo das linhas: arrastar a partir dele desenha o retângulo de seleção.
        let background = ui.interact(view, ui.scope_id().with("background"), Sense::click_and_drag());
        let mut order = 0;
        for (g, (email, docs)) in groups.iter().enumerate() {
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
            // O espaço antes da posição `at`, que só existe durante um arrasto.
            let mut space = |ui: &mut Ui, app: &mut App, at: usize, animating: &mut bool| {
                if !live {
                    return;
                }
                let size = approach(&mut app.list_anim, (GAP, at as u64), 0.0, if drop_at == Some(at) { 1.0 } else { 0.0 }, dt, animating);
                let place = gap(ui, size * open, width, inset);
                if drop_at == Some(at) {
                    landing = place;
                }
            };
            for &i in docs {
                if open <= 0.01 {
                    break;
                }
                space(ui, app, i, &mut animating);
                let doc = &app.state.docs[i];
                let t = if grouped { enter(order) } else { 1.0 };
                if grouped {
                    order += 1;
                    animating |= t < 1.0;
                }
                // Uma linha levada pelo arrasto encolhe até sumir; solta, a lista volta ao normal de uma vez.
                let present = if live {
                    approach(&mut app.list_anim, (PRESENT, doc.id), 1.0, if carried.contains(&doc.id) { 0.0 } else { 1.0 }, dt, &mut animating)
                } else {
                    1.0
                };
                let step = (ROW + 5.0) * open * present;
                let (slot, _) = ui.allocate_exact_size(vec2(width, step), Sense::hover());
                // Na entrada, a linha desliza para dentro do cartão.
                let rect = Rect::from_min_size(slot.min + vec2(inset - (1.0 - t) * 18.0, 0.0), vec2(width - 2.0 * inset, ROW));
                if !carried.contains(&doc.id) {
                    rows.push((i, rect));
                }
                if !ui.is_rect_visible(slot) || present < 0.05 {
                    continue;
                }
                let whole = open > 0.99 && t > 0.99 && present > 0.99;
                let clip = slot.expand2(vec2(0.0, 0.5));
                let resp = ui.interact(rect.intersect(slot), ui.scope_id().with(("row", doc.id)), Sense::click_and_drag());
                let picked = app.picked.contains(&doc.id);
                // O PDF aberto no visualizador, quando faz parte da seleção.
                let shown = picked && app.selected == Some(doc.id);
                let alpha = t * present;
                let fade = |c: Color32| c.gamma_multiply(alpha);
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered() || picked, 0.08);
                let p = ui.painter().with_clip_rect(clip);
                p.rect_filled(rect, 10, fade(theme::mix(if grouped { CARD } else { CARD.gamma_multiply(0.75) }, CARD_HOVER, hover)));
                if picked && !shown {
                    p.rect_filled(rect, 10, fade(ACCENT).gamma_multiply(0.05));
                }
                // Comprimidos ficam com a borda amarela, mais forte no selecionado.
                if doc.compressed {
                    p.rect_stroke(rect, 10, Stroke::new(1.2, fade(ACCENT).gamma_multiply(if picked { 1.0 } else { 0.55 })), StrokeKind::Inside);
                } else if picked {
                    p.rect_stroke(rect, 10, Stroke::new(1.0, fade(MUTED).gamma_multiply(0.7)), StrokeKind::Inside);
                }
                if shown {
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
                let hovered = whole && ui.rect_contains_pointer(rect) && !live && app.marquee.is_none();
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
                if open * present > 0.6 {
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
                    let keys = ui.input(|i| i.modifiers);
                    actions.push(Action::Pick(doc.id, keys.command, keys.shift));
                }
                if resp.drag_started() && whole && app.drag.is_none() {
                    // Arrastar uma linha da seleção leva a seleção inteira; outra linha vai sozinha.
                    let ids = if picked { app.picked.clone() } else { HashSet::from([doc.id]) };
                    if !picked {
                        actions.push(Action::Pick(doc.id, false, false));
                    }
                    let press = ui.input(|i| i.pointer.press_origin()).unwrap_or(rect.center());
                    let grab = (press - rect.min).clamp(egui::Vec2::ZERO, rect.size());
                    app.drag = Some(Drag { ids, lead: doc.id, grab, size: rect.size(), pos: rect.min, to: i, tilt: 0.0, lift: 0.0, landing: None });
                }
            }
            // Depois da última linha da lista também se solta.
            if g + 1 == groups.len() && open > 0.01 {
                space(ui, app, n, &mut animating);
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

        let pointer = ui.ctx().pointer_latest_pos();
        let held = ui.input(|i| i.pointer.primary_down());
        // Perto da borda de cima ou de baixo, a lista rola sozinha para o arrasto alcançar o resto.
        let edge_scroll = |ui: &Ui, pos: Pos2| {
            let reach = 36.0;
            let pull = ((view.min.y + reach - pos.y) / reach).clamp(0.0, 1.5) - ((pos.y - (view.max.y - reach)) / reach).clamp(0.0, 1.5);
            if pull != 0.0 {
                ui.scroll_with_delta(vec2(0.0, pull * 900.0 * dt));
            }
        };
        let above = egui::LayerId::new(egui::Order::Tooltip, ui.scope_id().with("above"));

        // O retângulo de seleção, a partir do fundo da lista.
        if background.drag_started()
            && app.drag.is_none()
            && let Some(press) = ui.input(|i| i.pointer.press_origin())
        {
            let keys = ui.input(|i| i.modifiers);
            let base = if keys.command || keys.shift { app.picked.clone() } else { HashSet::new() };
            app.marquee = Some(Marquee { start: press - origin, base });
        } else if background.clicked() {
            actions.push(Action::PickShown);
        }
        if let Some(marquee) = &app.marquee {
            if let Some(pos) = pointer {
                let area = Rect::from_two_pos(origin + marquee.start, pos);
                let mut picked = marquee.base.clone();
                picked.extend(rows.iter().filter(|(_, r)| r.intersects(area)).filter_map(|(i, _)| app.state.docs.get(*i).map(|d| d.id)));
                app.picked = picked;
                let shown = area.intersect(view);
                let painter = ui.ctx().layer_painter(above);
                painter.rect(shown, 4, ACCENT.gamma_multiply(0.10), Stroke::new(1.0, ACCENT.gamma_multiply(0.8)), StrokeKind::Inside);
                if held {
                    edge_scroll(ui, pos);
                }
            }
            if !held {
                app.marquee = None;
                actions.push(Action::ShowPicked);
            }
            ui.ctx().request_repaint();
        }

        if let Some(drag) = &mut app.drag {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            let mut motion = false;
            match drag.landing {
                None => {
                    if let Some(pos) = pointer {
                        drag.pos = pos - drag.grab;
                        edge_scroll(ui, pos);
                    }
                    // Cai antes da primeira linha cujo meio está abaixo do meio do cartão; senão, no fim.
                    let middle = drag.pos.y + drag.size.y / 2.0;
                    drag.to = rows.iter().find(|(_, r)| middle < r.center().y).map_or(n, |(i, _)| *i);
                    // "Tortinho": sempre um pouco inclinado, e mais quando é puxado para os lados.
                    let pull = ui.input(|i| i.pointer.velocity().x);
                    let tilt = (-0.045 + pull * 0.00020).clamp(-0.15, 0.15);
                    drag.tilt += (tilt - drag.tilt) * (1.0 - (-12.0 * dt).exp());
                    drag.lift = approach(&mut app.list_anim, (LIFT, 0), 0.0, 1.0, dt, &mut motion);
                    if !held {
                        drag.landing = Some((now, drag.pos));
                    }
                }
                Some((since, from)) => {
                    // Solto: o cartão endireita e voa até o espaço aberto na lista.
                    let t = (((now - since) / 0.17) as f32).clamp(0.0, 1.0);
                    let e = 1.0 - (1.0 - t).powi(3);
                    let goal = landing.map_or(from, |r| r.min);
                    drag.pos = from + (goal - from) * e;
                    drag.tilt *= 1.0 - e;
                    drag.lift = 1.0 - e;
                    if t >= 1.0 {
                        actions.push(Action::Move(drag.ids.clone(), drag.to));
                        finished = true;
                    }
                }
            }
            if let Some(doc) = app.state.docs.iter().find(|d| d.id == drag.lead) {
                floating_card(ui, &ui.ctx().layer_painter(above), drag, doc);
            } else {
                // A linha agarrada saiu da lista no meio do arrasto: não há o que mover.
                finished = true;
            }
            ui.ctx().request_repaint();
        }
    });

    if finished {
        // A lista volta ao normal de uma vez: a linha aparece onde o cartão pousou.
        app.drag = None;
        app.list_anim.clear();
    }
    if animating {
        ui.ctx().request_repaint();
    } else if app.organized_at.is_some_and(|start| now - start > 0.6) {
        app.organized_at = None;
    }

    let ctx = ui.ctx().clone();
    for action in actions {
        match action {
            Action::Pick(id, ctrl, shift) => app.pick(id, ctrl, shift),
            Action::PickShown => app.pick_shown(),
            Action::ShowPicked => app.show_picked(),
            Action::Check(id) => {
                // Marcar uma linha da seleção marca (ou desmarca) a seleção inteira.
                let Some(on) = app.state.docs.iter().find(|d| d.id == id).map(|d| !d.checked) else { continue };
                let group = app.picked.contains(&id) && app.picked.len() > 1;
                for doc in app.state.docs.iter_mut().filter(|d| d.id == id || (group && app.picked.contains(&d.id))) {
                    doc.checked = on;
                }
            }
            Action::CheckAll(on) => app.state.docs.iter_mut().for_each(|d| d.checked = on),
            Action::Remove(id) => app.remove(id),
            Action::Move(ids, to) => app.move_docs(&ids, to),
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
