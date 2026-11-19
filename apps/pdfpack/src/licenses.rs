//! Os avisos de licença de tudo o que vai dentro do PDFPack, em Sobre ▸ Licenças de terceiros.
//!
//! As licenças MIT, Apache, BSD e parecidas pedem que o texto delas e os créditos de autoria
//! acompanhem o programa. O arquivo é gerado por `scripts/licencas.py` a partir das dependências
//! que entram no executável; o `scripts/release.ps1` não publica se ele estiver desatualizado.

use std::sync::OnceLock;

use egui::{Align2, FontId, ScrollArea, Sense, Ui, pos2, vec2};

use crate::icons::Icon;
use crate::modals::{buttons, heading, muted};
use crate::theme::{ACCENT, BORDER, CARD, TEXT};

/// O texto completo, embutido no executável.
pub const TEXT_ALL: &str = include_str!("../LICENCAS-DE-TERCEIROS.txt");

const LINE: f32 = 15.0;

fn line_count() -> usize {
    static COUNT: OnceLock<usize> = OnceLock::new();
    *COUNT.get_or_init(|| TEXT_ALL.lines().count())
}

fn longest_line() -> usize {
    static LONGEST: OnceLock<usize> = OnceLock::new();
    *LONGEST.get_or_init(|| TEXT_ALL.lines().map(|l| l.chars().count()).max().unwrap_or(0))
}

/// Desenha o diálogo; devolve se a pessoa pediu para fechar.
pub fn dialog(ui: &mut Ui) -> bool {
    let width = (ui.ctx().content_rect().width() - 80.0).clamp(450.0, 760.0);
    ui.set_width(width);
    heading(ui, Icon::File, ACCENT, "Licenças de terceiros", "O que vai dentro do PDFPack e sob quais licenças");
    muted(
        ui,
        "O PDFPack é livre (MIT ou Apache-2.0) e usa bibliotecas e arquivos de outros autores, todos sob licenças que permitem distribuí-lo. Os créditos e os textos das licenças estão abaixo.",
    );
    ui.add_space(4.0);
    let height = (ui.ctx().content_rect().height() - 330.0).clamp(160.0, 520.0);
    let font = FontId::monospace(11.5);
    // Linhas de largura fixa: a mais longa define até onde a rolagem horizontal vai.
    let char_width = ui.fonts_mut(|f| f.glyph_width(&font, 'M'));
    let width = char_width * longest_line() as f32 + 8.0;
    egui::Frame::new().fill(CARD).stroke(egui::Stroke::new(1.0, BORDER)).corner_radius(10).inner_margin(10).show(ui, |ui| {
        // Sem espaço entre as linhas: é com a altura de cada uma que a rolagem calcula o que aparece.
        ui.spacing_mut().item_spacing.y = 0.0;
        ScrollArea::both().max_height(height).auto_shrink([false, false]).show_rows(ui, LINE, line_count(), |ui, rows| {
            for line in TEXT_ALL.lines().skip(rows.start).take(rows.len()) {
                let (rect, _) = ui.allocate_exact_size(vec2(width.max(ui.available_width()), LINE), Sense::hover());
                ui.painter().text(pos2(rect.min.x, rect.center().y), Align2::LEFT_CENTER, line, font.clone(), TEXT);
            }
        });
    });
    buttons(ui, Some((Icon::Check, "Fechar")), None).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_notices_the_licences_require_are_in_the_app() {
        // O cabeçalho com a licença do próprio PDFPack/PDFCraft e os avisos que as licenças
        // pedem por escrito.
        for needed in [
            "ArtCraft Team and the PdfCraft contributors",
            "Independent JPEG Group",
            "CDLA-Permissive-2.0",
            "LGPL-2.1-or-later",
            "Lucide",
            "OFL-Inter.txt",
            "LICENSE_FOXIT",
        ] {
            assert!(TEXT_ALL.contains(needed), "falta {needed:?} nas licenças de terceiros");
        }
        assert!(line_count() > 1000);
    }
}
