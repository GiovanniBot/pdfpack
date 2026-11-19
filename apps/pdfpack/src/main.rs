//! PDFPack: comprime PDFs e os organiza em pastas "Email N" que cabem no limite de anexos.
//!
//! Feito sobre o motor do PDFCraft (otimizador, divisão de páginas e renderizador).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod donate;
mod donate_data;
mod icons;
mod install;
mod list;
mod modals;
mod pick;
mod render;
mod state;
mod theme;
mod update;
mod viewer;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use state::{Doc, Msg, Prefs, Progress, Snapshot, Sort};

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().any(|a| a == name);
    if flag("--version") {
        // `println!` entraria em pânico se quem chamou já fechou a saída.
        let _ = std::io::Write::write_all(&mut std::io::stdout(), format!("{}\n", update::VERSION).as_bytes());
        return Ok(());
    }
    // Instalar e desinstalar sem a janela: é o que "Aplicativos instalados" do Windows chama, e
    // serve para quem distribui o PDFPack por script.
    if flag("--install") {
        std::process::exit(match install::install(!flag("--no-desktop")) {
            Ok(_) => 0,
            Err(_) => 1,
        });
    }
    if flag("--uninstall") {
        std::process::exit(uninstall_from_command_line(flag("--quiet")));
    }
    update::clean_up();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("PDFPack")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_icon(theme::icon())
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("PDFPack", options, Box::new(|cc| Ok(Box::new(App::new(&cc.egui_ctx)))))
}

/// `--uninstall`: confirma (a não ser com `--quiet`) e desinstala. Devolve o código de saída.
fn uninstall_from_command_line(quiet: bool) -> i32 {
    let tell = |text: &str| {
        if !quiet {
            rfd::MessageDialog::new().set_title("PDFPack").set_description(text).set_buttons(rfd::MessageButtons::Ok).show();
        }
    };
    if !quiet {
        let answer = rfd::MessageDialog::new()
            .set_title("PDFPack")
            .set_description(
                "Desinstalar o PDFPack deste computador?\n\nOs atalhos, o programa e as preferências são removidos. Seus PDFs não são tocados.",
            )
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if answer != rfd::MessageDialogResult::Yes {
            return 0;
        }
    }
    match install::uninstall() {
        Ok(()) => {
            tell("O PDFPack foi desinstalado.");
            0
        }
        Err(e) => {
            tell(&e);
            1
        }
    }
}

/// Instalar-se e atualizar-se sozinho só valem no Windows, e não em versões de desenvolvimento: a
/// release publica apenas o executável do Windows, e baixá-lo por cima de um programa de outro
/// sistema o estragaria.
pub const SELF_MANAGED: bool = cfg!(windows) && !cfg!(debug_assertions);

/// Um PDF que passava do limite do email e o que organizar fez com ele.
pub struct Reshaped {
    pub name: String,
    pub before: u64,
    pub after: u64,
    /// Em quantos arquivos ficou; 1 quando coube inteiro depois de regravado.
    pub parts: usize,
    pub pages: usize,
    /// Se alguma parte ainda passa do limite (uma página sozinha grande demais).
    pub over: bool,
}

pub enum Modal {
    None,
    /// Na primeira abertura fora da pasta de instalação; `desktop` é a caixa do atalho.
    InstallOffer {
        desktop: bool,
    },
    UninstallConfirm,
    CompressSetup,
    Compressing(Arc<Progress>),
    CompressDone {
        before: u64,
        after: u64,
        count: usize,
        failed: Vec<(String, String)>,
    },
    OrganizeSetup,
    Organizing(Arc<Progress>),
    /// O que organizar fez com os PDFs que passavam do limite, para ninguém ter de conferir.
    OrganizeDone {
        emails: usize,
        pdfs: usize,
        limit: u64,
        changed: Vec<Reshaped>,
    },
    Saving(Arc<Progress>),
    /// A doação pelo Pix: o valor escolhido e quando o código foi copiado.
    Donate {
        tier: usize,
        copied: Option<f64>,
    },
    Message {
        title: String,
        lines: Vec<String>,
    },
    /// Uma versão nova chegou; `always` é a escolha marcada (atualizar sozinho daqui para frente).
    UpdateOffer {
        release: update::Release,
        always: bool,
    },
    Updating(Arc<Progress>),
    UpdateDone {
        version: String,
    },
}

