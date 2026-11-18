//! Pictures in an indexed colour space or in CMYK are resampled too, and look as they did; and
//! the private data of other applications (`/PieceInfo`) can be discarded.

use std::sync::Arc;

use pdfcraft_cos::{Dict, Document, ObjRef, Object, PdfString, SaveOptions, Stream, write_full};
use pdfcraft_optimize::{Compression, ImageSettings, Settings, optimize};

/// A document with one page, `pt` × `pt`, drawing `img` over the whole of it.
fn one_page(doc: &mut Document, pt: u32, img: ObjRef) -> ObjRef {
    let pages = doc.root().and_then(|r| doc.get(r).as_dict().and_then(|d| d.reference(b"Pages"))).unwrap();
    let mut x = Dict::new();
    x.set(b"Im".to_vec(), Object::Ref(img));
    let mut res = Dict::new();
    res.set(b"XObject".to_vec(), Object::Dict(x));
    let content = format!("q {pt} 0 0 {pt} 0 0 cm /Im Do Q");
    let c = doc.add(Object::Stream(Stream::from_raw(Dict::new(), content.into_bytes())));
    let mut p = Dict::new();
    p.set(b"Type".to_vec(), Object::name("Page"));
    p.set(b"Parent".to_vec(), Object::Ref(pages));
    p.set(b"MediaBox".to_vec(), Object::Array(vec![0.into(), 0.into(), (pt as i64).into(), (pt as i64).into()]));
    p.set(b"Resources".to_vec(), Object::Dict(res));
    p.set(b"Contents".to_vec(), Object::Ref(c));
    let r = doc.add(Object::Dict(p));
    doc.update_dict(pages, |d| {
        d.set(b"Kids".to_vec(), Object::Array(vec![Object::Ref(r)]));
        d.set(b"Count".to_vec(), Object::Int(1));
    })
    .unwrap();
    r
}

fn image(doc: &mut Document, side: u32, colour_space: Object, decode: Option<Object>, samples: &[u8]) -> ObjRef {
    let mut d = Dict::new();
    d.set(b"Type".to_vec(), Object::name("XObject"));
    d.set(b"Subtype".to_vec(), Object::name("Image"));
    d.set(b"Width".to_vec(), Object::Int(side as i64));
    d.set(b"Height".to_vec(), Object::Int(side as i64));
    d.set(b"BitsPerComponent".to_vec(), Object::Int(8));
    d.set(b"ColorSpace".to_vec(), colour_space);
    if let Some(decode) = decode {
        d.set(b"Decode".to_vec(), decode);
    }
    doc.add(Object::Stream(Stream::flate(d, samples)))
}

/// Which of four quarters, numbered 0 to 3, pixel (x, y) of a `side` × `side` picture is in.
fn quarter(x: u32, y: u32, side: u32) -> usize {
    usize::from(x >= side / 2) + 2 * usize::from(y >= side / 2)
}

fn stream(doc: &Document, r: ObjRef) -> Stream {
    match &*doc.get(r) {
        Object::Stream(s) => s.clone(),
        _ => panic!("not a stream"),
    }
}

/// The page as PdfCraft's renderer draws it, one device pixel a point: RGB rows.
fn render(doc: &Document) -> (u32, u32, Vec<u8>) {
    let bytes = write_full(doc, &SaveOptions::default()).unwrap();
    let mut r = pdfcraft_render::PageRenderer::new(Arc::new(bytes), pdfcraft_render::RenderConfig::default());
    let out = r.render(pdfcraft_render::RenderRequest { page: 0, scale: 1.0, ..Default::default() });
    assert!(out.error.is_none(), "{:?}", out.error);
    (out.width, out.height, out.rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect())
}

