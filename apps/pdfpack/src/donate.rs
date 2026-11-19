//! A doação pelo Pix: três valores fixos, cada um com o seu QR code e o seu "copia e cola".
//!
//! Os códigos vêm prontos de `donate_data.rs`, gerado por `scripts/doacao.py`. O aplicativo só os
//! mostra: não fala com banco nem com a internet, e não sabe quem doou.

use egui::epaint::EllipseShape;
use egui::{Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::donate_data::{RECEIVER, TIERS};
use crate::icons::Icon;
use crate::modals::{buttons, heading, muted};
use crate::theme::{self, ACCENT, BORDER, CARD, CARD_HOVER, Kind, MUTED, TEXT};

/// Um valor de doação e o Pix dele.
pub struct Tier {
    pub name: &'static str,
    pub price: &'static str,
    /// Quantas moedas o cartão empilha: 1, 2 ou 3.
    pub coins: u8,
    /// O Pix copia e cola.
    pub code: &'static str,
    /// O QR code tem `size` módulos de lado; o bit `x` de `rows[y]` é o módulo da coluna `x`.
    pub size: usize,
    pub rows: &'static [u64],
}

/// Por quanto tempo o botão diz "Copiado" depois do clique.
const COPIED_FOR: f64 = 2.5;

/// O que a pessoa fez no diálogo neste quadro.
#[derive(Default)]
pub struct Outcome {
    pub pick: Option<usize>,
    pub copy: bool,
    pub close: bool,
}

/// O valor que abre selecionado: o do meio.
pub fn default_tier() -> usize {
    TIERS.len() / 2
}

pub fn dialog(ui: &mut Ui, tier: usize, copied: Option<f64>) -> Outcome {
    let mut out = Outcome::default();
    heading(ui, Icon::Heart, ACCENT, "Apoie o PDFPack", "Contribuição única pelo Pix. Não é assinatura");
    muted(
        ui,
        "O PDFPack é gratuito, sem propagandas e sem limites. Qualquer contribuição ajuda a cobrir os custos e a manter o aplicativo evoluindo.",
    );
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let count = TIERS.len().max(1) as f32;
        let width = (ui.available_width() - 8.0 * (count - 1.0)) / count;
        for (i, t) in TIERS.iter().enumerate() {
            if card(ui, t, width, i == tier) {
                out.pick = Some(i);
            }
        }
    });
    ui.add_space(6.0);
    if let Some(t) = TIERS.get(tier) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 16.0;
            qr(ui, t, 176.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 5.0;
                ui.add_space(2.0);
                ui.label(RichText::new(format!("{} · {}", t.name, t.price)).font(theme::semibold(15.0)));
                muted(ui, "1. Abra o aplicativo do seu banco no celular.");
                muted(ui, "2. Escolha Pix e depois Ler QR code.");
                muted(ui, format!("3. Confira o valor e o nome {RECEIVER} e confirme."));
                ui.add_space(6.0);
                let now = ui.input(|i| i.time);
                let fresh = copied.is_some_and(|at| now - at < COPIED_FOR);
                let (icon, text) = if fresh { (Icon::Check, "Código copiado") } else { (Icon::Copy, "Copiar Pix copia e cola") };
                if theme::button(ui, Some(icon), text, Kind::Quiet, true)
                    .on_hover_text("Para colar no Pix copia e cola do banco, neste computador")
                    .clicked()
                {
                    ui.ctx().copy_text(t.code.to_owned());
                    out.copy = true;
                }
                if fresh {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                }
            });
        });
    }
    out.close = buttons(ui, Some((Icon::Check, "Fechar")), None).0;
    out
}

/// O cartão de um valor: a pilha de moedas, o nome e o preço.
fn card(ui: &mut Ui, tier: &Tier, width: f32, selected: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 108.0), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), resp.hovered(), 0.08);
    let on = ui.ctx().animate_bool_with_time(resp.id.with("on"), selected, 0.12);
    let p = ui.painter();
    let fill = theme::mix(theme::mix(CARD, CARD_HOVER, hover), ACCENT.gamma_multiply(0.10), on);
    p.rect(rect, 12, fill, Stroke::new(1.0 + 0.5 * on, theme::mix(BORDER, ACCENT, on)), StrokeKind::Inside);
    coins(p, pos2(rect.center().x, rect.min.y + 48.0), tier.coins);
    p.text(pos2(rect.center().x, rect.min.y + 74.0), Align2::CENTER_CENTER, tier.name, theme::semibold(14.0), TEXT);
    p.text(pos2(rect.center().x, rect.min.y + 92.0), Align2::CENTER_CENTER, tier.price, theme::regular(12.5), MUTED);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

