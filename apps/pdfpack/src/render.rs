//! Páginas e miniaturas desenhadas fora da thread da interface, com o renderizador do PDFCraft.
//!
//! A interface diz a cada quadro o que quer ver ([`Renderer::want`]); o que saiu da tela deixa a
//! fila sem ser desenhado, então rolar um PDF grande nunca acumula trabalho velho.

use std::collections::{HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use pdfcraft_render::{PageRenderer, RenderConfig, RenderRequest};

use crate::core::Bytes;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Thumb,
    /// Página no visualizador, com a largura em pixels.
    Page(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub doc: u64,
    pub version: u64,
    pub page: usize,
    pub kind: Kind,
}

pub struct Job {
    pub key: Key,
    pub bytes: Bytes,
    /// Pixels por ponto do PDF.
    pub scale: f32,
}

pub struct Rendered {
    pub key: Key,
    pub image: Option<egui::ColorImage>,
}

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    running: HashSet<Key>,
}

pub struct Renderer {
    shared: Arc<(Mutex<Queue>, Condvar)>,
    rx: Receiver<Rendered>,
}

impl Renderer {
    pub fn new(ctx: &egui::Context) -> Self {
        let shared = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let (tx, rx) = channel();
        let workers = std::thread::available_parallelism().map_or(2, |n| n.get().saturating_sub(1)).clamp(2, 4);
        for _ in 0..workers {
            let (shared, tx, ctx) = (shared.clone(), tx.clone(), ctx.clone());
            std::thread::spawn(move || work(&shared, &tx, &ctx));
        }
        Self { shared, rx }
    }

    /// O que deve ser desenhado agora, em ordem de prioridade. Substitui a fila anterior.
    pub fn want(&self, jobs: Vec<Job>) {
        let (lock, cond) = &*self.shared;
        let Ok(mut q) = lock.lock() else { return };
        let running = q.running.clone();
        q.jobs = jobs.into_iter().filter(|j| !running.contains(&j.key)).collect();
        if !q.jobs.is_empty() {
            cond.notify_all();
        }
    }

    pub fn poll(&self) -> Vec<Rendered> {
        self.rx.try_iter().collect()
    }
}

fn work(shared: &Arc<(Mutex<Queue>, Condvar)>, tx: &Sender<Rendered>, ctx: &egui::Context) {
    // Um documento aberto por vez em cada thread: trocar de PDF reabre, rolar não.
    let mut open: Option<((u64, u64), PageRenderer)> = None;
    loop {
        let job = {
            let (lock, cond) = &**shared;
            let Ok(mut q) = lock.lock() else { return };
            loop {
                if let Some(job) = q.jobs.pop_front() {
                    q.running.insert(job.key);
                    break job;
                }
                q = match cond.wait(q) {
                    Ok(q) => q,
                    Err(_) => return,
                };
            }
        };
        let id = (job.key.doc, job.key.version);
        if open.as_ref().is_none_or(|(k, _)| *k != id) {
            open = Some((id, PageRenderer::new(job.bytes.clone(), RenderConfig::default().on_screen())));
        }
        let image = open.as_mut().and_then(|(_, r)| {
            let page =
                r.render(RenderRequest { page: job.key.page, scale: job.scale, preview: job.key.kind == Kind::Thumb, ..RenderRequest::default() });
            if page.error.is_some() || page.width == 0 || page.height == 0 {
                return None;
            }
            Some(egui::ColorImage::from_rgba_premultiplied([page.width as usize, page.height as usize], &page.rgba))
        });
        if let Ok(mut q) = shared.0.lock() {
            q.running.remove(&job.key);
        }
        if tx.send(Rendered { key: job.key, image }).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}