/// The largest difference of a colour component between two renders, away from where the
/// quarters meet and the picture's edges (which resampling smooths).
fn largest_difference(a: &(u32, u32, Vec<u8>), b: &(u32, u32, Vec<u8>)) -> u8 {
    assert_eq!((a.0, a.1), (b.0, b.1));
    let (w, h) = (a.0 as usize, a.1 as usize);
    let away = |v: usize, side: usize| v >= 4 && v + 4 < side && v.abs_diff(side / 2) >= 4;
    let mut most = 0;
    for y in (0..h).filter(|y| away(*y, h)) {
        for x in (0..w).filter(|x| away(*x, w)) {
            for c in 0..3 {
                let i = (y * w + x) * 3 + c;
                most = most.max(a.2[i].abs_diff(b.2[i]));
            }
        }
    }
    most
}

/// Media: to 100 ppi above 100, JPEG.
fn media() -> Settings {
    at_quality(25)
}

/// To 100 ppi above 100, JPEG quality `q`.
fn at_quality(q: u8) -> Settings {
    let image = ImageSettings { downsample: true, target_ppi: 100.0, above_ppi: 100.0, compression: Compression::Jpeg(q) };
    Settings { color: image, gray: image, ..Settings::default() }
}

/// 600 px drawn on 72 pt: 600 ppi.
const SIDE: u32 = 600;
const PT: u32 = 72;

#[test]
fn an_indexed_picture_is_resampled_in_its_base_space_and_keeps_its_colours() {
    // Four colours, one a quarter; the table holds six (two unused) to show it is looked up.
    let table: Vec<u8> = vec![200, 30, 30, 30, 160, 40, 20, 40, 200, 240, 220, 40, 0, 0, 0, 255, 255, 255];
    for (decode, inverted) in [(None, false), (Some(vec![0, 255]), false), (Some(vec![255, 0]), true)] {
        let what = format!("Decode {decode:?}");
        let mut doc = Document::new_empty();
        let indices: Vec<u8> =
            (0..SIDE * SIDE).map(|i| quarter(i % SIDE, i / SIDE, SIDE) as u8).map(|q| if inverted { 255 - q } else { q }).collect();
        let space = Object::Array(vec![
            Object::name("Indexed"),
            Object::name("DeviceRGB"),
            Object::Int(5),
            Object::String(PdfString { bytes: table.clone(), hex: true }),
        ]);
        let decode = decode.map(|d| Object::Array(d.into_iter().map(Object::Int).collect()));
        let img = image(&mut doc, SIDE, space, decode, &indices);
        one_page(&mut doc, PT, img);
        let before = render(&doc);
        // High quality: what is checked is the palette's colours, not JPEG's rounding at their
        // edges, which at low quality spreads a block or two.
        let report = optimize(&mut doc, &at_quality(90)).unwrap();
        let s = stream(&doc, img);
        assert_eq!(report.images_resampled, 1, "{what}");
        assert_eq!(s.dict.int(b"Width"), Some(100), "{what}: 600 ppi to 100");
        assert_eq!(s.dict.get(b"ColorSpace"), Some(&Object::name("DeviceRGB")), "{what}: stored in the palette's base space");
        assert!(!s.dict.contains(b"Decode"), "{what}: the decode array was applied as it was expanded");
        let difference = largest_difference(&before, &render(&doc));
        assert!(difference <= 24, "{what}: the quarters keep their colours (JPEG moves them a little): {difference}");
    }
}

#[test]
fn an_indexed_picture_with_another_decode_array_stays_as_it_is() {
    let mut doc = Document::new_empty();
    let space = Object::Array(vec![
        Object::name("Indexed"),
        Object::name("DeviceRGB"),
        Object::Int(1),
        Object::String(PdfString { bytes: vec![0, 0, 0, 255, 255, 255], hex: true }),
    ]);
    let decode = Object::Array(vec![Object::Int(0), Object::Int(1)]);
    let img = image(&mut doc, SIDE, space, Some(decode), &vec![1; (SIDE * SIDE) as usize]);
    one_page(&mut doc, PT, img);
    let original = doc.get(img);
    optimize(&mut doc, &media()).unwrap();
    assert_eq!(doc.get(img), original);
}

