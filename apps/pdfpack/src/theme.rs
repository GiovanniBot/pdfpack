//! Cores, fontes e os poucos componentes visuais que o PDFPack repete.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};

use crate::icons::Icon;

pub const BG: Color32 = Color32::from_rgb(0x12, 0x13, 0x16);
pub const PANEL: Color32 = Color32::from_rgb(0x19, 0x1a, 0x1e);
pub const CARD: Color32 = Color32::from_rgb(0x22, 0x24, 0x29);
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x2b, 0x2d, 0x34);
pub const BORDER: Color32 = Color32::from_rgb(0x30, 0x32, 0x3a);
pub const ACCENT_HI: Color32 = Color32::from_rgb(0xff, 0xd8, 0x4a);
pub const GOOD: Color32 = Color32::from_rgb(0x5a, 0xd1, 0x8b);
pub const TEXT: Color32 = Color32::from_rgb(0xec, 0xec, 0xee);
pub const MUTED: Color32 = Color32::from_rgb(0x98, 0x9a, 0xa4);
pub const ACCENT: Color32 = Color32::from_rgb(0xf5, 0xc5, 0x18);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x1a, 0x15, 0x00);
pub const DANGER: Color32 = Color32::from_rgb(0xf0, 0x6a, 0x5a);
pub const VIEWER: Color32 = Color32::from_rgb(0x0f, 0x10, 0x12);

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("inter".into(), Arc::new(egui::FontData::from_static(include_bytes!("../../../assets/fonts/Inter-Regular.ttf"))));
    fonts
        .font_data
        .insert("inter-semibold".into(), Arc::new(egui::FontData::from_static(include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"))));
    let fallback = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    let mut semi = vec!["inter-semibold".to_owned()];
    semi.extend(fallback);
    fonts.families.insert(FontFamily::Name("semibold".into()), semi);
    ctx.set_fonts(fonts);

    egui_extras::install_image_loaders(ctx);
    ctx.set_theme(egui::Theme::Dark);
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = PANEL;
        v.window_fill = CARD;
        v.extreme_bg_color = BG;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.hyperlink_color = ACCENT;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.inactive.bg_fill = CARD;
        v.widgets.inactive.weak_bg_fill = CARD;
        v.widgets.hovered.bg_fill = CARD_HOVER;
        v.widgets.hovered.weak_bg_fill = CARD_HOVER;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.active.bg_fill = CARD_HOVER;
        v.widgets.active.weak_bg_fill = CARD_HOVER;
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.noninteractive, &mut v.widgets.open] {
            w.corner_radius = CornerRadius::same(8);
        }
        style.spacing.item_spacing = vec2(8.0, 8.0);
        style.spacing.button_padding = vec2(14.0, 7.0);
        style.spacing.scroll.bar_width = 8.0;
        for font in style.text_styles.values_mut() {
            font.size = (font.size * 1.08).round();
        }
    });
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0.0, 1.0)).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Amarelo cheio: o próximo passo.
    Primary,
    /// Só a borda amarela: um passo disponível, mas não o próximo.
    Outline,
    /// Neutro com borda: ações secundárias (Cancelar).
    Quiet,
    /// Sem fundo até o ponteiro passar: ícones de barra.
    Ghost,
}

fn paint_button(ui: &Ui, rect: Rect, resp: &Response, kind: Kind, enabled: bool) -> Color32 {
    let hover = ui.ctx().animate_bool_with_time(resp.id.with("hover"), enabled && resp.hovered(), 0.09);
    let (fill, fg, stroke) = match kind {
        Kind::Primary => (mix(ACCENT, ACCENT_HI, hover), ON_ACCENT, Stroke::NONE),
        Kind::Outline => (ACCENT.gamma_multiply(0.07 + 0.11 * hover), ACCENT, Stroke::new(1.2, ACCENT.gamma_multiply(0.85))),
        Kind::Quiet => (mix(CARD, CARD_HOVER, hover), TEXT, Stroke::new(1.0, BORDER)),
        Kind::Ghost => (CARD_HOVER.gamma_multiply(hover), mix(MUTED, TEXT, hover), Stroke::NONE),
    };
    let alpha = if enabled { 1.0 } else { 0.36 };
    let rect = if enabled && resp.is_pointer_button_down_on() { rect.shrink(0.6) } else { rect };
    ui.painter().rect(rect, 9, fill.gamma_multiply(alpha), Stroke::new(stroke.width, stroke.color.gamma_multiply(alpha)), egui::StrokeKind::Inside);
    if enabled && resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    fg.gamma_multiply(alpha)
}

