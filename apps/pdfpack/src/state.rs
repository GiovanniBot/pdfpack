//! A lista de PDFs, seu histórico (desfazer/refazer) e o trabalho feito fora da interface.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::core::{self, Bytes, Level};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone)]
pub struct Doc {
    pub id: u64,
    pub name: String,
    pub path: Option<PathBuf>,
    pub modified: Option<SystemTime>,
    /// O arquivo como foi aberto (ou dividido): toda compressão parte dele.
    pub source: Bytes,
    /// O arquivo como será salvo.
    pub current: Bytes,
    /// Muda quando `current` muda, para o visualizador redesenhar.
    pub version: u64,
    /// Largura e altura de cada página, em pontos.
    pub pages: Arc<Vec<[f32; 2]>>,
    pub checked: bool,
    pub compressed: bool,
    /// Tamanho antes de qualquer compressão, para mostrar a economia.
    pub base_size: u64,
    /// Por que este arquivo passa do limite do email mesmo depois de dividido, se passa.
    pub error: Option<String>,
}

impl Doc {
    pub fn size(&self) -> u64 {
        self.current.len() as u64
    }

    /// Quanto do tamanho original foi economizado, de 0 a 100.
    pub fn saved_percent(&self) -> u32 {
        if self.base_size == 0 || self.size() >= self.base_size {
            return 0;
        }
        (100.0 * (1.0 - self.size() as f64 / self.base_size as f64)).round() as u32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sort {
    Numeracao,
    Nome,
    Tamanho,
    Data,
    Manual,
}

impl Sort {
    pub const MENU: [Sort; 4] = [Sort::Numeracao, Sort::Nome, Sort::Tamanho, Sort::Data];

    pub fn name(self) -> &'static str {
        match self {
            Sort::Numeracao => "Numeração",
            Sort::Nome => "Nome",
            Sort::Tamanho => "Tamanho",
            Sort::Data => "Data",
            Sort::Manual => "Manual",
        }
    }
}

/// Tudo o que desfazer e refazer restauram. Barato de copiar: os arquivos são compartilhados.
#[derive(Clone)]
pub struct Snapshot {
    pub docs: Vec<Doc>,
    /// O limite por email, em bytes, quando a lista está organizada em emails.
    pub organized: Option<u64>,
    pub sort: Sort,
}

impl Snapshot {
    pub fn sort_docs(&mut self) {
        match self.sort {
            Sort::Numeracao => self.docs.sort_by(|a, b| core::default_cmp(&a.name, &b.name)),
            Sort::Nome => self.docs.sort_by(|a, b| core::natural_cmp(&a.name, &b.name)),
            Sort::Tamanho => self.docs.sort_by(|a, b| b.size().cmp(&a.size()).then_with(|| core::natural_cmp(&a.name, &b.name))),
            Sort::Data => self.docs.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| core::natural_cmp(&a.name, &b.name))),
            Sort::Manual => {}
        }
    }

    /// Os emails (índices em `docs`), quando organizado.
    pub fn emails(&self) -> Option<Vec<Vec<usize>>> {
        let limit = self.organized?;
        Some(core::pack(&self.docs.iter().map(Doc::size).collect::<Vec<_>>(), limit))
    }

    pub fn total(&self) -> u64 {
        self.docs.iter().map(Doc::size).sum()
    }

    pub fn any_compressed(&self) -> bool {
        self.docs.iter().any(|d| d.compressed)
    }
}

/// O andamento de um trabalho longo, lido pela interface a cada quadro.
#[derive(Default)]
pub struct Progress {
    fraction: AtomicU32,
    pub cancel: AtomicBool,
    pub label: Mutex<String>,
}

impl Progress {
    pub fn set(&self, f: f32) {
        self.fraction.store(f.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.fraction.load(Ordering::Relaxed))
    }

    pub fn label(&self) -> String {
        self.label.lock().map(|l| l.clone()).unwrap_or_default()
    }

