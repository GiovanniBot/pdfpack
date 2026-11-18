//! Os ícones do aplicativo: Lucide (ISC/MIT, `assets/icons/LICENSE-lucide.txt`), desenhados como
//! vetor em qualquer tamanho e pintados na cor que o lugar pede.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use egui::{Color32, Pos2, Rect, Ui, vec2};

macro_rules! icons {
    ($($name:ident => $path:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Icon { $($name),* }

        impl Icon {
            fn svg(self) -> &'static str {
                match self { $(Icon::$name => include_str!($path)),* }
            }
        }
    };
}

icons! {
    Compress => "../../../assets/icons/minimize-2.svg",
    Mail => "../icons/mail.svg",
    Save => "../../../assets/icons/save.svg",
    Undo => "../../../assets/icons/undo-2.svg",
    Redo => "../../../assets/icons/redo-2.svg",
    Plus => "../../../assets/icons/plus.svg",
    Minus => "../../../assets/icons/minus.svg",
    Close => "../../../assets/icons/x.svg",
    Check => "../../../assets/icons/check.svg",
    Done => "../../../assets/icons/circle-check.svg",
    Sort => "../../../assets/icons/arrow-up-down.svg",
    File => "../../../assets/icons/file-text.svg",
    Files => "../../../assets/icons/files.svg",
    FolderOpen => "../../../assets/icons/folder-open.svg",
    ZoomIn => "../../../assets/icons/zoom-in.svg",
    ZoomOut => "../../../assets/icons/zoom-out.svg",
    Fit => "../../../assets/icons/maximize.svg",
    Alert => "../../../assets/icons/triangle-alert.svg",
    Scissors => "../../../assets/icons/scissors.svg",
    Hash => "../../../assets/icons/list-ordered.svg",
    Type => "../../../assets/icons/type.svg",
    Gauge => "../../../assets/icons/gauge.svg",
    Clock => "../../../assets/icons/clock.svg",
    Sparkles => "../../../assets/icons/sparkles.svg",
    Refresh => "../../../assets/icons/rotate-cw.svg",
    Info => "../../../assets/icons/info.svg",
    Monitor => "../../../assets/icons/monitor.svg",
    Trash => "../../../assets/icons/trash-2.svg",
}

impl Icon {
    /// O ícone como imagem, com o traço branco para poder ser tingido.
    pub fn image(self, size: f32, color: Color32) -> egui::Image<'static> {
        static CACHE: OnceLock<Mutex<HashMap<Icon, Arc<[u8]>>>> = OnceLock::new();
        let bytes = {
            let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
            cache.entry(self).or_insert_with(|| self.svg().replace("currentColor", "white").replace("stroke-width=\"2\"", "stroke-width=\"1.85\"").into_bytes().into()).clone()
        };
        egui::Image::from_bytes(format!("bytes://pdfpack/{self:?}.svg"), egui::load::Bytes::Shared(bytes)).fit_to_exact_size(vec2(size, size)).tint(color)
    }

    pub fn paint(self, ui: &Ui, center: Pos2, size: f32, color: Color32) {
        self.image(size, color).paint_at(ui, Rect::from_center_size(center, vec2(size, size)));
    }
}
