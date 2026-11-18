//! O que o PDFPack faz com os arquivos, sem interface: comprimir, dividir por tamanho e encaixar
//! em emails. Tudo aqui roda fora da thread da interface.

use std::sync::Arc;

use pdfcraft_cos::{Document, ObjRef, SaveOptions, write_full};
use pdfcraft_optimize::{self as optimize, Compression, ImageSettings, MonoCompression, MonoSettings, Settings, Stage};

pub type Bytes = Arc<Vec<u8>>;

/// Os quatro níveis de compressão do aplicativo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Level {
    Minima,
    Media,
    Alta,
    Extrema,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Minima, Level::Media, Level::Alta, Level::Extrema];

    pub fn name(self) -> &'static str {
        match self {
            Level::Minima => "Mínima",
            Level::Media => "Média",
            Level::Alta => "Alta",
            Level::Extrema => "Extrema",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Level::Minima => "150 ppi · JPEG médio · quase sem perda visível",
            Level::Media => "100 ppi · JPEG mínimo · bom para leitura na tela",
            Level::Alta => "72 ppi · JPEG mínimo · imagens visivelmente mais leves",
            Level::Extrema => "50 ppi · JPEG muito baixo · menor arquivo possível",
        }
    }

    /// As escolhas do otimizador para o nível. `Media` é a configuração de referência do
    /// Otimizador de PDF: 100 ppi acima de 100 ppi, JPEG qualidade Mínima, CCITT grupo 4.
    pub fn settings(self) -> Settings {
        let (ppi, above, quality, mono_ppi, mono_above) = match self {
            Level::Minima => (150.0, 225.0, 60, 300.0, 450.0),
            Level::Media => (100.0, 100.0, 25, 100.0, 100.0),
            Level::Alta => (72.0, 72.0, 25, 100.0, 100.0),
            Level::Extrema => (50.0, 50.0, 12, 72.0, 72.0),
        };
        let image = ImageSettings { downsample: true, target_ppi: ppi, above_ppi: above, compression: Compression::Jpeg(quality) };
        Settings {
            merge_fragments: true,
            color: image,
            gray: image,
            mono: MonoSettings { downsample: true, target_ppi: mono_ppi, above_ppi: mono_above, compression: MonoCompression::CcittG4 },
            ..Settings::default()
        }
    }
}

/// Quanto da compressão de um arquivo já foi feito, de 0 a 1 (mesmos pesos do PDFCraft).
fn fraction(stage: Stage) -> f32 {
    let part = |done: usize, total: usize| if total == 0 { 1.0 } else { done.min(total) as f32 / total as f32 };
    match stage {
        Stage::Fragments => 0.0,
        Stage::Reading { done, total } => 0.25 * if total == 0 { 0.0 } else { part(done, total) },
        Stage::Images { done, total } => 0.25 + 0.55 * part(done, total),
        Stage::CleanUp => 0.8,
    }
}

/// Comprime `bytes` e devolve o PDF novo. `progress` recebe de 0 a 1 e devolve `false` para parar.
pub fn compress(bytes: &Bytes, level: Level, progress: &mut dyn FnMut(f32) -> bool) -> Result<Vec<u8>, String> {
    let settings = level.settings();
    let mut doc = Document::open(bytes.clone()).map_err(|e| e.to_string())?;
    let report = optimize::optimize_with_progress(&mut doc, &settings, &mut |s| progress(fraction(s))).map_err(|e| e.to_string())?;
    #[cfg(test)]
    println!("{report:?}");
    let _ = &report;
    if !progress(0.85) {
        return Err("cancelado".into());
    }
    let all: Vec<ObjRef> = doc.object_numbers().into_iter().map(|n| ObjRef::new(n, doc.generation(n))).collect();
    pdfcraft_organize::dedupe_resources(&mut doc, &all, false);
    if !progress(0.9) {
        return Err("cancelado".into());
    }
    let opts = SaveOptions { object_streams: settings.effective_object_streams(), ..SaveOptions::default() };
    let out = write_full(&doc, &opts).map_err(|e| e.to_string())?;
    // Um arquivo que não abre de novo nunca substitui o original.
    Document::open(Arc::new(out.clone())).map_err(|e| format!("o resultado não abriu: {e}"))?;
    Ok(out)
}

/// Um pedaço de um PDF dividido: as páginas (base 0, fim exclusivo) e o arquivo.
pub struct Part {
    pub pages: std::ops::Range<usize>,
    pub bytes: Vec<u8>,
}

fn write_pages(src: &Document, pages: std::ops::Range<usize>) -> Result<Vec<u8>, String> {
    let doc = pdfcraft_organize::extract_pages(src, &pages.collect::<Vec<_>>()).map_err(|e| e.to_string())?;
    write_full(&doc, &SaveOptions { object_streams: true, ..SaveOptions::default() }).map_err(|e| e.to_string())
}