/// Uma pilha de `count` moedas, a de baixo apoiada em `base`.
fn coins(p: &egui::Painter, base: Pos2, count: u8) {
    let side = theme::mix(ACCENT, Color32::BLACK, 0.32);
    let edge = Stroke::new(1.2, theme::mix(ACCENT, Color32::BLACK, 0.55));
    let radius = vec2(15.0, 6.0);
    for i in 0..count.min(3) {
        let top = pos2(base.x, base.y - f32::from(i) * 8.0);
        let bottom = top + vec2(0.0, 5.0);
        p.add(EllipseShape::filled(bottom, radius, side));
        p.rect_filled(Rect::from_min_max(pos2(top.x - radius.x, top.y), pos2(top.x + radius.x, bottom.y)), 0, side);
        p.add(EllipseShape::stroke(bottom, radius, edge));
        p.add(EllipseShape::filled(top, radius, ACCENT));
        p.add(EllipseShape::stroke(top, radius, edge));
    }
    let top = pos2(base.x, base.y - f32::from(count.clamp(1, 3) - 1) * 8.0);
    p.add(EllipseShape::stroke(top, vec2(8.5, 3.0), Stroke::new(1.0, edge.color)));
}

/// O QR code em preto sobre branco, com a margem que os leitores pedem.
fn qr(ui: &mut Ui, tier: &Tier, side: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 10, Color32::WHITE);
    // Os bits moram em um u64: um código maior que isso não é desenhado pela metade.
    let n = tier.size;
    if n == 0 || n > 64 || tier.rows.len() < n {
        return;
    }
    // Módulos de tamanho inteiro em pixels: bordas borradas atrapalham a leitura pela câmera.
    let ppp = ui.pixels_per_point().max(0.5);
    let cell = ((side / (n as f32 + 8.0)) * ppp).floor().max(1.0) / ppp;
    let origin = rect.center() - vec2(cell * n as f32 / 2.0, cell * n as f32 / 2.0);
    for (y, row) in tier.rows.iter().take(n).enumerate() {
        let mut x = 0;
        while x < n {
            if (row >> x) & 1 == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < n && (row >> x) & 1 == 1 {
                x += 1;
            }
            let min = origin + vec2(start as f32 * cell, y as f32 * cell);
            p.rect_filled(Rect::from_min_size(min, vec2((x - start) as f32 * cell, cell)), 0, Color32::BLACK);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O CRC-16 que fecha todo BR Code (CCITT-FALSE), para conferir os códigos gerados.
    fn crc16(data: &[u8]) -> u16 {
        let mut crc = 0xFFFFu16;
        for b in data {
            crc ^= u16::from(*b) << 8;
            for _ in 0..8 {
                crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
            }
        }
        crc
    }

    #[test]
    fn every_pix_code_is_a_static_brcode_with_a_valid_checksum() {
        assert!(!TIERS.is_empty());
        for t in TIERS {
            let (body, sum) = t.code.split_at(t.code.len() - 4);
            assert!(t.code.starts_with("000201"), "{}", t.name);
            assert!(body.ends_with("6304"), "{}", t.name);
            assert!(body.contains("br.gov.bcb.pix"), "{}", t.name);
            assert!(body.contains("5303986"), "{}: a moeda é o real", t.name);
            assert_eq!(format!("{:04X}", crc16(body.as_bytes())), sum, "{}", t.name);
            // O valor do cartão é o valor do código.
            let amount = t.price.trim_start_matches("R$ ").replace(',', ".");
            assert!(body.contains(&format!("54{:02}{amount}", amount.len())), "{}", t.name);
        }
    }

    #[test]
    fn every_qr_code_fits_what_the_dialog_can_draw() {
        for t in TIERS {
            assert!((21..=64).contains(&t.size), "{}", t.name);
            assert_eq!(t.rows.len(), t.size, "{}", t.name);
            // Os três cantos de um QR code são sempre os quadrados de localização: 7 módulos cheios.
            let first = t.rows.first().copied().unwrap_or(0);
            assert_eq!(first & 0x7f, 0x7f, "{}", t.name);
            assert_eq!((first >> (t.size - 7)) & 0x7f, 0x7f, "{}", t.name);
            assert_eq!(t.rows.last().copied().unwrap_or(0) & 0x7f, 0x7f, "{}", t.name);
        }
    }

    #[test]
    fn the_dialog_opens_on_a_tier_that_exists() {
        assert!(TIERS.get(default_tier()).is_some());
    }
}
