//! Miniaturas dos arquivos `.gdr` no Explorador do Windows (GD-GDR-12).
//!
//! O `.gdr` é um ZIP que já traz `thumbnails/page-1.png` (a página 1 com o
//! desenho, GD-GDR-02). Esta DLL é o gerador de miniaturas do shell
//! (`IThumbnailProvider` + `IInitializeWithStream`): o Explorador entrega o
//! arquivo, ela lê a imagem e devolve um bitmap no tamanho pedido. Roda
//! dentro do Explorador, então nunca entra em pânico nem trava: qualquer
//! problema vira `E_FAIL` e o Windows mostra o ícone comum. No canto
//! inferior direito vai o ícone do GraphDraw, para reconhecer o programa.
//!
//! O registro é por usuário (`HKCU\Software\Classes`), sem administrador:
//! `regsvr32 graphdraw_thumbs.dll` (e `/u` para tirar) ou o instalador.

/// Identificador da classe do gerador de miniaturas do GraphDraw.
pub const CLSID_STR: &str = "{2BEA79CE-0115-4A17-B4E5-E2993BBF4825}";

/// Ícone do programa posto no canto da miniatura (o mesmo da janela).
#[cfg(windows)]
const BADGE_PNG: &[u8] = include_bytes!("../../graphdraw-app/assets/brand/app-128.png");
/// Lado do selo em fração da largura da miniatura, com mínimo e máximo (px).
const BADGE_FRACTION: f64 = 0.26;
const BADGE_MIN: u32 = 12;
const BADGE_MAX: u32 = 96;
/// Afastamento do selo até as bordas, em fração do lado do selo.
const BADGE_MARGIN: f64 = 0.12;

/// Lê a miniatura guardada num `.gdr` e devolve (largura, altura, BGRA de
/// cima para baixo) com o lado maior até `max_side` px e o ícone do
/// GraphDraw no canto inferior direito.
pub fn thumbnail_bgra(gdr: &[u8], max_side: u32) -> Option<(u32, u32, Vec<u8>)> {
    let png = read_member(gdr, "thumbnails/page-1.png")?;
    let (w, h, rgba) = decode_png(&png)?;
    let (tw, th, mut rgba) = fit(w, h, &rgba, max_side.max(1));
    stamp_badge(tw, th, &mut rgba);
    let mut bgra = rgba;
    for p in bgra.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    Some((tw, th, bgra))
}

#[cfg(windows)]
fn read_member(gdr: &[u8], name: &str) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(gdr)).ok()?;
    let mut f = z.by_name(name).ok()?;
    // Miniatura de verdade é pequena; um membro enorme é arquivo estranho.
    if f.size() > 32 * 1024 * 1024 {
        return None;
    }
    let mut out = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut out).ok()?;
    Some(out)
}

#[cfg(not(windows))]
fn read_member(_gdr: &[u8], _name: &str) -> Option<Vec<u8>> {
    None
}

/// PNG → RGBA 8 bits.
#[cfg(windows)]
fn decode_png(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().ok()?;
    let (w, h) = (r.info().width, r.info().height);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return None;
    }
    let mut buf = vec![0; r.output_buffer_size()?];
    let frame = r.next_frame(&mut buf).ok()?;
    let src = &buf[..frame.buffer_size()];
    let px = (w * h) as usize;
    let rgba = match frame.color_type {
        png::ColorType::Rgba => src.to_vec(),
        png::ColorType::Rgb => src
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => src
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Grayscale => src.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    (rgba.len() == px * 4).then_some((w, h, rgba))
}

#[cfg(not(windows))]
fn decode_png(_bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    None
}

/// Põe o ícone do programa no canto inferior direito (RGBA, em cima do que
/// estiver lá). Miniatura pequena demais para o selo fica sem ele.
fn stamp_badge(w: u32, h: u32, rgba: &mut [u8]) {
    let side = ((w.min(h) as f64 * BADGE_FRACTION).round() as u32).clamp(BADGE_MIN, BADGE_MAX);
    if side * 2 > w.min(h) {
        return;
    }
    let Some((bw, bh, badge)) = badge_png().map(|(bw, bh, px)| fit(bw, bh, &px, side)) else {
        return;
    };
    let m = ((side as f64 * BADGE_MARGIN).round() as u32).max(1);
    let (x0, y0) = (w - bw - m, h - bh - m);
    for y in 0..bh {
        for x in 0..bw {
            let s = ((y * bw + x) * 4) as usize;
            let d = (((y0 + y) * w + x0 + x) * 4) as usize;
            let a = badge[s + 3] as u32;
            for k in 0..3 {
                rgba[d + k] =
                    ((badge[s + k] as u32 * a + rgba[d + k] as u32 * (255 - a)) / 255) as u8;
            }
            rgba[d + 3] = (a + rgba[d + 3] as u32 * (255 - a) / 255) as u8;
        }
    }
}