    fn set_label(&self, text: String) {
        if let Ok(mut l) = self.label.lock() {
            *l = text;
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

pub struct SplitDoc {
    pub id: u64,
    pub parts: Vec<Doc>,
}

pub enum Msg {
    Loaded {
        docs: Vec<Doc>,
        errors: Vec<(String, String)>,
    },
    /// Para cada arquivo: o resultado comprimido, ou por que não deu.
    Compressed {
        results: Vec<(u64, Result<Bytes, String>)>,
        cancelled: bool,
    },
    Split {
        limit: u64,
        result: Result<Vec<SplitDoc>, String>,
    },
    Saved(Result<(PathBuf, usize), String>),
    /// O resultado de procurar uma versão nova; `manual` quando foi o usuário que pediu.
    UpdateChecked {
        release: Result<Option<crate::update::Release>, String>,
        manual: bool,
    },
    /// A versão instalada, pronta para a próxima abertura.
    Updated(Result<String, String>),
}

fn panic_text(e: Box<dyn std::any::Any + Send>) -> String {
    e.downcast_ref::<&str>().map(|s| (*s).to_owned()).or_else(|| e.downcast_ref::<String>().cloned()).unwrap_or_else(|| "erro inesperado".into())
}

fn open_error(e: &pdfcraft_render::OpenError) -> String {
    match e {
        pdfcraft_render::OpenError::NeedsPassword | pdfcraft_render::OpenError::WrongPassword => "protegido por senha".into(),
        pdfcraft_render::OpenError::Invalid(_) => "não é um PDF legível".into(),
        pdfcraft_render::OpenError::Unsupported(s) => s.clone(),
    }
}

fn page_sizes(bytes: &Bytes) -> Result<Vec<[f32; 2]>, String> {
    let info = pdfcraft_render::inspect(bytes.clone(), None).map_err(|e| open_error(&e))?;
    if info.pages.is_empty() {
        return Err("não tem páginas".into());
    }
    Ok(info.pages.iter().map(|p| [p.width.max(1.0), p.height.max(1.0)]).collect())
}

fn load_one(path: &Path) -> Result<Doc, String> {
    let bytes: Bytes = Arc::new(std::fs::read(path).map_err(|e| e.to_string())?);
    let pages = std::panic::catch_unwind(|| page_sizes(&bytes)).map_err(panic_text)??;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "documento.pdf".into());
    Ok(Doc {
        id: next_id(),
        name,
        path: Some(path.to_path_buf()),
        modified: std::fs::metadata(path).and_then(|m| m.modified()).ok(),
        base_size: bytes.len() as u64,
        source: bytes.clone(),
        current: bytes,
        version: next_id(),
        pages: Arc::new(pages),
        checked: true,
        compressed: false,
        error: None,
    })
}

pub fn spawn_load(paths: Vec<PathBuf>, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let (mut docs, mut errors) = (Vec::new(), Vec::new());
        for path in paths {
            match load_one(&path) {
                Ok(doc) => docs.push(doc),
                Err(e) => errors.push((path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), e)),
            }
        }
        let _ = tx.send(Msg::Loaded { docs, errors });
        ctx.request_repaint();
    });
}

pub fn spawn_compress(jobs: Vec<(u64, String, Bytes)>, level: Level, progress: Arc<Progress>, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let total = jobs.len().max(1);
        let mut results = Vec::new();
        for (i, (id, name, bytes)) in jobs.into_iter().enumerate() {
            if progress.cancelled() {
                break;
            }
            progress.set_label(format!("{} de {} · {}", i + 1, total, name));
            let mut tick = |f: f32| {
                progress.set((i as f32 + f) / total as f32);
                ctx.request_repaint();
                !progress.cancelled()
            };
            let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| core::compress(&bytes, level, &mut tick)))
                .unwrap_or_else(|e| Err(panic_text(e)));
            if progress.cancelled() {
                break;
            }
            results.push((id, out.map(Arc::new)));
        }
        progress.set(1.0);
        let _ = tx.send(Msg::Compressed { results, cancelled: progress.cancelled() });
        ctx.request_repaint();
    });
}

fn part_name(name: &str, i: usize, n: usize) -> String {
    let stem = name.strip_suffix(".pdf").or_else(|| name.strip_suffix(".PDF")).unwrap_or(name);
    format!("{stem} (parte {} de {n}).pdf", i + 1)
}

fn split_one(doc: &Doc, limit: u64, tick: &mut dyn FnMut(f32) -> bool) -> Result<Vec<Doc>, String> {
    let parts = core::split_to_fit(&doc.current, limit, tick)?;
    let n = parts.len();
    // A economia de cada parte é a do arquivo inteiro: a compressão foi feita antes de dividir.
    let ratio = doc.base_size as f64 / doc.size().max(1) as f64;
    Ok(parts
        .into_iter()
        .enumerate()
        .map(|(i, part)| {
            let bytes: Bytes = Arc::new(part.bytes);
            let over = bytes.len() as u64 > limit;
            Doc {
                id: next_id(),
                name: if n == 1 { doc.name.clone() } else { part_name(&doc.name, i, n) },
                path: None,
                modified: doc.modified,
                base_size: (bytes.len() as f64 * ratio) as u64,
                source: bytes.clone(),
                current: bytes,
                version: next_id(),
                pages: Arc::new(doc.pages[part.pages.start.min(doc.pages.len())..part.pages.end.min(doc.pages.len())].to_vec()),
                checked: doc.checked,
                compressed: doc.compressed,
                error: over.then(|| "uma única página já passa do limite".to_owned()),
            }
        })
        .collect())
}

