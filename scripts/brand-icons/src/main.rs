//! Ícones do GraphDraw a partir do ícone vetorial do projeto
//! (`assets/brand/icone.svg`, desenhado no próprio GraphDraw):
//! - `app-<n>.png` e `app.ico`: o ícone rasterizado em cada tamanho direto
//!   do vetor (nítido também nos pequenos);
//! - `gdr-<n>.png` e `gdr.ico`: folha com dobra e o ícone (arquivo `.gdr`).

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};
use std::path::Path;

const MASTER: u32 = 1024;
const SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];

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

/// Rasteriza o SVG num quadrado de `size` px (fundo transparente).
fn render(tree: &resvg::usvg::Tree, size: u32) -> RgbaImage {
    let mut pix = resvg::tiny_skia::Pixmap::new(size, size).unwrap();
    let s = tree.size();
    let k = (size as f32 / s.width()).min(size as f32 / s.height());
    let dx = (size as f32 - s.width() * k) / 2.0;
    let dy = (size as f32 - s.height() * k) / 2.0;
    resvg::render(
        tree,
        resvg::tiny_skia::Transform::from_row(k, 0.0, 0.0, k, dx, dy),
        &mut pix.as_mut(),
    );
    // tiny-skia guarda alfa pré-multiplicado; o PNG quer o direto.
    let mut img = RgbaImage::new(size, size);
    for (p, c) in img.pixels_mut().zip(pix.pixels()) {
        let c = c.demultiply();
        *p = Rgba([c.red(), c.green(), c.blue(), c.alpha()]);
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
    // Ícone do programa no meio da folha.
    let s = (0.56 * m) as u32;
    let tile = imageops::resize(app, s, s, FilterType::Lanczos3);
    let (tx, ty) = (((m - s as f32) / 2.0) as i64, (0.38 * m) as i64);
    imageops::overlay(&mut img, &tile, tx, ty);
    img
}

/// Grava `<name>-<n>.png` (cada tamanho vindo de `at`), o de 1024 e o `.ico`.
fn write_set(at: &dyn Fn(u32) -> RgbaImage, out: &Path, name: &str) {
    let mut frames = Vec::new();
    for s in SIZES {
        let img = at(s);
        img.save(out.join(format!("{name}-{s}.png"))).unwrap();
        if [16, 24, 32, 48, 64, 128, 256].contains(&s) {
            frames.push(IcoFrame::as_png(img.as_raw(), s, s, image::ExtendedColorType::Rgba8).unwrap());
        }
    }
    at(MASTER).save(out.join(format!("{name}-1024.png"))).unwrap();
    let f = std::fs::File::create(out.join(format!("{name}.ico"))).unwrap();
    IcoEncoder::new(f).encode_images(&frames).unwrap();
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/graphdraw-app/assets/brand");
    let svg = std::fs::read(root.join("icone.svg")).unwrap();
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).unwrap();
    write_set(&|s| render(&tree, s), &root, "app");
    let gdr = gdr_master(&render(&tree, MASTER));
    write_set(&|s| if s == MASTER { gdr.clone() } else { imageops::resize(&gdr, s, s, FilterType::Lanczos3) }, &root, "gdr");
    println!("ícones em {}", root.display());
}