#[cfg(windows)]
fn badge_png() -> Option<(u32, u32, Vec<u8>)> {
    decode_png(BADGE_PNG)
}

#[cfg(not(windows))]
fn badge_png() -> Option<(u32, u32, Vec<u8>)> {
    None
}

/// Reduz por média de área até o lado maior caber em `max_side` (não amplia).
fn fit(w: u32, h: u32, rgba: &[u8], max_side: u32) -> (u32, u32, Vec<u8>) {
    let longest = w.max(h);
    if longest <= max_side {
        return (w, h, rgba.to_vec());
    }
    let s = max_side as f64 / longest as f64;
    let tw = ((w as f64 * s).round() as u32).max(1);
    let th = ((h as f64 * s).round() as u32).max(1);
    let mut out = vec![0u8; (tw * th * 4) as usize];
    for ty in 0..th {
        let y0 = (ty as f64 / s) as u32;
        let y1 = (((ty + 1) as f64 / s) as u32).clamp(y0 + 1, h);
        for tx in 0..tw {
            let x0 = (tx as f64 / s) as u32;
            let x1 = (((tx + 1) as f64 / s) as u32).clamp(x0 + 1, w);
            let mut acc = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    for k in 0..4 {
                        acc[k] += rgba[i + k] as u32;
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let o = ((ty * tw + tx) * 4) as usize;
            for k in 0..4 {
                out[o + k] = (acc[k] / n) as u8;
            }
        }
    }
    (tw, th, out)
}

#[cfg(windows)]
mod shell;

#[cfg(test)]
mod tests {
    use super::*;

    /// GD-GDR-12: a miniatura de um .gdr salvo pelo GraphDraw sai no
    /// tamanho pedido pelo Explorador, com o desenho (não só o branco).
    #[cfg(windows)]
    #[test]
    fn gd_gdr_12_thumbnail_from_saved_file() {
        use graphdraw_core::model::{Color, Document, Fill, Object, ObjectKind, Page, RectData};
        let mut d = Document::new();
        let mut page = Page::new(210_000.0, 297_000.0);
        let mut r = Object::new(ObjectKind::Rectangle(RectData {
            width: 150_000.0,
            height: 150_000.0,
            corners: None,
            extra: Default::default(),
        }));
        r.transform = graphdraw_core::model::Transform::translate(30_000.0, 70_000.0);
        r.fill = Some(Fill::solid(Color::rgb(1.0, 0.0, 0.0)));
        page.layers[0].objects.push(r);
        d.data.pages.push(page);
        let bytes = graphdraw_core::gdr::write_bytes(&d, &Default::default()).unwrap();
        let (w, h, bgra) = thumbnail_bgra(&bytes, 256).unwrap();
        assert_eq!(h, 256, "A4 em pé: a altura é o lado maior");
        assert!((w as i32 - 181).abs() <= 1, "{w}");
        assert_eq!(bgra.len(), (w * h * 4) as usize);
        // No meio: vermelho (BGRA); no canto: o branco da página.
        let px = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [bgra[i], bgra[i + 1], bgra[i + 2], bgra[i + 3]]
        };
        assert_eq!(px(w / 2, h / 2), [0, 0, 255, 255]);
        assert_eq!(px(1, 1), [255, 255, 255, 255]);
        // Canto inferior direito: o ícone do programa (verde), não o branco.
        let side = (w as f64 * BADGE_FRACTION).round() as u32;
        let m = (side as f64 * BADGE_MARGIN).round() as u32;
        // Borda esquerda do selo, na meia altura: o verde da moldura.
        let [b, g, r, _] = px(w - m - side + side / 8, h - m - side / 2);
        assert!(g > r && g > b, "selo verde: {r} {g} {b}");
        // Miniatura minúscula: sem selo (nada quebra).
        assert!(thumbnail_bgra(&bytes, 16).is_some());
        // Arquivo qualquer: sem miniatura, sem pânico.
        assert!(thumbnail_bgra(b"isto nao e zip", 256).is_none());
    }

    #[test]
    fn fit_reduces_keeping_aspect() {
        let rgba = vec![255u8; 100 * 50 * 4];
        let (w, h, out) = fit(100, 50, &rgba, 40);
        assert_eq!((w, h), (40, 20));
        assert_eq!(out.len(), 40 * 20 * 4);
        assert!(out.iter().all(|&v| v == 255));
        // Menor que o pedido: fica como está.
        assert_eq!(fit(10, 10, &rgba[..400], 256).0, 10);
    }
}