#[test]
fn a_cmyk_picture_is_resampled_with_flate_and_keeps_its_inks() {
    let inks: [[u8; 4]; 4] = [[255, 0, 0, 0], [0, 200, 30, 0], [10, 20, 230, 40], [0, 0, 0, 180]];
    let samples: Vec<u8> = (0..SIDE * SIDE).flat_map(|i| inks[quarter(i % SIDE, i / SIDE, SIDE)]).collect();
    let mut doc = Document::new_empty();
    let img = image(&mut doc, SIDE, Object::name("DeviceCMYK"), None, &samples);
    one_page(&mut doc, PT, img);
    let before = render(&doc);
    let report = optimize(&mut doc, &media()).unwrap();
    let s = stream(&doc, img);
    assert_eq!(report.images_resampled, 1);
    assert_eq!((s.dict.int(b"Width"), s.dict.int(b"Height")), (Some(100), Some(100)));
    assert_eq!(s.dict.get(b"ColorSpace"), Some(&Object::name("DeviceCMYK")));
    assert_eq!(s.dict.name(b"Filter"), Some(&b"FlateDecode"[..]), "CMYK is never stored as JPEG");
    // Not inverted: the inks of each quarter's middle are what they were.
    let px = s.decoded().unwrap();
    for (q, (x, y)) in [(25u32, 25u32), (75, 25), (25, 75), (75, 75)].into_iter().enumerate() {
        let at = ((y * 100 + x) * 4) as usize;
        assert_eq!(px[at..at + 4], inks[q], "quarter {q}");
    }
    assert!(largest_difference(&before, &render(&doc)) <= 2, "the page looks the same");
}