pub enum ToastAction {
    None,
    Folder(PathBuf),
    Restart,
}

pub struct Toast {
    pub text: String,
    pub action: ToastAction,
    pub until: f64,
}

pub struct App {
    pub state: Snapshot,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// O PDF mostrado no visualizador.
    pub selected: Option<u64>,
    pub collapsed: HashSet<usize>,
    pub prefs: Prefs,
    pub modal: Modal,
    pub toast: Option<Toast>,
    pub loading: usize,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pub renderer: render::Renderer,
    pub textures: HashMap<render::Key, egui::TextureHandle>,
    pub failed: HashSet<render::Key>,
    pub wanted: Vec<render::Job>,
    /// As miniaturas que o minimapa mostrou no último quadro (primeira e última página).
    pub thumbs_shown: Option<(usize, usize)>,
    /// A largura em pixels pedida para cada página perto da tela.
    pub desired: HashMap<usize, u32>,
    pub zoom: f32,
    /// O olho do visualizador: mostra o PDF como foi aberto, para comparar com o comprimido.
    pub show_original: bool,
    pub view: viewer::View,
    pub current_page: usize,
    pub scroll_to: Option<usize>,
    /// Quando a lista foi organizada em emails, para a animação de entrada.
    pub organized_at: Option<f64>,
    /// As linhas selecionadas na lista. `selected` é a que o visualizador mostra.
    pub picked: HashSet<u64>,
    /// De onde um Shift+clique estende a seleção.
    pub anchor: Option<u64>,
    pub drag: Option<list::Drag>,
    pub marquee: Option<list::Marquee>,
    /// As animações da lista durante um arrasto (linhas que somem, espaço que abre).
    pub list_anim: HashMap<(u8, u64), f32>,
    pub limit_text: f64,
    /// Uma versão nova já encontrada, à espera de um momento sem diálogo aberto.
    pending_update: Option<update::Release>,
    /// A atualização em andamento é silenciosa (o usuário escolheu atualizar sempre sozinho).
    silent_update: bool,
}

impl App {
    fn new(ctx: &egui::Context) -> Self {
        theme::install(ctx);
        let (tx, rx) = channel();
        let prefs = Prefs::load();
        let mut app = Self {
            state: Snapshot { docs: Vec::new(), organized: None, sort: Sort::Numeracao },
            undo: Vec::new(),
            redo: Vec::new(),
            selected: None,
            collapsed: HashSet::new(),
            prefs,
            modal: Modal::None,
            toast: None,
            loading: 0,
            tx,
            rx,
            renderer: render::Renderer::new(ctx),
            textures: HashMap::new(),
            failed: HashSet::new(),
            wanted: Vec::new(),
            thumbs_shown: None,
            desired: HashMap::new(),
            zoom: 1.0,
            show_original: false,
            view: viewer::View::default(),
            current_page: 0,
            scroll_to: None,
            organized_at: None,
            picked: HashSet::new(),
            anchor: None,
            drag: None,
            marquee: None,
            list_anim: HashMap::new(),
            limit_text: prefs.email_limit_mb,
            pending_update: None,
            silent_update: false,
        };
        // Quem abriu o arquivo baixado é perguntado, uma vez, se quer instalar.
        if SELF_MANAGED && !app.prefs.install_asked && !install::is_installed() {
            app.modal = Modal::InstallOffer { desktop: true };
        }
        // Versões de desenvolvimento não se atualizam sozinhas.
        if SELF_MANAGED {
            state::spawn_update_check(false, app.tx.clone(), ctx.clone());
        }
        // PDFs passados na linha de comando ("Abrir com", ou soltos sobre o executável).
        app.open_paths(std::env::args_os().skip(1).map(PathBuf::from).collect(), ctx);
        app
    }