/// Divide os PDFs maiores que o limite. A lista só muda quando todos deram certo.
pub fn spawn_split(docs: Vec<Doc>, limit: u64, progress: Arc<Progress>, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let total = docs.len().max(1);
        let mut out = Vec::new();
        let mut result = Ok(());
        for (i, doc) in docs.iter().enumerate() {
            progress.set_label(format!("Dividindo {}", doc.name));
            let mut tick = |f: f32| {
                progress.set((i as f32 + f) / total as f32);
                ctx.request_repaint();
                !progress.cancelled()
            };
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| split_one(doc, limit, &mut tick))).unwrap_or_else(|e| Err(panic_text(e))) {
                Ok(parts) => out.push(SplitDoc { id: doc.id, parts }),
                Err(e) => {
                    result = Err(if progress.cancelled() { "cancelado".to_owned() } else { format!("{}: {e}", doc.name) });
                    break;
                }
            }
        }
        let _ = tx.send(Msg::Split { limit, result: result.map(|()| out) });
        ctx.request_repaint();
    });
}

/// Um nome que ainda não foi usado na pasta de destino.
fn unique(used: &mut std::collections::HashSet<String>, name: &str) -> String {
    let stem = name.strip_suffix(".pdf").or_else(|| name.strip_suffix(".PDF")).unwrap_or(name);
    let mut candidate = format!("{stem}.pdf");
    let mut n = 2;
    while !used.insert(candidate.to_lowercase()) {
        candidate = format!("{stem} ({n}).pdf");
        n += 1;
    }
    candidate
}

fn save_all(snapshot: &Snapshot, dir: &Path, progress: &Progress) -> Result<usize, String> {
    let groups: Vec<(PathBuf, Vec<usize>)> = match snapshot.emails() {
        Some(emails) => emails.into_iter().enumerate().map(|(i, docs)| (dir.join(format!("Email {}", i + 1)), docs)).collect(),
        None => vec![(dir.to_path_buf(), (0..snapshot.docs.len()).collect())],
    };
    let total = snapshot.docs.len().max(1);
    let mut written = 0;
    for (folder, docs) in groups {
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
        let mut used = std::collections::HashSet::new();
        for i in docs {
            let doc = &snapshot.docs[i];
            let mut target = folder.join(unique(&mut used, &doc.name));
            // Nunca por cima do arquivo de origem: o original fica como estava.
            if doc.path.as_deref().is_some_and(|src| same_file(src, &target)) {
                let stem = doc.name.strip_suffix(".pdf").or_else(|| doc.name.strip_suffix(".PDF")).unwrap_or(&doc.name);
                target = folder.join(unique(&mut used, &format!("{stem} (comprimido).pdf")));
            }
            progress.set_label(doc.name.clone());
            std::fs::write(&target, &**doc.current).map_err(|e| format!("{}: {e}", target.display()))?;
            written += 1;
            progress.set(written as f32 / total as f32);
        }
    }
    Ok(written)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

pub fn spawn_save(snapshot: Snapshot, dir: PathBuf, progress: Arc<Progress>, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let result = save_all(&snapshot, &dir, &progress).map(|n| (dir, n));
        let _ = tx.send(Msg::Saved(result));
        ctx.request_repaint();
    });
}

/// As preferências que voltam na próxima vez que o aplicativo abre.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub email_limit_mb: f64,
    pub level: Level,
    /// Se as atualizações se instalam sozinhas. `None` até a primeira atualização chegar, quando
    /// o aplicativo pergunta.
    pub auto_update: Option<bool>,
    /// Se a pergunta "Instalar o PDFPack?" já foi respondida, para não repetir a cada abertura.
    pub install_asked: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self { email_limit_mb: 25.0, level: Level::Media, auto_update: None, install_asked: false }
    }
}

impl Prefs {
    fn file() -> Option<PathBuf> {
        Some(PathBuf::from(std::env::var_os("APPDATA")?).join("PDFPack").join("preferencias.json"))
    }

    pub fn load() -> Self {
        Self::file().and_then(|f| std::fs::read(f).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(file) = Self::file() else { return };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_vec_pretty(self) {
            let _ = std::fs::write(file, json);
        }
    }
}

pub fn spawn_update_check(manual: bool, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        let release = std::panic::catch_unwind(crate::update::check).unwrap_or_else(|e| Err(panic_text(e)));
        let _ = tx.send(Msg::UpdateChecked { release, manual });
        ctx.request_repaint();
    });
}

pub fn spawn_update(release: crate::update::Release, progress: Arc<Progress>, tx: Sender<Msg>, ctx: egui::Context) {
    std::thread::spawn(move || {
        progress.set_label(format!("Baixando a versão {}", release.version));
        let mut tick = |f: f32| {
            progress.set(f);
            ctx.request_repaint();
            !progress.cancelled()
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::update::install(&release, &mut tick)))
            .unwrap_or_else(|e| Err(panic_text(e)));
        let _ = tx.send(Msg::Updated(result.map(|()| release.version.clone())));
        ctx.request_repaint();
    });
}