#[test]
fn private_data_of_other_applications_is_discarded_only_when_asked() {
    let build = || {
        let mut doc = Document::new_empty();
        let img = image(&mut doc, 8, Object::name("DeviceGray"), None, &[128; 64]);
        let page = one_page(&mut doc, PT, img);
        // An editor's own copy of the document, stored without a filter, a megabyte.
        // Noise, which compression can't shrink.
        let mut x = 0x2545_f491u32;
        let noise: Vec<u8> = (0..1_000_000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        let copy = doc.add(Object::Stream(Stream::from_raw(Dict::new(), noise)));
        let mut private = Dict::new();
        private.set(b"AIPrivateData1".to_vec(), Object::Ref(copy));
        let private = doc.add(Object::Dict(private));
        let mut app = Dict::new();
        app.set(b"LastModified".to_vec(), Object::String(PdfString { bytes: b"D:20260101".to_vec(), hex: false }));
        app.set(b"Private".to_vec(), Object::Ref(private));
        let mut info = Dict::new();
        info.set(b"Illustrator".to_vec(), Object::Dict(app));
        let info = Object::Dict(info);
        doc.update_dict(page, |d| d.set(b"PieceInfo".to_vec(), info.clone())).unwrap();
        let root = doc.root().unwrap();
        doc.update_dict(root, |d| d.set(b"PieceInfo".to_vec(), info)).unwrap();
        (doc, page)
    };
    let saved = |doc: &Document| write_full(doc, &SaveOptions::default()).unwrap().len();

    let (mut kept, page) = build();
    let report = optimize(&mut kept, &Settings::default()).unwrap();
    assert_eq!(report.private_data, 0);
    assert!(kept.get(page).as_dict().unwrap().contains(b"PieceInfo"), "off by default");
    assert!(saved(&kept) > 1_000_000);

    let (mut doc, page) = build();
    let before = render(&doc);
    let report = optimize(&mut doc, &Settings { discard_private_data: true, ..Settings::default() }).unwrap();
    assert_eq!(report.private_data, 2, "the page's and the catalog's");
    assert!(!doc.get(page).as_dict().unwrap().contains(b"PieceInfo"));
    assert!(saved(&doc) < 10_000, "the copy is left out of the saved file: {}", saved(&doc));
    assert_eq!(largest_difference(&before, &render(&doc)), 0, "the page looks the same");
}

#[test]
fn image_metadata_is_discarded_only_when_asked_and_the_documents_stays() {
    let xmp = |doc: &mut Document, what: &str| {
        let mut d = Dict::new();
        d.set(b"Type".to_vec(), Object::name("Metadata"));
        d.set(b"Subtype".to_vec(), Object::name("XML"));
        doc.add(Object::Stream(Stream::from_raw(d, format!("<x:xmpmeta>{what}</x:xmpmeta>").into_bytes())))
    };
    let build = || {
        let mut doc = Document::new_empty();
        let img = image(&mut doc, 8, Object::name("DeviceGray"), None, &[128; 64]);
        one_page(&mut doc, PT, img);
        let picture = xmp(&mut doc, "picture");
        let s = stream(&doc, img);
        let mut s = s.clone();
        s.dict.set(b"Metadata".to_vec(), Object::Ref(picture));
        doc.set(img, Object::Stream(s));
        let document = xmp(&mut doc, "document");
        let root = doc.root().unwrap();
        doc.update_dict(root, |d| d.set(b"Metadata".to_vec(), Object::Ref(document))).unwrap();
        // As InDesign writes a placed picture: its XMP on the marked content around it; and a
        // layer with metadata of its own.
        let placed = xmp(&mut doc, "placed");
        let mut list = Dict::new();
        list.set(b"Metadata".to_vec(), Object::Ref(placed));
        let list = doc.add(Object::Dict(list));
        let layer_xmp = xmp(&mut doc, "layer");
        let mut layer = Dict::new();
        layer.set(b"Type".to_vec(), Object::name("OCG"));
        layer.set(b"Name".to_vec(), Object::String(PdfString { bytes: b"L".to_vec(), hex: false }));
        layer.set(b"Metadata".to_vec(), Object::Ref(layer_xmp));
        let layer = doc.add(Object::Dict(layer));
        let page = doc.root().and_then(|r| doc.get(r).as_dict().and_then(|d| d.reference(b"Pages"))).unwrap();
        let page = doc.get(page).as_dict().unwrap().get(b"Kids").unwrap().as_array().unwrap()[0].as_ref().unwrap();
        doc.update_dict(page, |p| {
            let mut res = p.get(b"Resources").unwrap().as_dict().unwrap().clone();
            let mut props = Dict::new();
            props.set(b"MC0".to_vec(), Object::Ref(list));
            props.set(b"OC0".to_vec(), Object::Ref(layer));
            res.set(b"Properties".to_vec(), Object::Dict(props));
            p.set(b"Resources".to_vec(), Object::Dict(res));
        })
        .unwrap();
        (doc, img, root)
    };

    let (mut kept, img, _) = build();
    let report = optimize(&mut kept, &Settings::default()).unwrap();
    assert_eq!(report.image_metadata, 0);
    assert!(stream(&kept, img).dict.contains(b"Metadata"), "off by default");

    let (mut doc, img, root) = build();
    let report = optimize(&mut doc, &Settings { discard_image_metadata: true, ..Settings::default() }).unwrap();
    assert_eq!(report.image_metadata, 2, "the picture's and the marked content's");
    assert!(!stream(&doc, img).dict.contains(b"Metadata"));
    assert!(doc.get(root).as_dict().unwrap().contains(b"Metadata"), "the document's metadata stays");
    let saved = write_full(&doc, &SaveOptions { object_streams: false, ..SaveOptions::default() }).unwrap();
    let text = String::from_utf8_lossy(&saved);
    assert!(!text.contains("picture") && !text.contains("placed"), "the pictures' XMP is left out of the file");
    assert!(text.contains("document") && text.contains("layer"), "the document's and the layer's stay");
}

/// Page content compressed weakly, as with zlib's fastest setting and its stored blocks: here
/// written as stored deflate blocks, which hold the data as it is.
fn stored_zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = data.chunks(65535).collect();
    for (i, c) in chunks.iter().enumerate() {
        out.push(u8::from(i + 1 == chunks.len()));
        let len = c.len() as u16;
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend_from_slice(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}

#[test]
fn weakly_compressed_streams_are_compressed_again_only_when_asked_and_draw_the_same() {
    let build = || {
        let mut doc = Document::new_empty();
        let img = image(&mut doc, 8, Object::name("DeviceGray"), None, &[128; 64]);
        let page = one_page(&mut doc, PT, img);
        // The picture, then many grey rectangles: content as long and as repetitive as a drawing's.
        let mut content = b"q 72 0 0 72 0 0 cm /Im Do Q
"
        .to_vec();
        for i in 0..2000 {
            content.extend(
                format!(
                    "0.{} g {} {} 3 3 re f
",
                    i % 10,
                    i % 60,
                    (i / 60) % 60
                )
                .into_bytes(),
            );
        }
        let mut d = Dict::new();
        d.set(b"Filter".to_vec(), Object::name("FlateDecode"));
        let c = doc.add(Object::Stream(Stream::from_raw(d, stored_zlib(&content))));
        doc.update_dict(page, |p| p.set(b"Contents".to_vec(), Object::Ref(c))).unwrap();
        (doc, c, content)
    };

    let (mut kept, c, _) = build();
    let original = kept.get(c);
    let report = optimize(&mut kept, &Settings::default()).unwrap();
    assert_eq!(report.streams_reflated, 0);
    assert_eq!(kept.get(c), original, "off by default");

    let (mut doc, c, content) = build();
    let before = render(&doc);
    let was = stream(&doc, c).raw.len();
    let report = optimize(&mut doc, &Settings { reflate: true, ..Settings::default() }).unwrap();
    assert!(report.streams_reflated >= 1, "{report:?}");
    let now = stream(&doc, c);
    assert!(now.raw.len() * 4 < was, "{} -> {}", was, now.raw.len());
    assert_eq!(now.decoded().unwrap(), content, "the same content");
    assert_eq!(largest_difference(&before, &render(&doc)), 0, "the page looks the same");
}

/// PDFPack: a picture whose soft mask can't be resampled with it (here one with a `/Matte`) is
/// still recompressed at its own size, and the mask stays as it is.
#[test]
fn a_picture_whose_mask_has_a_matte_is_recompressed_at_its_own_size() {
    let mut doc = Document::new_empty();
    let alpha: Vec<u8> = (0..SIDE * SIDE).map(|i| (i % SIDE * 255 / SIDE) as u8).collect();
    let mask = image(&mut doc, SIDE, Object::name("DeviceGray"), None, &alpha);
    let mut m = stream(&doc, mask);
    m.dict.set(b"Matte".to_vec(), Object::Array(vec![Object::Int(0); 3]));
    doc.set(mask, Object::Stream(m.clone()));
    let photo: Vec<u8> = (0..SIDE * SIDE).flat_map(|i| [(i % SIDE) as u8, (i / SIDE % 256) as u8, ((i * 7) % 251) as u8]).collect();
    let img = image(&mut doc, SIDE, Object::name("DeviceRGB"), None, &photo);
    let mut s = stream(&doc, img);
    s.dict.set(b"SMask".to_vec(), Object::Ref(mask));
    doc.set(img, Object::Stream(s));
    one_page(&mut doc, PT, img);
    let report = optimize(&mut doc, &media()).unwrap();
    let s = stream(&doc, img);
    assert_eq!(report.images_recompressed, 1, "{report:?}");
    assert_eq!(s.dict.int(b"Width"), Some(SIDE as i64), "its own size");
    assert_eq!(s.dict.name(b"Filter"), Some(&b"DCTDecode"[..]));
    assert_eq!(stream(&doc, mask), m, "the mask stays as it is");
}
