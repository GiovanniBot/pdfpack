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
            Level::Minima => "300 ppi · JPEG alto · sem perda visível",
            Level::Media => "100 ppi · JPEG mínimo · bom para leitura na tela",
            Level::Alta => "72 ppi · JPEG mínimo · imagens visivelmente mais leves",
            Level::Extrema => "50 ppi · JPEG muito baixo · menor arquivo possível",
        }
    }

    /// As escolhas do otimizador para o nível. `Media` é a configuração de referência do
    /// Otimizador de PDF: 100 ppi acima de 100 ppi, JPEG qualidade Mínima, CCITT grupo 4.
    /// `Minima` só reduz o que passa de 450 ppi, mais do que a impressão pede, e regrava em JPEG
    /// qualidade 90: no zoom, igual ao original. O grosso do que ela economiza vem do que nunca
    /// aparece na página (a cópia de edição, os metadados das imagens, os recursos repetidos).
    pub fn settings(self) -> Settings {
        let (ppi, above, quality, mono_ppi, mono_above) = match self {
            Level::Minima => (300.0, 450.0, 90, 300.0, 450.0),
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
            // A cópia de edição que Illustrator, InDesign e outros guardam dentro do PDF: nunca
            // aparece na página e pode ser maior que o próprio PDF.
            discard_private_data: true,
            // Os metadados XMP que cada imagem traz do programa de diagramação: nunca aparecem.
            // Os do documento (título, autor) ficam.
            discard_image_metadata: true,
            // Só na Extrema: comprimir de novo o desenho vetorial (sem perda, mas mais lento).
            reflate: self == Level::Extrema,
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

/// As mesmas páginas de outro arquivo do mesmo PDF: com elas cada parte de um PDF comprimido e
/// dividido guarda o seu original. Um arquivo com outro número de páginas não serve.
pub fn same_pages(bytes: &Bytes, ranges: &[std::ops::Range<usize>]) -> Result<Vec<Vec<u8>>, String> {
    let src = Document::open(bytes.clone()).map_err(|e| e.to_string())?;
    let n = pdfcraft_organize::page_count(&src).map_err(|e| e.to_string())?;
    if ranges.iter().any(|r| r.start >= r.end || r.end > n) {
        return Err("o original não tem as mesmas páginas".into());
    }
    ranges.iter().map(|r| write_pages(&src, r.clone())).collect()
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
    if mb >= 1.0 { format!("{mb:.1} MB").replace('.', ",") } else { format!("{:.0} KB", (bytes as f64 / 1024.0).max(1.0)) }
}

/// PDFs sintéticos para os testes deste e de outros módulos.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use pdfcraft_cos::{Dict, Object, Stream};

    /// Um PDF de `count` páginas de 4 × 4 polegadas, cada uma com uma foto sintética diferente de
    /// `side` × `side` px.
    pub fn photos(count: usize, side: u32) -> Bytes {
        let mut doc = Document::new_empty();
        let pages = doc.root().and_then(|r| doc.get(r).as_dict().and_then(|d| d.reference(b"Pages"))).expect("pages");
        let mut kids = Vec::new();
        for n in 0..count {
            let shift = (n as u32).wrapping_mul(37);
            let mut px = Vec::with_capacity((side * side * 3) as usize);
            for y in 0..side {
                for x in 0..side {
                    let t = ((x ^ y ^ shift) & 15) as u8;
                    px.extend([(x * 255 / side) as u8 ^ t, (y * 255 / side) as u8 ^ (shift as u8), ((x + y) * 127 / side) as u8 ^ (t << 2)]);
                }
            }
            let mut d = Dict::new();
            d.set(b"Type".to_vec(), Object::name("XObject"));
            d.set(b"Subtype".to_vec(), Object::name("Image"));
            d.set(b"Width".to_vec(), Object::Int(side as i64));
            d.set(b"Height".to_vec(), Object::Int(side as i64));
            d.set(b"BitsPerComponent".to_vec(), Object::Int(8));
            d.set(b"ColorSpace".to_vec(), Object::name("DeviceRGB"));
            let img = doc.add(Object::Stream(Stream::flate(d, &px)));
            let mut x = Dict::new();
            x.set(b"Im".to_vec(), Object::Ref(img));
            let mut res = Dict::new();
            res.set(b"XObject".to_vec(), Object::Dict(x));
            let c = doc.add(Object::Stream(Stream::from_raw(Dict::new(), b"q 288 0 0 288 0 0 cm /Im Do Q".to_vec())));
            let mut p = Dict::new();
            p.set(b"Type".to_vec(), Object::name("Page"));
            p.set(b"Parent".to_vec(), Object::Ref(pages));
            p.set(b"MediaBox".to_vec(), Object::Array(vec![0.into(), 0.into(), 288.into(), 288.into()]));
            p.set(b"Resources".to_vec(), Object::Dict(res));
            p.set(b"Contents".to_vec(), Object::Ref(c));
            kids.push(Object::Ref(doc.add(Object::Dict(p))));
        }
        doc.update_dict(pages, |d| {
            d.set(b"Count".to_vec(), Object::Int(kids.len() as i64));
            d.set(b"Kids".to_vec(), Object::Array(kids));
        })
        .expect("pages");
        Arc::new(write_full(&doc, &SaveOptions::default()).expect("save"))
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

    /// Um PDF de uma página com uma foto sintética de 1200 × 1200 px em 4 × 4 polegadas (300 ppi).
    fn photo_pdf() -> Bytes {
        fixtures::photos(1, 1200)
    }

    /// Comprimir na Extrema um arquivo já comprimido na Média ainda diminui, e não fica muito
    /// maior que a Extrema direto do original (o app parte sempre do original, mas o arquivo
    /// salvo pode ser aberto e comprimido de novo).
    #[test]
    fn extrema_over_media_still_shrinks() {
        let original = photo_pdf();
        let media: Bytes = Arc::new(compress(&original, Level::Media, &mut |_| true).expect("média"));
        let both = compress(&media, Level::Extrema, &mut |_| true).expect("extrema depois da média");
        let direct = compress(&original, Level::Extrema, &mut |_| true).expect("extrema");
        assert!(media.len() < original.len(), "{} -> {}", original.len(), media.len());
        assert!(both.len() < media.len(), "média {} -> extrema {}", media.len(), both.len());
        assert!(both.len() * 10 <= direct.len() * 11, "por cima {} contra direto {}", both.len(), direct.len());
    }

    /// A largura em pixels das imagens de um PDF.
    fn image_widths(bytes: &[u8]) -> Vec<i64> {
        let doc = Document::open(Arc::new(bytes.to_vec())).expect("open");
        doc.object_numbers()
            .into_iter()
            .filter_map(|n| match &*doc.get(ObjRef::new(n, doc.generation(n))) {
                pdfcraft_cos::Object::Stream(s) if s.dict.name(b"Subtype") == Some(b"Image".as_slice()) => s.dict.int(b"Width"),
                _ => None,
            })
            .collect()
    }

    /// A Mínima não mexe na resolução de uma foto de 300 ppi, a de uma boa impressão; só as
    /// outras reduzem. Antes ela levava tudo acima de 225 ppi para 150.
    #[test]
    fn minima_keeps_the_resolution_of_a_300_ppi_photo() {
        let original = photo_pdf();
        let minima = compress(&original, Level::Minima, &mut |_| true).expect("mínima");
        assert_eq!(image_widths(&minima), vec![1200]);
        let media = compress(&original, Level::Media, &mut |_| true).expect("média");
        assert_eq!(image_widths(&media), vec![400]);
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
            // Algumas páginas desenhadas antes e depois: a diferença média por componente de cor.
            let draw = |b: &Bytes, page: usize| {
                let mut r = pdfcraft_render::PageRenderer::new(b.clone(), pdfcraft_render::RenderConfig::default());
                let out = r.render(pdfcraft_render::RenderRequest { page, scale: 1.0, ..Default::default() });
                assert!(out.error.is_none(), "{:?}", out.error);
                AsRef::<[u8]>::as_ref(&out.rgba).to_vec()
            };
            let out_bytes: Bytes = Arc::new(out.clone());
            let pages = pdfcraft_organize::page_count(&Document::open(bytes.clone()).expect("open")).expect("pages");
            let mut diffs: Vec<(f64, usize)> = (0..pages)
                .map(|page| {
                    let (a, b) = (draw(&bytes, page), draw(&out_bytes, page));
                    assert_eq!(a.len(), b.len());
                    (a.iter().zip(&b).map(|(x, y)| u64::from(x.abs_diff(*y))).sum::<u64>() as f64 / a.len().max(1) as f64, page)
                })
                .collect();
            diffs.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (diff, page) in diffs.iter().take(3) {
                println!("    página {}: diferença média {diff:.2} (de 255)", page + 1);
            }
            // `PDFPACK_OUT=pasta` guarda cada resultado lá, para conferir.
            if let Some(dir) = std::env::var_os("PDFPACK_OUT") {
                std::fs::write(std::path::Path::new(&dir).join(format!("{}.pdf", level.name())), &out).expect("write");
            }
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
