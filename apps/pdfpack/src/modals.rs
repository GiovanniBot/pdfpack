//! Os diálogos: comprimir (escolha, andamento, resultado), organizar para email e salvar.

use std::sync::atomic::Ordering;

use egui::{Align, Align2, Color32, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::core::{self, Level, human_size};
use crate::icons::Icon;
use crate::list::plural;
use crate::theme::{self, ACCENT, BG, BORDER, CARD, CARD_HOVER, DANGER, Kind, MUTED, ON_ACCENT, TEXT};
use crate::{App, Modal};

enum Next {
    Stay,
    Close,
    Compress,
    Organize,
    Update,
    Restart,
    Install,
    InstallLater,
    Uninstall,
}

/// Uma opção de escolha única, com título e explicação.
fn choice_card(ui: &mut Ui, title: &str, hint: &str, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.08);
    let on = ui.ctx().animate_bool_with_time(resp.id.with("on"), selected, 0.12);
    let p = ui.painter();
    let fill = theme::mix(theme::mix(CARD, CARD_HOVER, hover), ACCENT.gamma_multiply(0.10), on);
    p.rect(rect, 11, fill, Stroke::new(1.0 + 0.5 * on, theme::mix(BORDER, ACCENT, on)), StrokeKind::Inside);
    let dot = pos2(rect.min.x + 21.0, rect.center().y);
    p.circle_stroke(dot, 7.5, Stroke::new(1.5, theme::mix(MUTED, ACCENT, on)));
    p.circle_filled(dot, 4.0 * on, ACCENT);
    p.text(pos2(rect.min.x + 42.0, rect.min.y + 18.0), Align2::LEFT_CENTER, title, theme::semibold(14.5), TEXT);
    p.text(pos2(rect.min.x + 42.0, rect.min.y + 37.0), Align2::LEFT_CENTER, hint, theme::regular(12.0), MUTED);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// O cabeçalho de um diálogo: o ícone num quadradinho, o título e uma linha de contexto.
fn heading(ui: &mut Ui, icon: Icon, color: Color32, title: &str, subtitle: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let (slot, _) = ui.allocate_exact_size(vec2(42.0, 42.0), Sense::hover());
        theme::tile(ui, slot.center(), 42.0, icon, color);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(if subtitle.is_empty() { 10.0 } else { 2.0 });
            ui.label(RichText::new(title).font(theme::semibold(18.0)));
            if !subtitle.is_empty() {
                ui.label(RichText::new(subtitle).color(MUTED).size(12.5));
            }
        });
    });
    ui.add_space(10.0);
}

fn muted(ui: &mut Ui, text: impl Into<String>) {
    ui.label(RichText::new(text).color(MUTED).size(12.5));
}

/// Os botões no rodapé do diálogo, o principal à direita.
fn buttons(ui: &mut Ui, primary: Option<(Icon, &str)>, secondary: Option<&str>) -> (bool, bool) {
    let mut out = (false, false);
    ui.add_space(16.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if let Some((icon, text)) = primary {
            out.0 = theme::button(ui, Some(icon), text, Kind::Primary, true).clicked();
        }
        if let Some(text) = secondary {
            out.1 = theme::button(ui, None, text, Kind::Quiet, true).clicked();
        }
    });
    out
}

fn bar(ui: &mut Ui, fraction: f32, color: Color32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(rect, 4, BG);
    let mut fill = rect;
    fill.set_width((rect.width() * fraction.clamp(0.0, 1.0)).max(8.0));
    ui.painter().rect_filled(fill, 4, color);
    rect
}

fn progress(ui: &mut Ui, icon: Icon, title: &str, p: &crate::state::Progress) {
    let f = p.get();
    // O número anda suave entre um aviso e outro do trabalho.
    let shown = ui.ctx().animate_value_with_time(egui::Id::unique(("progress", title.len())), f, 0.25);
    heading(ui, icon, ACCENT, title, "");
    ui.horizontal(|ui| {
        let label = p.label();
        let width = ui.available_width() - 52.0;
        let galley = crate::list::elided_name(ui, &label, theme::regular(12.5), MUTED, width);
        let (slot, _) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
        ui.painter().galley(slot.min, galley, MUTED);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(format!("{:.0}%", shown * 100.0)).font(theme::semibold(14.0)).color(ACCENT));
        });
    });
    bar(ui, shown, ACCENT);
    ui.ctx().request_repaint_after_secs(0.05);
}