    pub fn busy(&self) -> bool {
        !matches!(self.modal, Modal::None)
    }

    pub fn selected_doc(&self) -> Option<&Doc> {
        self.state.docs.iter().find(|d| Some(d.id) == self.selected)
    }

    /// Guarda o estado atual antes de uma mudança, para desfazer.
    pub fn checkpoint(&mut self) {
        self.undo.push(self.state.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() && !self.busy()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty() && !self.busy()
    }

    pub fn undo(&mut self) {
        if !self.can_undo() {
            return;
        }
        if let Some(prev) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.state, prev));
            self.after_history();
        }
    }

    pub fn redo(&mut self) {
        if !self.can_redo() {
            return;
        }
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.state, next));
            self.after_history();
        }
    }

    fn after_history(&mut self) {
        self.collapsed.clear();
        self.organized_at = None;
        self.fix_selection();
    }

    /// Mantém um PDF selecionado enquanto houver algum.
    pub fn fix_selection(&mut self) {
        let ids: HashSet<u64> = self.state.docs.iter().map(|d| d.id).collect();
        self.picked.retain(|id| ids.contains(id));
        self.anchor = self.anchor.filter(|id| ids.contains(id));
        if self.selected_doc().is_none() {
            // O PDF aberto saiu da lista: abre outro da seleção, ou o primeiro da lista.
            let next = self.state.docs.iter().find(|d| self.picked.contains(&d.id)).or(self.state.docs.first()).map(|d| d.id);
            self.select(next);
            if self.picked.is_empty() {
                self.picked = next.into_iter().collect();
                self.anchor = next;
            }
        }
    }

    /// Um clique numa linha da lista, com as teclas que estavam apertadas.
    pub fn pick(&mut self, id: u64, ctrl: bool, shift: bool) {
        let order: Vec<u64> = self.state.docs.iter().map(|d| d.id).collect();
        let (picked, anchor) = pick::click(&order, &self.picked, self.anchor, id, ctrl, shift);
        self.picked = picked;
        self.anchor = anchor;
        if self.picked.contains(&id) {
            self.select(Some(id));
        } else {
            self.show_picked();
        }
    }

    /// A seleção volta a ser só o PDF aberto no visualizador.
    pub fn pick_shown(&mut self) {
        self.picked = self.selected.into_iter().collect();
        self.anchor = self.selected;
    }

    /// Se o PDF aberto ficou fora da seleção, abre o primeiro dela.
    pub fn show_picked(&mut self) {
        if self.selected.is_some_and(|id| self.picked.contains(&id)) {
            return;
        }
        if let Some(first) = self.state.docs.iter().find(|d| self.picked.contains(&d.id)).map(|d| d.id) {
            self.select(Some(first));
            self.anchor = Some(first);
        }
    }

    pub fn pick_all(&mut self) {
        self.picked = self.state.docs.iter().map(|d| d.id).collect();
    }

    /// Tira da lista tudo o que está selecionado, em um passo só do desfazer.
    pub fn remove_picked(&mut self) {
        if !self.state.docs.iter().any(|d| self.picked.contains(&d.id)) {
            return;
        }
        self.checkpoint();
        let picked = std::mem::take(&mut self.picked);
        self.state.docs.retain(|d| !picked.contains(&d.id));
        self.fix_selection();
    }

    /// Larga um arrasto ou um retângulo de seleção pela metade, sem mover nada.
    pub fn cancel_gesture(&mut self) -> bool {
        let any = self.drag.is_some() || self.marquee.is_some();
        self.drag = None;
        self.marquee = None;
        self.list_anim.clear();
        any
    }

    pub fn select(&mut self, id: Option<u64>) {
        if self.selected != id {
            self.selected = id;
            self.current_page = 0;
            self.scroll_to = Some(0);
        }
    }

    pub fn pick_files(&mut self, ctx: &egui::Context) {
        if let Some(paths) = rfd::FileDialog::new().set_title("Selecionar PDFs").add_filter("PDF", &["pdf"]).pick_files() {
            self.open_paths(paths, ctx);
        }
    }

    pub fn open_paths(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let mut files = Vec::new();
        for path in paths {
            if path.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&path) {
                    files.extend(entries.flatten().map(|e| e.path()).filter(|p| is_pdf(p)));
                }
            } else if is_pdf(&path) {
                files.push(path);
            }
        }
        if files.is_empty() {
            return;
        }
        self.loading += 1;
        state::spawn_load(files, self.tx.clone(), ctx.clone());
    }

    pub fn start_compress(&mut self, ctx: &egui::Context) {
        let jobs: Vec<_> = self.state.docs.iter().filter(|d| d.checked).map(|d| (d.id, d.name.clone(), d.source.clone())).collect();
        if jobs.is_empty() {
            return;
        }
        self.prefs.save();
        let progress = Arc::new(Progress::default());
        state::spawn_compress(jobs, self.prefs.level, progress.clone(), self.tx.clone(), ctx.clone());
        self.modal = Modal::Compressing(progress);
    }

    pub fn start_organize(&mut self, ctx: &egui::Context) {
        self.prefs.email_limit_mb = self.limit_text.clamp(0.5, 2000.0);
        self.prefs.save();
        let limit = (self.prefs.email_limit_mb * 1024.0 * 1024.0) as u64;
        let big: Vec<Doc> = self.state.docs.iter().filter(|d| d.size() > limit && d.pages.len() > 1).cloned().collect();
        if big.is_empty() {
            self.apply_organize(limit, Vec::new(), ctx);
            return;
        }
        let progress = Arc::new(Progress::default());
        state::spawn_split(big, limit, progress.clone(), self.tx.clone(), ctx.clone());
        self.modal = Modal::Organizing(progress);
    }

    fn apply_organize(&mut self, limit: u64, split: Vec<state::SplitDoc>, ctx: &egui::Context) {
        let mut split: HashMap<u64, Vec<Doc>> = split.into_iter().map(|s| (s.id, s.parts)).collect();
        // Nenhuma página pode sumir: se a soma das partes não bate, a lista fica como estava.
        let lost = self
            .state
            .docs
            .iter()
            .find(|d| split.get(&d.id).is_some_and(|parts| parts.iter().map(|p| p.pages.len()).sum::<usize>() != d.pages.len()));
        if let Some(doc) = lost {
            self.modal = Modal::Message {
                title: "Não foi possível organizar".into(),
                lines: vec![format!("{}: a divisão não manteve todas as páginas. Nada foi alterado.", doc.name)],
            };
            return;
        }
        self.checkpoint();
        let mut changed = Vec::new();
        let old = std::mem::take(&mut self.state.docs);
        for doc in old {
            match split.remove(&doc.id) {
                Some(parts) => {
                    changed.push(Reshaped {
                        name: doc.name.clone(),
                        before: doc.size(),
                        after: parts.iter().map(Doc::size).sum(),
                        parts: parts.len(),
                        pages: doc.pages.len(),
                        over: parts.iter().any(|p| p.error.is_some()),
                    });
                    self.state.docs.extend(parts);
                }
                None => self.state.docs.push(doc),
            }
        }
        if self.state.sort != Sort::Manual && !self.state.docs.is_empty() {
            // Partes de um PDF dividido ficam juntas e em ordem; a ordem da lista já é a final.
            self.state.sort = Sort::Manual;
        }
        self.state.organized = Some(limit);
        self.collapsed.clear();
        self.organized_at = Some(ctx.input(|i| i.time));
        // Se algum PDF foi regravado ou dividido, o resumo diz o que aconteceu com ele.
        self.modal = if changed.is_empty() {
            Modal::None
        } else {
            let emails = self.state.emails().map_or(0, |e| e.len());
            Modal::OrganizeDone { emails, pdfs: self.state.docs.len(), limit, changed }
        };
        self.fix_selection();
    }

    pub fn start_save(&mut self, ctx: &egui::Context) {
        if self.state.docs.is_empty() || self.busy() {
            return;
        }
        let Some(dir) = rfd::FileDialog::new().set_title("Salvar em").pick_folder() else { return };
        let progress = Arc::new(Progress::default());
        state::spawn_save(self.state.clone(), dir, progress.clone(), self.tx.clone(), ctx.clone());
        self.modal = Modal::Saving(progress);
    }

    pub fn remove(&mut self, id: u64) {
        self.checkpoint();
        self.state.docs.retain(|d| d.id != id);
        self.fix_selection();
    }

    pub fn sort_by(&mut self, sort: Sort) {
        self.checkpoint();
        self.state.sort = sort;
        self.state.sort_docs();
    }

    /// Move os PDFs `ids`, juntos e na ordem em que estão, para antes da posição `to` da lista.
    pub fn move_docs(&mut self, ids: &HashSet<u64>, to: usize) {
        let order: Vec<u64> = self.state.docs.iter().map(|d| d.id).collect();
        let moved = pick::moved(&order, ids, to);
        if moved == order {
            return;
        }
        self.checkpoint();
        let mut docs: HashMap<u64, Doc> = std::mem::take(&mut self.state.docs).into_iter().map(|d| (d.id, d)).collect();
        self.state.docs = moved.iter().filter_map(|id| docs.remove(id)).collect();
        self.state.sort = Sort::Manual;
    }

    fn on_message(&mut self, msg: Msg, ctx: &egui::Context) {
        match msg {
            Msg::Loaded { docs, errors } => {
                self.loading = self.loading.saturating_sub(1);
                if !docs.is_empty() {
                    self.checkpoint();
                    self.state.docs.extend(docs);
                    self.state.sort_docs();
                    self.fix_selection();
                }
                if !errors.is_empty() && !self.busy() {
                    self.modal = Modal::Message {
                        title: "Alguns arquivos não puderam ser abertos".into(),
                        lines: errors.into_iter().map(|(name, e)| format!("{name}: {e}")).collect(),
                    };
                }
            }
            Msg::Compressed { results, cancelled } => {
                if cancelled {
                    self.modal = Modal::None;
                    return;
                }
                self.checkpoint();
                // O resultado novo aparece na tela, não o original de uma comparação anterior.
                self.show_original = false;
                let (mut before, mut after, mut count, mut failed) = (0, 0, 0, Vec::new());
                for (id, result) in results {
                    let Some(doc) = self.state.docs.iter_mut().find(|d| d.id == id) else { continue };
                    match result {
                        Ok(bytes) => {
                            before += doc.base_size;
                            // Um resultado maior que a origem não serve: fica a origem.
                            doc.current = if bytes.len() < doc.source.len() { bytes } else { doc.source.clone() };
                            doc.version = state::next_id();
                            doc.compressed = true;
                            after += doc.size().min(doc.base_size);
                            count += 1;
                        }
                        Err(e) => failed.push((doc.name.clone(), e)),
                    }
                }
                if self.state.sort == Sort::Tamanho {
                    self.state.sort_docs();
                }
                self.modal = Modal::CompressDone { before, after, count, failed };
            }
            Msg::Split { limit, result } => match result {
                Ok(split) => self.apply_organize(limit, split, ctx),
                Err(e) if e == "cancelado" => self.modal = Modal::None,
                Err(e) => self.modal = Modal::Message { title: "Não foi possível dividir".into(), lines: vec![e] },
            },
            Msg::Saved(result) => match result {
                Ok((dir, n)) => {
                    self.modal = Modal::None;
                    let what = match self.state.emails() {
                        Some(emails) => format!("{n} PDFs salvos em {} pastas de email", emails.len()),
                        None => format!("{n} PDFs salvos"),
                    };
                    self.toast = Some(Toast { text: what, action: ToastAction::Folder(dir), until: ctx.input(|i| i.time) + 8.0 });
                }
                Err(e) => self.modal = Modal::Message { title: "Não foi possível salvar".into(), lines: vec![e] },
            },
            Msg::UpdateChecked { release, manual } => match release {
                Ok(Some(release)) => self.pending_update = Some(release),
                Ok(None) if manual => self.say(format!("Você já está na versão mais recente ({})", update::VERSION), ctx),
                Err(e) if manual => self.modal = Modal::Message { title: "Não foi possível procurar atualizações".into(), lines: vec![e] },
                _ => {}
            },
            Msg::Updated(result) => {
                let silent = std::mem::take(&mut self.silent_update);
                match result {
                    Ok(version) if silent => {
                        let text = format!("PDFPack {version} instalado · vale ao reabrir");
                        self.toast = Some(Toast { text, action: ToastAction::Restart, until: ctx.input(|i| i.time) + 20.0 });
                    }
                    Ok(version) => self.modal = Modal::UpdateDone { version },
                    // Uma atualização silenciosa que falha tenta de novo na próxima abertura.
                    Err(_) if silent => {}
                    Err(e) if e == "cancelado" => self.modal = Modal::None,
                    Err(e) => self.modal = Modal::Message { title: "Não foi possível atualizar".into(), lines: vec![e] },
                }
            }
        }
    }

    fn say(&mut self, text: String, ctx: &egui::Context) {
        self.toast = Some(Toast { text, action: ToastAction::None, until: ctx.input(|i| i.time) + 6.0 });
    }

    pub fn check_updates(&mut self, ctx: &egui::Context) {
        if !cfg!(windows) {
            return self.say("As atualizações automáticas só existem no Windows".into(), ctx);
        }
        state::spawn_update_check(true, self.tx.clone(), ctx.clone());
    }

    /// O que fazer com uma versão nova: instalar em silêncio se o usuário já escolheu isso, ou
    /// perguntar (na primeira vez, também se quer que seja sempre assim).
    fn offer_update(&mut self, ctx: &egui::Context) {
        if self.busy() || self.silent_update {
            return;
        }
        let Some(release) = self.pending_update.take() else { return };
        if self.prefs.auto_update == Some(true) {
            self.silent_update = true;
            state::spawn_update(release, Arc::new(Progress::default()), self.tx.clone(), ctx.clone());
        } else {
            self.modal = Modal::UpdateOffer { release, always: self.prefs.auto_update.is_none() };
        }
    }

    /// O usuário aceitou a atualização; `always` é a resposta à pergunta da primeira vez.
    pub fn start_update(&mut self, release: update::Release, always: Option<bool>, ctx: &egui::Context) {
        if let Some(always) = always {
            self.prefs.auto_update = Some(always);
            self.prefs.save();
        }
        let progress = Arc::new(Progress::default());
        state::spawn_update(release, progress.clone(), self.tx.clone(), ctx.clone());
        self.modal = Modal::Updating(progress);
    }

    /// A resposta à pergunta de instalação não se repete, qualquer que tenha sido.
    pub fn install_later(&mut self) {
        self.prefs.install_asked = true;
        self.prefs.save();
        self.modal = Modal::None;
    }

    /// Instala e, se não há nada aberto para perder, reabre já pela cópia instalada.
    pub fn install(&mut self, desktop: bool, ctx: &egui::Context) {
        self.prefs.install_asked = true;
        self.prefs.save();
        self.modal = Modal::None;
        match install::install(desktop) {
            Ok(exe) if self.state.docs.is_empty() && self.loading == 0 => match install::launch(&exe) {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(_) => self.say("PDFPack instalado · abra pelo atalho da próxima vez".into(), ctx),
            },
            Ok(_) => self.say("PDFPack instalado · abra pelo atalho da próxima vez".into(), ctx),
            Err(e) => self.modal = Modal::Message { title: "Não foi possível instalar".into(), lines: vec![e] },
        }
    }

    pub fn uninstall(&mut self, ctx: &egui::Context) {
        match install::uninstall() {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => self.modal = Modal::Message { title: "Não foi possível desinstalar".into(), lines: vec![e] },
        }
    }

    pub fn restart(&mut self, ctx: &egui::Context) {
        match update::restart() {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => self.modal = Modal::Message { title: "Não foi possível reiniciar".into(), lines: vec![e] },
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            self.open_paths(dropped, ctx);
        }
        if self.busy() {
            return;
        }
        let pressed = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        if pressed(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || pressed(Modifiers::COMMAND, Key::Y) {
            self.redo();
        } else if pressed(Modifiers::COMMAND, Key::Z) {
            self.undo();
        }
        if pressed(Modifiers::COMMAND, Key::S) {
            self.start_save(ctx);
        }
        if pressed(Modifiers::COMMAND, Key::O) {
            self.pick_files(ctx);
        }
        // Ctrl + e Ctrl − ampliam o documento, não a interface (veja `theme::install`). O teclado
        // numérico chega como as mesmas teclas.
        if self.selected_doc().is_some() {
            if pressed(Modifiers::COMMAND, Key::Num0) {
                viewer::set_zoom(self, 1.0, None);
            }
            if pressed(Modifiers::COMMAND, Key::Plus) || pressed(Modifiers::COMMAND, Key::Equals) {
                viewer::set_zoom(self, self.zoom * viewer::ZOOM_STEP, None);
            }
            if pressed(Modifiers::COMMAND, Key::Minus) {
                viewer::set_zoom(self, self.zoom / viewer::ZOOM_STEP, None);
            }
            // A barra invertida alterna entre o original e o comprimido, como o antes e depois dos
            // editores de imagem.
            if pressed(Modifiers::NONE, Key::Backslash) {
                viewer::toggle_original(self);
            }
        }
        if pressed(Modifiers::NONE, Key::Delete) {
            self.remove_picked();
        }
        if pressed(Modifiers::COMMAND, Key::A) {
            self.pick_all();
        }
        // Esc larga um arrasto pela metade; sem arrasto, desfaz a seleção múltipla.
        if ctx.input(|i| i.key_pressed(Key::Escape)) && !self.cancel_gesture() && self.picked.len() > 1 {
            self.pick_shown();
        }
    }

    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            self.on_message(msg, ctx);
        }
        self.offer_update(ctx);
        for done in self.renderer.poll() {
            match done.image {
                Some(image) => {
                    let texture = ctx.load_texture(format!("{:?}", done.key), image, egui::TextureOptions::LINEAR);
                    self.textures.insert(done.key, texture);
                }
                None => {
                    self.failed.insert(done.key);
                }
            }
        }
    }
}