/// Divide `bytes` em partes de no máximo `max_bytes`, em ordem de página, cada uma com o maior
/// número de páginas que cabe (medido no arquivo de verdade, não estimado). Uma página sozinha
/// maior que o limite vira uma parte só dela.
pub fn split_to_fit(bytes: &Bytes, max_bytes: u64, progress: &mut dyn FnMut(f32) -> bool) -> Result<Vec<Part>, String> {
    let src = Document::open(bytes.clone()).map_err(|e| e.to_string())?;
    let n = pdfcraft_organize::page_count(&src).map_err(|e| e.to_string())?;
    let mut parts = Vec::new();
    let mut start = 0;
    while start < n {
        if !progress(start as f32 / n.max(1) as f32) {
            return Err("cancelado".into());
        }
        // Primeiro palpite proporcional ao tamanho, depois busca binária no número de páginas.
        let mut best = (start + 1, write_pages(&src, start..start + 1)?);
        let (mut lo, mut hi) = (start + 1, n);
        let per_page = (bytes.len() as f64 / n.max(1) as f64).max(1.0);
        let mut guess = start + ((max_bytes as f64 / per_page) as usize).clamp(1, n - start);
        while lo < hi {
            let mid = guess.clamp(lo + 1, hi);
            let out = write_pages(&src, start..mid)?;
            if out.len() as u64 <= max_bytes {
                lo = mid;
                best = (mid, out);
            } else {
                hi = mid - 1;
            }
            guess = lo + (hi - lo).div_ceil(2);
        }
        parts.push(Part { pages: start..best.0, bytes: best.1 });
        start = best.0;
    }
    Ok(parts)
}

/// Encaixa arquivos em emails, na ordem da lista: enche um email até o limite e passa ao próximo.
/// Devolve, para cada email, os índices dos arquivos.
pub fn pack(sizes: &[u64], max_bytes: u64) -> Vec<Vec<usize>> {
    let mut emails: Vec<Vec<usize>> = Vec::new();
    let mut used = 0u64;
    for (i, size) in sizes.iter().enumerate() {
        match emails.last_mut() {
            Some(last) if used + size <= max_bytes => {
                last.push(i);
                used += size;
            }
            _ => {
                emails.push(vec![i]);
                used = *size;
            }
        }
    }
    emails
}

/// O número no começo do nome ("1.", "02 -", "10_"), para a ordem padrão da lista.
pub fn leading_number(name: &str) -> Option<u64> {
    let digits: String = name.trim_start().chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Comparação "natural": números dentro do nome contam pelo valor ("2" antes de "10").
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars<'_>>| {
                    let mut v = 0u128;
                    while let Some(k) = it.peek().and_then(|k| k.to_digit(10)) {
                        v = v.saturating_mul(10).saturating_add(k as u128);
                        it.next();
                    }
                    v
                };
                let o = take(&mut x).cmp(&take(&mut y));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

/// Ordem padrão: primeiro os numerados, pelo número; depois os demais, por nome.
pub fn default_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    match (leading_number(a), leading_number(b)) {
        (Some(x), Some(y)) => x.cmp(&y).then_with(|| natural_cmp(a, b)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => natural_cmp(a, b),
    }
}

pub fn human_size(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1.0 {
        format!("{mb:.1} MB").replace('.', ",")
    } else {
        format!("{:.0} KB", (bytes as f64 / 1024.0).max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_tens_make_two_emails() {
        let mb = 1024 * 1024;
        assert_eq!(pack(&[10 * mb, 10 * mb, 10 * mb], 25 * mb), vec![vec![0, 1], vec![2]]);
    }

    #[test]
    fn an_oversized_file_gets_its_own_email() {
        assert_eq!(pack(&[5, 40, 5], 25), vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn numbered_names_sort_by_number() {
        let mut v = vec!["10. z.pdf", "b.pdf", "2. a.pdf", "1. c.pdf", "a.pdf"];
        v.sort_by(|a, b| default_cmp(a, b));
        assert_eq!(v, ["1. c.pdf", "2. a.pdf", "10. z.pdf", "a.pdf", "b.pdf"]);
    }

    /// Com um PDF de verdade: `PDFPACK_SAMPLE=arquivo.pdf cargo test -p pdfpack --release -- --ignored --nocapture`.
    #[test]
    #[ignore = "precisa de PDFPACK_SAMPLE"]
    fn a_real_file_compresses_and_splits() {
        let path = std::env::var("PDFPACK_SAMPLE").expect("PDFPACK_SAMPLE");
        let bytes: Bytes = Arc::new(std::fs::read(&path).expect("read"));
        for level in Level::ALL {
            let t = std::time::Instant::now();
            let out = compress(&bytes, level, &mut |_| true).expect("compress");
            println!("{:8} {} -> {} em {:?}", level.name(), human_size(bytes.len() as u64), human_size(out.len() as u64), t.elapsed());
        }
        let limit = (bytes.len() as u64 / 3).max(1);
        let t = std::time::Instant::now();
        let parts = split_to_fit(&bytes, limit, &mut |_| true).expect("split");
        let n = pdfcraft_organize::page_count(&Document::open(bytes.clone()).expect("open")).expect("pages");
        assert_eq!(parts.iter().map(|p| p.pages.len()).sum::<usize>(), n);
        for p in &parts {
            println!("partes: páginas {:?} = {} (limite {})", p.pages, human_size(p.bytes.len() as u64), human_size(limit));
            assert!(p.bytes.len() as u64 <= limit || p.pages.len() == 1);
            Document::open(Arc::new(p.bytes.clone())).expect("part reopens");
        }
        println!("dividido em {:?}", t.elapsed());
    }
}