/// Um botão com ícone e texto.
pub fn button(ui: &mut Ui, icon: Option<Icon>, text: &str, kind: Kind, enabled: bool) -> Response {
    let width = ui.painter().layout_no_wrap(text.to_owned(), semibold(13.5), TEXT).size().x;
    let lead = if icon.is_some() { 23.0 } else { 0.0 };
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0 + lead + width, 34.0), if enabled { Sense::click() } else { Sense::hover() });
    let fg = paint_button(ui, rect, &resp, kind, enabled);
    if let Some(icon) = icon {
        icon.paint(ui, pos2(rect.min.x + 14.0 + 8.0, rect.center().y), 16.0, fg);
    }
    ui.painter().text(pos2(rect.min.x + 14.0 + lead, rect.center().y), egui::Align2::LEFT_CENTER, text, semibold(13.5), fg);
    resp
}

/// Um botão quadrado só com ícone.
pub fn icon_button(ui: &mut Ui, icon: Icon, kind: Kind, enabled: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), if enabled { Sense::click() } else { Sense::hover() });
    let fg = paint_button(ui, rect, &resp, kind, enabled);
    icon.paint(ui, rect.center(), 16.0, fg);
    resp
}

/// Uma etiqueta arredondada, alinhada pela direita em `right`. Devolve o retângulo que ocupou.
pub fn pill(painter: &egui::Painter, right: Pos2, text: &str, fg: Color32, bg: Color32) -> Rect {
    let galley = painter.layout_no_wrap(text.to_owned(), semibold(11.5), fg);
    let rect = Rect::from_min_size(pos2(right.x - galley.size().x - 14.0, right.y - 10.0), vec2(galley.size().x + 14.0, 20.0));
    painter.rect_filled(rect, 10, bg);
    painter.galley(pos2(rect.min.x + 7.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    rect
}

/// O quadradinho colorido com um ícone que abre diálogos e marca as pastas.
pub fn tile(ui: &Ui, center: Pos2, side: f32, icon: Icon, color: Color32) {
    ui.painter().rect_filled(Rect::from_center_size(center, vec2(side, side)), (side * 0.28) as u8, color.gamma_multiply(0.16));
    icon.paint(ui, center, side * 0.52, color);
}

/// Uma seta ">" que gira para "v" conforme `open` vai de 0 a 1.
pub fn chevron(painter: &egui::Painter, center: Pos2, open: f32, color: Color32) {
    let angle = open * std::f32::consts::FRAC_PI_2;
    let rot = |p: Vec2| center + vec2(p.x * angle.cos() - p.y * angle.sin(), p.x * angle.sin() + p.y * angle.cos());
    let stroke = Stroke::new(1.6, color);
    painter.line_segment([rot(vec2(-2.0, -4.0)), rot(vec2(2.5, 0.0))], stroke);
    painter.line_segment([rot(vec2(2.5, 0.0)), rot(vec2(-2.0, 4.0))], stroke);
}

/// O ícone da janela: um quadrado amarelo arredondado com uma folha dobrada.
/// O logo do PDFPack (o canguru), já com os cantos arredondados.
const LOGO: &[u8] = include_bytes!("../assets/icon.png");

pub fn icon() -> egui::IconData {
    // O PNG vem embutido e é válido; se um dia não for, o ícone desenhado assume.
    eframe::icon_data::from_png_bytes(LOGO).unwrap_or_else(|_| drawn_icon())
}

/// O logo como textura, criada na primeira vez que é pedida.
pub fn logo(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::unique("pdfpack-logo");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return texture;
    }
    let icon = icon();
    let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
    let texture = ctx.load_texture("pdfpack-logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    texture
}

fn drawn_icon() -> egui::IconData {
    const N: usize = 64;
    let mut rgba = vec![0u8; N * N * 4];
    let inside = |x: f32, y: f32, r: Rect, radius: f32| {
        let dx = (r.min.x + radius - x).max(x - (r.max.x - radius)).max(0.0);
        let dy = (r.min.y + radius - y).max(y - (r.max.y - radius)).max(0.0);
        r.contains(pos2(x, y)) && dx * dx + dy * dy <= radius * radius
    };
    let tile = Rect::from_min_max(pos2(2.0, 2.0), pos2(62.0, 62.0));
    let sheet = Rect::from_min_max(pos2(19.0, 13.0), pos2(45.0, 51.0));
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let fold = fx - 45.0 + fy - 13.0 < -9.0 + 18.0 && fx > 36.0 && fy < 22.0 && (fx - 36.0) > -(22.0 - fy) && fx - 36.0 + (22.0 - fy) > 9.0;
            let lines = [30.0, 36.0, 42.0].iter().any(|ly| (fy - ly).abs() < 1.4 && fx > 24.0 && fx < 40.0);
            let c = if !inside(fx, fy, tile, 13.0) {
                [0, 0, 0, 0]
            } else if inside(fx, fy, sheet, 3.0) && !fold {
                if lines { [0xf5, 0xc5, 0x18, 255] } else { [0x1a, 0x15, 0x00, 255] }
            } else {
                [0xf5, 0xc5, 0x18, 255]
            };
            rgba[(y * N + x) * 4..][..4].copy_from_slice(&c);
        }
    }
    egui::IconData { rgba, width: N as u32, height: N as u32 }
}