fn is_pdf(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receive(&ctx);
        self.shortcuts(&ctx);
        self.wanted.clear();

        let line = egui::Stroke::new(1.0, theme::BORDER);
        let bar = egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::symmetric(14, 10));
        let top = egui::Panel::top("toolbar").frame(bar).show_separator_line(false).show(ui, |ui| list::toolbar(self, ui));
        ui.painter().hline(top.response.rect.x_range(), top.response.rect.max.y - 0.5, line);
        if self.state.docs.is_empty() {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BG)).show(ui, |ui| list::empty(self, ui));
        } else {
            let side = egui::Frame::new().fill(theme::PANEL).inner_margin(egui::Margin::same(10));
            let left = egui::Panel::left("list")
                .frame(side)
                .default_size(340.0)
                .size_range(280.0..=520.0)
                .show_separator_line(false)
                .show(ui, |ui| list::panel(self, ui));
            let right = egui::Panel::right("minimap")
                .frame(side)
                .exact_size(160.0)
                .resizable(false)
                .show_separator_line(false)
                .show(ui, |ui| viewer::minimap(self, ui));
            egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::VIEWER)).show(ui, |ui| viewer::pages(self, ui));
            ui.painter().vline(left.response.rect.max.x - 0.5, left.response.rect.y_range(), line);
            ui.painter().vline(right.response.rect.min.x + 0.5, right.response.rect.y_range(), line);
        }
        modals::show(self, &ctx);

        viewer::evict(self);
        self.renderer.want(std::mem::take(&mut self.wanted));
    }
}
