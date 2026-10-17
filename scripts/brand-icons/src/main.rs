//! Ícones do GraphDraw a partir da logo do projeto:
//! - `app-<n>.png` e `app.ico`: a marca num quadrado escuro de cantos
//!   arredondados (fundo transparente fora dele);
//! - `gdr-<n>.png` e `gdr.ico`: folha com dobra e a marca (arquivo `.gdr`).

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use std::path::Path;

const MASTER: u32 = 1024;
const SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

/// Caixa da marca (sem o texto) na logo de 2400 × 1792: x, y, largura, altura.
const CROP: (u32, u32, u32, u32) = (680, 360, 1040, 860);
/// Largura da marca no quadrado (fração).
const MARK: f32 = 0.82;
/// Saturação (máx − mín, 0–255) do fundo e faixa até a marca opaca.
const CHROMA_BG: f32 = 48.0;
const CHROMA_SPAN: f32 = 50.0;

fn inside_round_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    let cx = x.clamp(x0 + r, x1 - r);
    let cy = y.clamp(y0 + r, y1 - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// Cobertura 0..1 com 4×4 amostras por pixel (borda suave).
fn coverage(px: u32, py: u32, f: &dyn Fn(f32, f32) -> bool) -> f32 {
    let mut n = 0;
    for j in 0..4 {
        for i in 0..4 {
            if f(px as f32 + (i as f32 + 0.5) / 4.0, py as f32 + (j as f32 + 0.5) / 4.0) {
                n += 1;
            }
        }
    }
    n as f32 / 16.0
}

fn app_master(logo: &RgbaImage) -> RgbaImage {
    let (x, y, w, h) = CROP;
    let mut crop = imageops::crop_imm(logo, x, y, w, h).to_image();
    // Fundo: média dos cantos do recorte (o escuro da logo).
    let mut bg = [0f32; 3];
    for (cx, cy) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
        let p = crop.get_pixel(cx, cy).0;
        for k in 0..3 {
            bg[k] += p[k] as f32 / 4.0;
        }
    }
    let bg = bg.map(|v| v.round() as u8);
    // A marca é muito saturada e o fundo é cinza-azulado: a saturação
    // vira o alfa, e a cor é separada do fundo (sem emenda no quadrado).
    for p in crop.pixels_mut() {
        let [r, g, b, _] = p.0;
        let chroma = (r.max(g).max(b) - r.min(g).min(b)) as f32;
        let a = ((chroma - CHROMA_BG) / CHROMA_SPAN).clamp(0.0, 1.0);
        if a <= 0.0 {
            p.0 = [0, 0, 0, 0];
            continue;
        }
        for k in 0..3 {
            let v = (p.0[k] as f32 - bg[k] as f32 * (1.0 - a)) / a;
            p.0[k] = v.clamp(0.0, 255.0).round() as u8;
        }
        p.0[3] = (a * 255.0).round() as u8;
    }
    crop.save(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/graphdraw-app/assets/brand/mark.png")).unwrap();
    let m = MASTER as f32;
    let tw = (m * MARK) as u32;
    let th = (tw as f32 * h as f32 / w as f32) as u32;
    let mark = imageops::resize(&crop, tw, th, FilterType::Lanczos3);
    let mut img = RgbaImage::from_pixel(MASTER, MASTER, Rgba([bg[0], bg[1], bg[2], 255]));
    imageops::overlay(&mut img, &mark, ((MASTER - tw) / 2) as i64, ((MASTER - th) / 2) as i64);
    let r = m * 0.2;
    for (px, py, p) in img.enumerate_pixels_mut() {
        let c = coverage(px, py, &|x, y| inside_round_rect(x, y, 0.0, 0.0, m, m, r));
        p.0[3] = (p.0[3] as f32 * c).round() as u8;
    }
    img
}

fn blend(dst: &mut Rgba<u8>, src: [u8; 3], a: f32) {
    let da = dst.0[3] as f32 / 255.0;
    let oa = a + da * (1.0 - a);
    if oa <= 0.0 {
        return;
    }
    for k in 0..3 {
        let v = (src[k] as f32 * a + dst.0[k] as f32 * da * (1.0 - a)) / oa;
        dst.0[k] = v.round() as u8;
    }
    dst.0[3] = (oa * 255.0).round() as u8;
}

fn gdr_master(app: &RgbaImage) -> RgbaImage {
    let m = MASTER as f32;
    let mut img = RgbaImage::new(MASTER, MASTER);
    let (x0, y0, x1, y1) = (0.16 * m, 0.04 * m, 0.84 * m, 0.96 * m);
    let fold = 0.2 * m;
    let r = 0.04 * m;
    let page = |x: f32, y: f32| {
        inside_round_rect(x, y, x0, y0, x1, y1, r) && (x1 - x) + (y - y0) >= fold
    };
    let border = 0.012 * m;
    let inner = |x: f32, y: f32| {
        inside_round_rect(x, y, x0 + border, y0 + border, x1 - border, y1 - border, r)
            && (x1 - x) + (y - y0) >= fold + border * 1.5
    };
    let flap = |x: f32, y: f32| x <= x1 && y >= y0 && x >= x1 - fold && y <= y0 + fold && (x1 - x) + (y - y0) >= fold && (x1 - x) <= fold && (y - y0) <= fold;
    for (px, py, p) in img.enumerate_pixels_mut() {
        let c = coverage(px, py, &page);
        if c > 0.0 {
            blend(p, [0xB8, 0xBE, 0xCC], c);
        }
        let c = coverage(px, py, &inner);
        if c > 0.0 {
            blend(p, [0xF6, 0xF7, 0xFA], c);
        }
        let c = coverage(px, py, &|x, y| flap(x, y) && (x1 - x) + (y - y0) <= fold + border * 6.0 || flap(x, y) && (x - (x1 - fold)) >= 0.0 && (y0 + fold - y) >= 0.0 && (x1 - x) + (y - y0) >= fold && (x - (x1 - fold)) + (y0 + fold - y) <= fold);
        if c > 0.0 {
            blend(p, [0xD9, 0xDD, 0xE6], c);
        }
    }
    // Marca no meio da folha.
    let s = (0.56 * m) as u32;
    let tile = imageops::resize(app, s, s, FilterType::Lanczos3);
    let (tx, ty) = (((m - s as f32) / 2.0) as i64, (0.38 * m) as i64);
    imageops::overlay(&mut img, &tile, tx, ty);
    img
}

fn write_set(master: &RgbaImage, out: &Path, name: &str) {
    let mut frames = Vec::new();
    for s in SIZES {
        let img = imageops::resize(master, s, s, FilterType::Lanczos3);
        img.save(out.join(format!("{name}-{s}.png"))).unwrap();
        if [16, 24, 32, 48, 64, 128, 256].contains(&s) {
            frames.push(IcoFrame::as_png(img.as_raw(), s, s, image::ExtendedColorType::Rgba8).unwrap());
        }
    }
    master.save(out.join(format!("{name}-1024.png"))).unwrap();
    let f = std::fs::File::create(out.join(format!("{name}.ico"))).unwrap();
    IcoEncoder::new(f).encode_images(&frames).unwrap();
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/graphdraw-app/assets/brand");
    let logo = image::open(root.join("logo-original.jpg")).unwrap().to_rgba8();
    let app = app_master(&logo);
    write_set(&app, &root, "app");
    let gdr = gdr_master(&app);
    write_set(&gdr, &root, "gdr");
    println!("ícones em {}", root.display());
}