fn level_card(ui: &mut Ui, level: Level, strength: usize, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 54.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.08);
    let on = ui.ctx().animate_bool_with_time(resp.id.with("on"), selected, 0.12);
    let p = ui.painter();
    let fill = theme::mix(theme::mix(CARD, CARD_HOVER, hover), ACCENT.gamma_multiply(0.10), on);
    p.rect(rect, 11, fill, Stroke::new(1.0 + 0.5 * on, theme::mix(BORDER, ACCENT, on)), StrokeKind::Inside);
    let dot = pos2(rect.min.x + 21.0, rect.center().y);
    p.circle_stroke(dot, 7.5, Stroke::new(1.5, theme::mix(MUTED, ACCENT, on)));
    p.circle_filled(dot, 4.0 * on, ACCENT);
    p.text(pos2(rect.min.x + 42.0, rect.min.y + 18.0), Align2::LEFT_CENTER, level.name(), theme::semibold(14.5), TEXT);
    p.text(pos2(rect.min.x + 42.0, rect.min.y + 37.0), Align2::LEFT_CENTER, level.hint(), theme::regular(12.0), MUTED);
    // Quatro barrinhas: quanto o nível aperta.
    for i in 0..4 {
        let h = 7.0 + i as f32 * 4.0;
        let x = rect.max.x - 44.0 + i as f32 * 7.0;
        let bar = Rect::from_min_max(pos2(x, rect.center().y + 9.5 - h), pos2(x + 4.0, rect.center().y + 9.5));
        p.rect_filled(bar, 2, if i < strength { theme::mix(MUTED, ACCENT, on) } else { BORDER });
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Antes e depois, lado a lado, na mesma escala.
fn comparison(ui: &mut Ui, before: u64, after: u64) {
    for (label, size, color, text) in [("Antes", before, BORDER.gamma_multiply(1.7), MUTED), ("Depois", after, ACCENT, TEXT)] {
        ui.horizontal(|ui| {
            let (slot, _) = ui.allocate_exact_size(vec2(50.0, 18.0), Sense::hover());
            ui.painter().text(slot.left_center(), Align2::LEFT_CENTER, label, theme::regular(12.5), MUTED);
            let width = ui.available_width() - 78.0;
            let (track, _) = ui.allocate_exact_size(vec2(width, 18.0), Sense::hover());
            let fill =
                Rect::from_min_size(pos2(track.min.x, track.center().y - 5.0), vec2((width * size as f32 / before.max(1) as f32).max(6.0), 10.0));
            ui.painter().rect_filled(fill, 5, color);
            let (num, _) = ui.allocate_exact_size(vec2(70.0, 18.0), Sense::hover());
            ui.painter().text(num.right_center(), Align2::RIGHT_CENTER, human_size(size), theme::semibold(12.5), text);
        });
    }
}

/// Quantos emails a organização deve dar, sem dividir nada de verdade.
fn estimate_emails(app: &App, limit: u64) -> usize {
    let mut sizes = Vec::new();
    for doc in &app.state.docs {
        let mut left = doc.size();
        while left > limit && doc.pages.len() > 1 {
            sizes.push(limit);
            left -= limit;
        }
        sizes.push(left);
    }
    core::pack(&sizes, limit).len()
}

pub fn show(app: &mut App, ctx: &egui::Context) {
    if matches!(app.modal, Modal::None) {
        return;
    }
    let shadow = egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(140) };
    let frame = egui::Frame::new().fill(theme::PANEL).stroke(Stroke::new(1.0, BORDER)).corner_radius(18).inner_margin(24).shadow(shadow);
    let mut next = Next::Stay;
    let mut pick_always = None;
    let mut toggle_desktop = false;
    let first_update = app.prefs.auto_update.is_none();
    // Cada diálogo tem a sua identidade: com uma só, o seguinte herdava a altura do anterior e
    // ficava com um vazio no meio.
    let response = egui::Modal::new(egui::Id::unique(("pdfpack-modal", std::mem::discriminant(&app.modal)))).frame(frame).backdrop_color(Color32::from_black_alpha(165)).show(ctx, |ui| {
        ui.set_width(450.0);
        ui.spacing_mut().item_spacing.y = 7.0;
        match &app.modal {
            Modal::None => {}
            Modal::InstallOffer { desktop } => {
                heading(ui, Icon::Monitor, ACCENT, "Instalar o PDFPack?", "Para abrir pelo menu Iniciar, sem procurar o arquivo");
                muted(ui, "O PDFPack é copiado para a sua pasta de programas e passa a aparecer no menu Iniciar. Não pede senha de administrador e continua se atualizando sozinho.");
                ui.add_space(4.0);
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
                let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.08);
                ui.painter().rect(rect, 11, theme::mix(CARD, CARD_HOVER, hover), Stroke::new(1.0, BORDER), StrokeKind::Inside);
                crate::list::checkbox(ui, Rect::from_center_size(pos2(rect.min.x + 22.0, rect.center().y), vec2(18.0, 18.0)), *desktop, resp.hovered(), 1.0);
                ui.painter().text(pos2(rect.min.x + 44.0, rect.center().y), Align2::LEFT_CENTER, "Criar atalho na área de trabalho", theme::semibold(14.0), TEXT);
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                toggle_desktop = resp.clicked();
                let (go, later) = buttons(ui, Some((Icon::Check, "Instalar")), Some("Agora não"));
                if go {
                    next = Next::Install;
                } else if later {
                    next = Next::InstallLater;
                }
            }
            Modal::UninstallConfirm => {
                heading(ui, Icon::Trash, DANGER, "Desinstalar o PDFPack?", "O aplicativo fecha em seguida");
                muted(ui, "Os atalhos, o programa e as preferências são removidos deste computador. Seus PDFs não são tocados; o que não foi salvo na lista atual se perde.");
                let (go, cancel) = buttons(ui, Some((Icon::Trash, "Desinstalar")), Some("Cancelar"));
                if go {
                    next = Next::Uninstall;
                } else if cancel {
                    next = Next::Close;
                }
            }
            Modal::CompressSetup => {
                let docs = &app.state.docs;
                let chosen: Vec<_> = docs.iter().filter(|d| d.checked).collect();
                let size: u64 = chosen.iter().map(|d| d.source.len() as u64).sum();
                let which = if docs.len() == 1 && chosen.len() == 1 {
                    "1 PDF".to_owned()
                } else if chosen.len() == docs.len() {
                    format!("Todos os {} PDFs", docs.len())
                } else {
                    format!("{} de {} PDFs selecionados", chosen.len(), docs.len())
                };
                heading(ui, Icon::Compress, ACCENT, "Comprimir PDFs", &format!("{which} · {}", human_size(size)));
                ui.label(RichText::new("Nível de compressão").font(theme::semibold(12.5)).color(MUTED));
                let mut pick = None;
                for (i, level) in Level::ALL.into_iter().enumerate() {
                    if level_card(ui, level, i + 1, app.prefs.level == level) {
                        pick = Some(level);
                    }
                }
                let none = chosen.is_empty();
                if none {
                    ui.horizontal(|ui| {
                        let (slot, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                        Icon::Alert.paint(ui, slot.center(), 14.0, DANGER);
                        ui.label(RichText::new("Marque ao menos um PDF na lista.").color(DANGER).size(12.5));
                    });
                }
                let (go, cancel) = buttons(ui, (!none).then_some((Icon::Compress, "Comprimir")), Some("Cancelar"));
                if let Some(level) = pick {
                    app.prefs.level = level;
                }
                if go {
                    next = Next::Compress;
                } else if cancel {
                    next = Next::Close;
                }
            }
            Modal::Compressing(p) => {
                progress(ui, Icon::Compress, "Comprimindo…", p);
                if buttons(ui, None, Some("Cancelar")).1 {
                    p.cancel.store(true, Ordering::Relaxed);
                }
            }
            Modal::CompressDone { before, after, count, failed } => {
                let saved = before.saturating_sub(*after);
                let percent = if *before == 0 { 0.0 } else { 100.0 * saved as f64 / *before as f64 };
                heading(ui, Icon::Done, ACCENT, "Compressão concluída", &format!("{} no nível {}", plural(*count, "PDF comprimido", "PDFs comprimidos"), app.prefs.level.name()));
                let (hero, _) = ui.allocate_exact_size(vec2(ui.available_width(), 84.0), Sense::hover());
                ui.painter().rect_filled(hero, 14, ACCENT);
                let number = ui.painter().text(pos2(hero.min.x + 22.0, hero.center().y), Align2::LEFT_CENTER, format!("−{percent:.0}%"), theme::semibold(40.0), ON_ACCENT);
                ui.painter().text(pos2(number.max.x + 18.0, hero.center().y - 10.0), Align2::LEFT_CENTER, format!("{} economizados", human_size(saved)), theme::semibold(16.0), ON_ACCENT);
                ui.painter().text(pos2(number.max.x + 18.0, hero.center().y + 12.0), Align2::LEFT_CENTER, "de armazenamento a menos", theme::regular(12.5), ON_ACCENT.gamma_multiply(0.75));
                ui.add_space(8.0);
                comparison(ui, *before, *after);
                if !failed.is_empty() {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let (slot, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                        Icon::Alert.paint(ui, slot.center(), 14.0, DANGER);
                        ui.label(RichText::new(format!("{} e ficaram como estavam:", plural(failed.len(), "não pôde ser comprimido", "não puderam ser comprimidos"))).color(DANGER).size(12.5));
                    });
                    egui::ScrollArea::vertical().max_height(90.0).show(ui, |ui| {
                        for (name, e) in failed {
                            muted(ui, format!("{name}: {e}"));
                        }
                    });
                }
                if buttons(ui, Some((Icon::Check, "Continuar")), None).0 {
                    next = Next::Close;
                }
            }
            Modal::OrganizeSetup => {
                heading(ui, Icon::Mail, ACCENT, "Organizar para email", "Pastas Email 1, Email 2… na ordem da lista, cada uma dentro do limite.");
                let (card, _) = ui.allocate_exact_size(vec2(ui.available_width(), 62.0), Sense::hover());
                ui.painter().rect(card, 12, CARD, Stroke::new(1.0, BORDER), StrokeKind::Inside);
                ui.painter().text(pos2(card.min.x + 16.0, card.center().y - 9.0), Align2::LEFT_CENTER, "Tamanho máximo por email", theme::semibold(14.0), TEXT);
                ui.painter().text(pos2(card.min.x + 16.0, card.center().y + 10.0), Align2::LEFT_CENTER, "Vale para a soma dos anexos de cada pasta", theme::regular(12.0), MUTED);
                let mut field = ui.new_child(egui::UiBuilder::new().max_rect(card.shrink2(vec2(12.0, 14.0))).layout(Layout::right_to_left(Align::Center)));
                field.spacing_mut().item_spacing.x = 4.0;
                if theme::icon_button(&mut field, Icon::Plus, Kind::Quiet, true).clicked() {
                    app.limit_text = (app.limit_text + 1.0).floor().min(2000.0);
                }
                field.label(RichText::new("MB").color(MUTED).size(12.5));
                field.spacing_mut().button_padding = vec2(6.0, 7.0);
                field.add_sized(vec2(58.0, 32.0), egui::DragValue::new(&mut app.limit_text).range(0.5..=2000.0).speed(0.25).max_decimals(1).update_while_editing(true)
                    .custom_formatter(|v, _| format!("{v:.1}").trim_end_matches('0').trim_end_matches('.').replace('.', ","))
                    .custom_parser(|t| t.trim().replace(',', ".").parse().ok()));
                if theme::icon_button(&mut field, Icon::Minus, Kind::Quiet, app.limit_text > 1.0).clicked() {
                    app.limit_text = (app.limit_text - 1.0).ceil().max(1.0);
                }

                // Os limites mais comuns, a um clique.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    for (mb, who) in [(10.0, "10 MB"), (20.0, "20 MB · Outlook"), (25.0, "25 MB · Gmail")] {
                        let on = (app.limit_text - mb).abs() < 0.01;
                        if theme::button(ui, None, who, if on { Kind::Outline } else { Kind::Quiet }, true).clicked() {
                            app.limit_text = mb;
                        }
                    }
                });

                let limit = (app.limit_text * 1024.0 * 1024.0) as u64;
                let big = app.state.docs.iter().filter(|d| d.size() > limit).count();
                let emails = estimate_emails(app, limit.max(1));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let (slot, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                    Icon::Mail.paint(ui, slot.center(), 14.0, ACCENT);
                    ui.label(RichText::new(format!("{} em cerca de {}", plural(app.state.docs.len(), "PDF", "PDFs"), plural(emails, "email", "emails"))).font(theme::semibold(13.0)));
                });
                if big > 0 {
                    ui.horizontal(|ui| {
                        let (slot, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                        Icon::Scissors.paint(ui, slot.center(), 14.0, MUTED);
                        muted(ui, format!("{} do limite e {} em partes.", plural(big, "PDF passa", "PDFs passam"), if big == 1 { "será dividido" } else { "serão divididos" }));
                    });
                }
                let (go, cancel) = buttons(ui, Some((Icon::Mail, "Organizar")), Some("Cancelar"));
                if go {
                    next = Next::Organize;
                } else if cancel {
                    next = Next::Close;
                }
            }
            Modal::Organizing(p) => {
                progress(ui, Icon::Scissors, "Organizando…", p);
                if buttons(ui, None, Some("Cancelar")).1 {
                    p.cancel.store(true, Ordering::Relaxed);
                }
            }
            Modal::Saving(p) => progress(ui, Icon::Save, "Salvando…", p),
            Modal::UpdateOffer { release, always } => {
                heading(ui, Icon::Sparkles, ACCENT, "Nova versão disponível", &format!("PDFPack {} → {}", crate::update::VERSION, release.version));
                if !release.notes.is_empty() {
                    egui::ScrollArea::vertical().max_height(110.0).show(ui, |ui| {
                        for line in release.notes.lines().filter(|l| !l.trim().is_empty()) {
                            muted(ui, line.trim_start_matches('#').trim().to_owned());
                        }
                    });
                    ui.add_space(4.0);
                }
                // Só na primeira atualização: como devem ser as próximas.
                if first_update {
                    ui.label(RichText::new("Daqui para frente").font(theme::semibold(12.5)).color(MUTED));
                    if choice_card(ui, "Atualizar sempre sozinho", "As próximas versões se instalam sem perguntar", *always) {
                        pick_always = Some(true);
                    }
                    if choice_card(ui, "Me perguntar a cada atualização", "Você decide quando instalar cada versão", !*always) {
                        pick_always = Some(false);
                    }
                } else {
                    muted(ui, "O download é feito em segundo plano e a versão nova vale ao reabrir o aplicativo.");
                }
                let (go, later) = buttons(ui, Some((Icon::Refresh, "Atualizar")), Some("Agora não"));
                if go {
                    next = Next::Update;
                } else if later {
                    next = Next::Close;
                }
            }
            Modal::Updating(p) => {
                progress(ui, Icon::Refresh, "Atualizando…", p);
                if buttons(ui, None, Some("Cancelar")).1 {
                    p.cancel.store(true, Ordering::Relaxed);
                }
            }
            Modal::UpdateDone { version } => {
                heading(ui, Icon::Done, ACCENT, "Atualização instalada", &format!("O PDFPack {version} abre na próxima vez."));
                muted(ui, "Reiniciar agora fecha a lista atual; salve antes se precisar.");
                let (go, later) = buttons(ui, Some((Icon::Refresh, "Reiniciar agora")), Some("Depois"));
                if go {
                    next = Next::Restart;
                } else if later {
                    next = Next::Close;
                }
            }
            Modal::Message { title, lines } => {
                heading(ui, Icon::Alert, DANGER, title, "");
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for line in lines {
                        muted(ui, line.clone());
                    }
                });
                if buttons(ui, Some((Icon::Check, "Continuar")), None).0 {
                    next = Next::Close;
                }
            }
        }
    });
    if let (Some(pick), Modal::UpdateOffer { always, .. }) = (pick_always, &mut app.modal) {
        *always = pick;
    }
    if let (true, Modal::InstallOffer { desktop }) = (toggle_desktop, &mut app.modal) {
        *desktop = !*desktop;
    }
    // Fechar a pergunta de instalação conta como "agora não": ela não volta a cada abertura.
    if response.should_close() && matches!(app.modal, Modal::InstallOffer { .. }) && matches!(next, Next::Stay) {
        next = Next::InstallLater;
    }
    // Esc ou um clique fora fecham os diálogos que não estão trabalhando.
    let idle = matches!(
        app.modal,
        Modal::CompressSetup
            | Modal::OrganizeSetup
            | Modal::CompressDone { .. }
            | Modal::Message { .. }
            | Modal::UpdateOffer { .. }
            | Modal::UpdateDone { .. }
            | Modal::UninstallConfirm
    );
    if response.should_close() && idle {
        next = Next::Close;
    }
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    if enter && matches!(next, Next::Stay) {
        next = match app.modal {
            Modal::CompressSetup if app.state.docs.iter().any(|d| d.checked) => Next::Compress,
            Modal::OrganizeSetup => Next::Organize,
            Modal::CompressDone { .. } | Modal::Message { .. } => Next::Close,
            _ => Next::Stay,
        };
    }
    match next {
        Next::Stay => {}
        Next::Close => app.modal = Modal::None,
        Next::InstallLater => app.install_later(),
        Next::Install => {
            if let Modal::InstallOffer { desktop } = app.modal {
                app.install(desktop, ctx);
            }
        }
        Next::Uninstall => {
            app.modal = Modal::None;
            app.uninstall(ctx);
        }
        Next::Compress => app.start_compress(ctx),
        Next::Organize => app.start_organize(ctx),
        Next::Update => {
            if let Modal::UpdateOffer { release, always } = std::mem::replace(&mut app.modal, Modal::None) {
                app.start_update(release, first_update.then_some(always), ctx);
            }
        }
        Next::Restart => {
            app.modal = Modal::None;
            app.restart(ctx);
        }
    }
}
