//! Fidelidade de importação `.cdr` (doc 25, GD-FID-01..42): compara, caixa
//! preta, o PDF de referência exportado pelo dono do arquivo com o PDF que
//! o GraphDraw gera do mesmo original. Os dois passam pelo **mesmo**
//! rasterizador (`pdftoppm`, do Poppler) e são comparados pixel a pixel.
//!
//! Métricas por página: diferença visual (% de pixels com ΔE2000 > 5 depois
//! de alinhar, GD-FID-10), SSIM, geometria (tamanho da página e caixa da
//! tinta, em mm; GD-FID-11), contagem de páginas (GD-FID-14), texto
//! extraído e fontes (GD-FID-13, com `pdftotext`/`pdffonts`) e
//! robustez/tempo de abertura (GD-FID-15). A geometria vem da imagem: a
//! resolução é a do pixel (0,17 mm a 150 ppp). Caixas por objeto e valores
//! CMYK (GD-FID-12) ainda não são medidos.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Limite de ΔE2000 para um pixel contar como diferente (GD-FID-10).
pub const DELTA_E_LIMIT: f64 = 5.0;

/// Nível de aprovação de uma página (doc 25, "Limiares").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Fiel,
    Aceitavel,
    Falha,
}

impl Level {
    /// Pela diferença visual, SSIM e desvio de geometria (mm).
    pub fn of(diff_pct: f64, ssim: f64, geometry_mm: f64) -> Level {
        if diff_pct <= 1.0 && ssim >= 0.98 && geometry_mm <= 0.1 {
            Level::Fiel
        } else if diff_pct <= 5.0 && ssim >= 0.95 && geometry_mm <= 0.5 {
            Level::Aceitavel
        } else {
            Level::Falha
        }
    }
    /// Rebaixa pelo desvio CMYK (pp): até 1 Fiel, até 3 Aceitável.
    pub fn with_cmyk(self, pp: Option<f64>) -> Level {
        let by = match pp {
            Some(v) if v > 3.0 => Level::Falha,
            Some(v) if v > 1.0 => Level::Aceitavel,
            _ => Level::Fiel,
        };
        let rank = |l: Level| match l {
            Level::Fiel => 0,
            Level::Aceitavel => 1,
            Level::Falha => 2,
        };
        if rank(by) > rank(self) { by } else { self }
    }

    pub fn label(self) -> &'static str {
        match self {
            Level::Fiel => "Fiel",
            Level::Aceitavel => "Aceitável",
            Level::Falha => "Falha",
        }
    }
}

// ---- cor: sRGB → Lab (D65) e CIEDE2000 -----------------------------------------

fn srgb_to_lab(rgb: [u8; 3]) -> [f64; 3] {
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = (0.0193339 * r + 0.1191920 * g + 0.9503041 * b) / 1.08883;
    let f = |t: f64| {
        if t > 216.0 / 24389.0 {
            t.cbrt()
        } else {
            (24389.0 / 27.0 * t + 16.0) / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Diferença de cor CIEDE2000 entre duas cores Lab.
pub fn delta_e2000(a: [f64; 3], b: [f64; 3]) -> f64 {
    use std::f64::consts::PI;
    let (l1, a1, b1) = (a[0], a[1], a[2]);
    let (l2, a2, b2) = (b[0], b[1], b[2]);
    let c1 = a1.hypot(b1);
    let c2 = a2.hypot(b2);
    let cm = (c1 + c2) / 2.0;
    let g = 0.5 * (1.0 - (cm.powi(7) / (cm.powi(7) + 25f64.powi(7))).sqrt());
    let (a1p, a2p) = ((1.0 + g) * a1, (1.0 + g) * a2);
    let (c1p, c2p) = (a1p.hypot(b1), a2p.hypot(b2));
    let hue = |b: f64, a: f64| {
        if a == 0.0 && b == 0.0 {
            0.0
        } else {
            let h = b.atan2(a).to_degrees();
            if h < 0.0 { h + 360.0 } else { h }
        }
    };
    let (h1p, h2p) = (hue(b1, a1p), hue(b2, a2p));
    let dl = l2 - l1;
    let dc = c2p - c1p;
    let dh = if c1p * c2p == 0.0 {
        0.0
    } else if (h2p - h1p).abs() <= 180.0 {
        h2p - h1p
    } else if h2p - h1p > 180.0 {
        h2p - h1p - 360.0
    } else {
        h2p - h1p + 360.0
    };
    let dhh = 2.0 * (c1p * c2p).sqrt() * (dh.to_radians() / 2.0).sin();
    let lm = (l1 + l2) / 2.0;
    let cmp = (c1p + c2p) / 2.0;
    let hm = if c1p * c2p == 0.0 {
        h1p + h2p
    } else if (h1p - h2p).abs() <= 180.0 {
        (h1p + h2p) / 2.0
    } else if h1p + h2p < 360.0 {
        (h1p + h2p + 360.0) / 2.0
    } else {
        (h1p + h2p - 360.0) / 2.0
    };
    let t = 1.0 - 0.17 * (hm - 30.0).to_radians().cos()
        + 0.24 * (2.0 * hm).to_radians().cos()
        + 0.32 * (3.0 * hm + 6.0).to_radians().cos()
        - 0.20 * (4.0 * hm - 63.0).to_radians().cos();
    let dtheta = 30.0 * (-((hm - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cmp.powi(7) / (cmp.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (lm - 50.0).powi(2) / (20.0 + (lm - 50.0).powi(2)).sqrt();
    let sc = 1.0 + 0.045 * cmp;
    let sh = 1.0 + 0.015 * cmp * t;
    let rt = -(2.0 * dtheta * PI / 180.0).sin() * rc;
    ((dl / sl).powi(2) + (dc / sc).powi(2) + (dhh / sh).powi(2) + rt * (dc / sc) * (dhh / sh))
        .sqrt()
}

// ---- imagens -------------------------------------------------------------------

/// Página rasterizada: RGB 8 bits.
#[derive(Debug, Clone, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u8>,
}

impl Raster {
    fn px(&self, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * self.width + x) * 3) as usize;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]]
    }

    /// Pixel deslocado; fora da imagem é papel branco.
    fn px_at(&self, x: i64, y: i64) -> [u8; 3] {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            [255, 255, 255]
        } else {
            self.px(x as u32, y as u32)
        }
    }

    /// Reamostra (vizinho mais próximo) para o tamanho dado.
    fn resized(&self, w: u32, h: u32) -> Raster {
        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let sx = (x as u64 * self.width as u64 / w.max(1) as u64) as u32;
                let sy = (y as u64 * self.height as u64 / h.max(1) as u64) as u32;
                rgb.extend(self.px(sx.min(self.width - 1), sy.min(self.height - 1)));
            }
        }
        Raster {
            width: w,
            height: h,
            rgb,
        }
    }

    /// Imagem deslocada de `(dx, dy)` pixels.
    fn shifted(&self, dx: i32, dy: i32) -> Raster {
        if (dx, dy) == (0, 0) {
            return self.clone();
        }
        let mut rgb = Vec::with_capacity(self.rgb.len());
        for y in 0..self.height as i64 {
            for x in 0..self.width as i64 {
                rgb.extend(self.px_at(x - dx as i64, y - dy as i64));
            }
        }
        Raster {
            width: self.width,
            height: self.height,
            rgb,
        }
    }

    /// Caixa da tinta (pixels que não são papel): `[x0, y0, x1, y1]` em
    /// pixels, `x1`/`y1` exclusivos.
    fn ink_box(&self) -> Option<[u32; 4]> {
        let mut b: Option<[u32; 4]> = None;
        for y in 0..self.height {
            for x in 0..self.width {
                if self.px(x, y).iter().any(|c| *c < INK_LEVEL) {
                    let e = b.get_or_insert([x, y, x + 1, y + 1]);
                    e[0] = e[0].min(x);
                    e[1] = e[1].min(y);
                    e[2] = e[2].max(x + 1);
                    e[3] = e[3].max(y + 1);
                }
            }
        }
        b
    }
}

/// Abaixo disto (em qualquer canal) o pixel tem tinta.
const INK_LEVEL: u8 = 245;
/// Deslocamento máximo procurado no alinhamento, em pixels.
const MAX_SHIFT: i32 = 3;

/// Resultado da comparação de duas páginas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageScore {
    pub page: usize,
    /// % de pixels com ΔE2000 > 5, depois do alinhamento.
    pub diff_pct: f64,
    pub ssim: f64,
    /// Tamanhos diferentes (o candidato foi reamostrado).
    pub size_mismatch: bool,
    /// Deslocamento (pixels) que melhor alinha o candidato à referência.
    #[serde(default)]
    pub shift: [i32; 2],
    /// Maior desvio de geometria em mm: tamanho da página e bordas da
    /// caixa da tinta (inclui o deslocamento).
    #[serde(default)]
    pub geometry_mm: f64,
    /// Desvio CMYK em pontos percentuais contra `ref-cmyk.pdf`
    /// (GD-FID-12); `None` sem referência CMYK.
    #[serde(default)]
    pub cmyk_pp: Option<f64>,
    pub level: Level,
}

/// Deslocamento (até [`MAX_SHIFT`] px) com menos pixels diferentes,
/// medido numa grade espaçada para ser rápido.
fn best_shift(a: &Raster, b: &Raster) -> (i32, i32) {
    let step = ((a.width.max(a.height) / 300).max(1)) as usize;
    let far = |p: [u8; 3], q: [u8; 3]| {
        p.iter()
            .zip(q)
            .map(|(x, y)| x.abs_diff(y) as u32)
            .sum::<u32>()
            > 24
    };
    let mut best: ((i32, i32), usize) = ((0, 0), usize::MAX);
    for dy in -MAX_SHIFT..=MAX_SHIFT {
        for dx in -MAX_SHIFT..=MAX_SHIFT {
            let mut n = 0;
            // Moldura ampliada: empurrar tinta para fora da página não
            // conta como acerto.
            let m = MAX_SHIFT as i64;
            for y in (-m..a.height as i64 + m).step_by(step) {
                for x in (-m..a.width as i64 + m).step_by(step) {
                    if far(a.px_at(x, y), b.px_at(x - dx as i64, y - dy as i64)) {
                        n += 1;
                    }
                }
            }
            // Empate: o menor deslocamento.
            let better = n < best.1
                || (n == best.1 && dx.abs() + dy.abs() < best.0.0.abs() + best.0.1.abs());
            if better {
                best = ((dx, dy), n);
            }
        }
    }
    best.0
}

/// Compara referência e candidato rasterizados a `dpi`; devolve a nota e o
/// mapa de diferença (pixels diferentes em vermelho sobre a referência
/// clareada).
pub fn compare(
    reference: &Raster,
    candidate: &Raster,
    page: usize,
    dpi: u32,
) -> (PageScore, Raster) {
    let mm = 25.4 / dpi.max(1) as f64;
    let size_mismatch = reference.width != candidate.width || reference.height != candidate.height;
    // Geometria: tamanho da página e caixa da tinta, antes de alinhar.
    let mut geometry_px: f64 = (reference.width as f64 - candidate.width as f64)
        .abs()
        .max((reference.height as f64 - candidate.height as f64).abs());
    let cand = if size_mismatch {
        candidate.resized(reference.width, reference.height)
    } else {
        candidate.clone()
    };
    match (reference.ink_box(), candidate.ink_box()) {
        (Some(a), Some(b)) => {
            for i in 0..4 {
                geometry_px = geometry_px.max((a[i] as f64 - b[i] as f64).abs());
            }
        }
        (None, None) => {}
        // Só um dos dois tem tinta: a página inteira está errada.
        _ => geometry_px = geometry_px.max(reference.width.max(reference.height) as f64),
    }
    let (dx, dy) = best_shift(reference, &cand);
    let cand = cand.shifted(dx, dy);
    let (w, h) = (reference.width, reference.height);
    let mut diff = 0usize;
    let mut map = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let (a, b) = (reference.px(x, y), cand.px(x, y));
            let de = if a == b {
                0.0
            } else {
                delta_e2000(srgb_to_lab(a), srgb_to_lab(b))
            };
            if de > DELTA_E_LIMIT {
                diff += 1;
                map.extend([220, 30, 30]);
            } else {
                // Referência clareada para dar contexto.
                map.extend(a.map(|c| 255 - (255 - c) / 4));
            }
        }
    }
    let total = (w as usize * h as usize).max(1);
    let diff_pct = diff as f64 * 100.0 / total as f64;
    let ssim = ssim(reference, &cand);
    let geometry_mm = geometry_px * mm;
    let level = Level::of(diff_pct, ssim, geometry_mm);
    (
        PageScore {
            page,
            diff_pct,
            ssim,
            size_mismatch,
            shift: [dx, dy],
            geometry_mm,
            cmyk_pp: None,
            level,
        },
        Raster {
            width: w,
            height: h,
            rgb: map,
        },
    )
}

/// SSIM médio da luminância em janelas de 8 × 8 (sem sobreposição).
pub fn ssim(a: &Raster, b: &Raster) -> f64 {
    let lum = |r: &Raster, x: u32, y: u32| {
        let p = r.px(x, y);
        0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64
    };
    let (c1, c2) = ((0.01f64 * 255.0).powi(2), (0.03f64 * 255.0).powi(2));
    let (w, h) = (a.width.min(b.width), a.height.min(b.height));
    let (mut sum, mut n) = (0.0, 0usize);
    let step = 8;
    let mut y = 0;
    while y + step <= h.max(step) && y < h {
        let mut x = 0;
        while x < w {
            let (x1, y1) = ((x + step).min(w), (y + step).min(h));
            let count = ((x1 - x) * (y1 - y)) as f64;
            let (mut ma, mut mb) = (0.0, 0.0);
            for yy in y..y1 {
                for xx in x..x1 {
                    ma += lum(a, xx, yy);
                    mb += lum(b, xx, yy);
                }
            }
            ma /= count;
            mb /= count;
            let (mut va, mut vb, mut cov) = (0.0, 0.0, 0.0);
            for yy in y..y1 {
                for xx in x..x1 {
                    let (da, db) = (lum(a, xx, yy) - ma, lum(b, xx, yy) - mb);
                    va += da * da;
                    vb += db * db;
                    cov += da * db;
                }
            }
            va /= count;
            vb /= count;
            cov /= count;
            sum += ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            n += 1;
            x += step;
        }
        y += step;
    }
    if n == 0 { 1.0 } else { sum / n as f64 }
}

// ---- ferramentas externas (Poppler) -------------------------------------------

/// Programa do Poppler: variável `GRAPHDRAW_POPPLER` (pasta), PATH, ou
/// pastas comuns do MSYS2.
pub fn poppler_tool(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("GRAPHDRAW_POPPLER") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    if let Some(home) = std::env::var_os("LOCALAPPDATA") {
        let base = PathBuf::from(home).join("Programs").join("msys64");
        dirs.push(base.join("ucrt64").join("bin"));
        dirs.push(base.join("mingw64").join("bin"));
    }
    dirs.push(PathBuf::from(r"C:\msys64\ucrt64\bin"));
    dirs.push(PathBuf::from(r"C:\msys64\mingw64\bin"));
    dirs.into_iter().map(|d| d.join(&exe)).find(|p| p.is_file())
}

/// Rasteriza todas as páginas de um PDF a `dpi` com o `pdftoppm`.
pub fn rasterize(pdf: &Path, dpi: u32, work: &Path) -> Result<Vec<Raster>, String> {
    let tool = poppler_tool("pdftoppm")
        .ok_or("pdftoppm (Poppler) não encontrado; defina GRAPHDRAW_POPPLER")?;
    std::fs::create_dir_all(work).map_err(|e| e.to_string())?;
    let prefix = work.join("p");
    let out = Command::new(tool)
        .arg("-r")
        .arg(dpi.to_string())
        .arg("-png")
        .arg(pdf)
        .arg(&prefix)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "pdftoppm falhou: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(work)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    // p-1.png, p-02.png…: ordena pelo número.
    files.sort_by_key(|p| {
        p.file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('-').next())
            .and_then(|n| n.parse::<u32>().ok())
            .unwrap_or(0)
    });
    files
        .iter()
        .map(|f| {
            let img = image::open(f).map_err(|e| e.to_string())?.to_rgb8();
            Ok(Raster {
                width: img.width(),
                height: img.height(),
                rgb: img.into_raw(),
            })
        })
        .collect()
}

/// Texto do PDF (`pdftotext`), com os espaços normalizados.
pub fn pdf_text(pdf: &Path) -> Option<String> {
    let tool = poppler_tool("pdftotext")?;
    let out = Command::new(tool)
        .arg("-enc")
        .arg("UTF-8")
        .arg(pdf)
        .arg("-")
        .output()
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    })
}

/// Fontes de um PDF (`pdffonts`), sem o prefixo de subconjunto
/// (`ABCDEF+`), em ordem e sem repetição.
pub fn pdf_fonts(pdf: &Path) -> Option<Vec<String>> {
    let tool = poppler_tool("pdffonts")?;
    let out = Command::new(tool).arg(pdf).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names: Vec<String> = text
        .lines()
        .skip(2)
        .filter_map(|l| l.split_whitespace().next())
        .map(strip_subset)
        .collect();
    names.sort();
    names.dedup();
    Some(names)
}

fn strip_subset(name: &str) -> String {
    match name.split_once('+') {
        Some((tag, rest)) if tag.len() == 6 && tag.chars().all(|c| c.is_ascii_uppercase()) => {
            rest.to_string()
        }
        _ => name.to_string(),
    }
}

/// Nome de fonte para comparar: só letras e números, minúsculas.
fn font_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

// ---- cor CMYK (GD-FID-12) -----------------------------------------------------

/// Cores CMYK (0–1) usadas em cada página do PDF: operadores `k`/`K` e
/// `sc`/`scn` com quatro números, no conteúdo da página e nos formulários
/// (XObject) que ela usa.
pub fn pdf_cmyk_colors(pdf: &Path) -> Option<Vec<Vec<[f64; 4]>>> {
    let doc = lopdf::Document::load(pdf).ok()?;
    let mut pages = Vec::new();
    for (_, id) in doc.get_pages() {
        let mut colors = Vec::new();
        if let Ok(c) = doc.get_and_decode_page_content(id) {
            collect_cmyk(&c.operations, &mut colors);
        }
        if let Ok((res, _)) = doc.get_page_resources(id)
            && let Some(res) = res
        {
            forms_cmyk(&doc, res, &mut colors, 0);
        }
        colors.sort_by(|a: &[f64; 4], b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        colors.dedup();
        pages.push(colors);
    }
    Some(pages)
}

fn collect_cmyk(ops: &[lopdf::content::Operation], out: &mut Vec<[f64; 4]>) {
    for op in ops {
        let cmyk = matches!(op.operator.as_str(), "k" | "K")
            || (matches!(op.operator.as_str(), "sc" | "scn" | "SC" | "SCN")
                && op.operands.len() == 4);
        if !cmyk || op.operands.len() != 4 {
            continue;
        }
        let v: Vec<f64> = op
            .operands
            .iter()
            .filter_map(|o| o.as_float().ok().map(f64::from))
            .collect();
        if let [c, m, y, k] = v[..] {
            out.push([c, m, y, k].map(|x| x.clamp(0.0, 1.0)));
        }
    }
}

fn forms_cmyk(
    doc: &lopdf::Document,
    res: &lopdf::Dictionary,
    out: &mut Vec<[f64; 4]>,
    depth: usize,
) {
    if depth > 4 {
        return;
    }
    let Ok(xobjects) = res
        .get(b"XObject")
        .and_then(|o| doc.dereference(o))
        .and_then(|(_, o)| o.as_dict())
    else {
        return;
    };
    for (_, x) in xobjects.iter() {
        let Ok((_, lopdf::Object::Stream(s))) = doc.dereference(x) else {
            continue;
        };
        if s.dict.get(b"Subtype").and_then(|t| t.as_name()).ok() != Some(b"Form".as_slice()) {
            continue;
        }
        let data = s
            .decompressed_content()
            .unwrap_or_else(|_| s.content.clone());
        if let Ok(c) = lopdf::content::Content::decode(&data) {
            collect_cmyk(&c.operations, out);
        }
        if let Ok((_, lopdf::Object::Dictionary(r))) =
            s.dict.get(b"Resources").and_then(|o| doc.dereference(o))
        {
            forms_cmyk(doc, r, out, depth + 1);
        }
    }
}

/// Maior desvio (pontos percentuais) entre cada cor da referência e a cor
/// mais próxima do candidato; `None` sem cores na referência.
pub fn cmyk_deviation(reference: &[[f64; 4]], candidate: &[[f64; 4]]) -> Option<f64> {
    if reference.is_empty() {
        return None;
    }
    let dist = |a: &[f64; 4], b: &[f64; 4]| (0..4).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max);
    Some(
        reference
            .iter()
            .map(|r| candidate.iter().map(|c| dist(r, c)).fold(1.0, f64::min))
            .fold(0.0, f64::max)
            * 100.0,
    )
}

fn write_png(r: &Raster, path: &Path) -> Result<(), String> {
    image::RgbImage::from_raw(r.width, r.height, r.rgb.clone())
        .ok_or("imagem inválida")?
        .save(path)
        .map_err(|e| e.to_string())
}

// ---- execução sobre o corpus ---------------------------------------------------

/// `meta.json` de um caso (GD-FID-04).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub versao_origem: String,
    #[serde(default)]
    pub versao_cdr: String,
    #[serde(default)]
    pub categoria: String,
    #[serde(default)]
    pub autorizado_por: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case: String,
    pub meta: Meta,
    /// Abriu sem erro nem pânico (GD-FID-15).
    pub opened: bool,
    pub error: Option<String>,
    pub open_ms: u128,
    pub ref_pages: usize,
    pub cand_pages: usize,
    pub pages: Vec<PageScore>,
    /// Texto igual ao de `ref-texto.pdf` (`None` = sem referência).
    pub text_equal: Option<bool>,
    /// Fontes que o relatório de importação deu como ausentes.
    pub missing_fonts: Vec<String>,
    /// Fontes de `ref-texto.pdf` e do candidato com texto vivo
    /// (`pdffonts`), e as da referência que o candidato não usa
    /// (substituídas, GD-FID-13).
    #[serde(default)]
    pub ref_fonts: Vec<String>,
    #[serde(default)]
    pub cand_fonts: Vec<String>,
    #[serde(default)]
    pub substituted_fonts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub date: String,
    pub dpi: u32,
    pub cases: Vec<CaseResult>,
    pub summary: Summary,
}

/// Números agregados (o que pode ir para o repositório público, GD-FID-31).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub cases: usize,
    pub pages: usize,
    pub fiel: usize,
    pub aceitavel: usize,
    pub falha: usize,
    pub crashes: usize,
    pub fiel_pct: f64,
    pub fiel_ou_aceitavel_pct: f64,
    /// Variação de "Fiel" (pontos percentuais) desde a execução anterior.
    pub trend_fiel_pp: Option<f64>,
    /// Páginas por categoria e por versão do formato (GD-FID-30).
    #[serde(default)]
    pub by_category: BTreeMap<String, Counts>,
    #[serde(default)]
    pub by_version: BTreeMap<String, Counts>,
}

/// Páginas por nível.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Counts {
    pub pages: usize,
    pub fiel: usize,
    pub aceitavel: usize,
    pub falha: usize,
}

impl Counts {
    fn add(&mut self, l: Level) {
        self.pages += 1;
        match l {
            Level::Fiel => self.fiel += 1,
            Level::Aceitavel => self.aceitavel += 1,
            Level::Falha => self.falha += 1,
        }
    }
}

impl Summary {
    fn of(cases: &[CaseResult]) -> Summary {
        let mut s = Summary {
            cases: cases.len(),
            ..Default::default()
        };
        for c in cases {
            if !c.opened {
                s.crashes += 1;
            }
            let label = |v: &str| {
                if v.is_empty() {
                    "—".to_string()
                } else {
                    v.to_string()
                }
            };
            // Páginas que faltam no candidato (ou caso que não abriu)
            // contam como falha.
            let missing = c
                .ref_pages
                .max(usize::from(!c.opened))
                .saturating_sub(c.pages.len());
            let levels = c
                .pages
                .iter()
                .map(|p| p.level)
                .chain(std::iter::repeat_n(Level::Falha, missing));
            for l in levels {
                s.pages += 1;
                match l {
                    Level::Fiel => s.fiel += 1,
                    Level::Aceitavel => s.aceitavel += 1,
                    Level::Falha => s.falha += 1,
                }
                s.by_category
                    .entry(label(&c.meta.categoria))
                    .or_default()
                    .add(l);
                s.by_version
                    .entry(label(&c.meta.versao_cdr))
                    .or_default()
                    .add(l);
            }
        }
        if s.pages > 0 {
            s.fiel_pct = s.fiel as f64 * 100.0 / s.pages as f64;
            s.fiel_ou_aceitavel_pct = (s.fiel + s.aceitavel) as f64 * 100.0 / s.pages as f64;
        }
        s
    }
}

fn find_original(dir: &Path) -> Option<PathBuf> {
    ["original.cdr", "original.cmx", "original.gdr"]
        .iter()
        .map(|n| dir.join(n))
        .find(|p| p.is_file())
}

/// Executa um caso: abre o original, exporta o candidato, rasteriza os dois
/// e compara. As imagens vão para `diffs`.
pub fn run_case(dir: &Path, dpi: u32, diffs: &Path) -> CaseResult {
    let case = dir
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let meta: Meta = std::fs::read(dir.join("meta.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut r = CaseResult {
        case: case.clone(),
        meta,
        opened: false,
        error: None,
        open_ms: 0,
        ref_pages: 0,
        cand_pages: 0,
        pages: Vec::new(),
        text_equal: None,
        missing_fonts: Vec::new(),
        ref_fonts: Vec::new(),
        cand_fonts: Vec::new(),
        substituted_fonts: Vec::new(),
    };
    let Some(original) = find_original(dir) else {
        r.error = Some("sem original.cdr/.cmx/.gdr".into());
        return r;
    };
    let reference = dir.join("ref-curvas.pdf");
    let work = diffs.join(format!("{case}-tmp"));
    let t0 = Instant::now();
    let opened = std::panic::catch_unwind(|| crate::open_path(&original));
    r.open_ms = t0.elapsed().as_millis();
    let doc = match opened {
        Ok(Ok((doc, report))) => {
            r.opened = true;
            r.missing_fonts = report
                .items
                .iter()
                .filter(|m| m.code.contains("font"))
                .map(|m| m.message.clone())
                .collect();
            doc
        }
        Ok(Err(e)) => {
            r.error = Some(e.to_string());
            return r;
        }
        Err(_) => {
            r.error = Some("pânico ao abrir".into());
            return r;
        }
    };
    let opts = crate::export::ExportOptions {
        text_to_curves: true,
        ..Default::default()
    };
    let cand_pdf = work.join("candidato.pdf");
    let _ = std::fs::create_dir_all(&work);
    match crate::export::pdf_document(&doc, &opts).map(|b| std::fs::write(&cand_pdf, b)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            r.error = Some(e.to_string());
            return r;
        }
        Err(e) => {
            r.error = Some(e.to_string());
            return r;
        }
    }
    let refs = rasterize(&reference, dpi, &work.join("ref"));
    let cands = rasterize(&cand_pdf, dpi, &work.join("cand"));
    let (refs, cands) = match (refs, cands) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            r.error = Some(e);
            return r;
        }
    };
    r.ref_pages = refs.len();
    r.cand_pages = cands.len();
    for (i, (a, b)) in refs.iter().zip(&cands).enumerate() {
        let (score, map) = compare(a, b, i + 1, dpi);
        let stem = diffs.join(format!("{case}-p{}", i + 1));
        let _ = write_png(a, &stem.with_extension("ref.png"));
        let _ = write_png(b, &stem.with_extension("cand.png"));
        let _ = write_png(&map, &stem.with_extension("diff.png"));
        r.pages.push(score);
    }
    // Cor (GD-FID-12): candidato PDF/X-1a contra ref-cmyk.pdf.
    let ref_cmyk = dir.join("ref-cmyk.pdf");
    if ref_cmyk.is_file() {
        let cmyk = work.join("candidato-cmyk.pdf");
        let opts = crate::export::ExportOptions {
            text_to_curves: true,
            pdf: crate::export::PdfStandard::X1a(Default::default()),
            ..Default::default()
        };
        if let Ok(b) = crate::export::pdf_document(&doc, &opts)
            && std::fs::write(&cmyk, b).is_ok()
            && let (Some(a), Some(b)) = (pdf_cmyk_colors(&ref_cmyk), pdf_cmyk_colors(&cmyk))
        {
            for (i, p) in r.pages.iter_mut().enumerate() {
                let empty = Vec::new();
                p.cmyk_pp = cmyk_deviation(a.get(i).unwrap_or(&empty), b.get(i).unwrap_or(&empty));
                p.level = p.level.with_cmyk(p.cmyk_pp);
            }
        }
    }
    // Texto (GD-FID-13): candidato com texto vivo contra ref-texto.pdf.
    let ref_text = dir.join("ref-texto.pdf");
    if ref_text.is_file() {
        let live = work.join("candidato-texto.pdf");
        if let Ok(b) = crate::export::pdf_document(&doc, &Default::default())
            && std::fs::write(&live, b).is_ok()
        {
            r.text_equal = match (pdf_text(&ref_text), pdf_text(&live)) {
                (Some(a), Some(b)) => Some(a == b),
                _ => None,
            };
            r.ref_fonts = pdf_fonts(&ref_text).unwrap_or_default();
            r.cand_fonts = pdf_fonts(&live).unwrap_or_default();
            let have: Vec<String> = r.cand_fonts.iter().map(|f| font_key(f)).collect();
            r.substituted_fonts = r
                .ref_fonts
                .iter()
                .filter(|f| !have.contains(&font_key(f)))
                .cloned()
                .collect();
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    r
}

/// Roda o corpus inteiro (`<corpus>/casos/*`) e grava em
/// `<corpus>/resultados/<data>/` o relatório (HTML e JSON), as imagens e o
/// resumo público (só números).
pub fn run(corpus: &Path, dpi: u32, date: &str) -> Result<(Report, PathBuf), String> {
    let cases_dir = corpus.join("casos");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cases_dir)
        .map_err(|e| format!("{}: {e}", cases_dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir() && find_original(p).is_some() && p.join("ref-curvas.pdf").is_file())
        .collect();
    dirs.sort();
    let out = corpus.join("resultados").join(date);
    let diffs = out.join("diffs");
    std::fs::create_dir_all(&diffs).map_err(|e| e.to_string())?;
    let previous = previous_summary(&corpus.join("resultados"), date);
    let cases: Vec<CaseResult> = dirs.iter().map(|d| run_case(d, dpi, &diffs)).collect();
    let mut summary = Summary::of(&cases);
    summary.trend_fiel_pp = previous.map(|p| summary.fiel_pct - p.fiel_pct);
    let report = Report {
        date: date.into(),
        dpi,
        cases,
        summary: summary.clone(),
    };
    let json = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(out.join("relatorio.json"), json).map_err(|e| e.to_string())?;
    std::fs::write(out.join("relatorio.html"), html(&report)).map_err(|e| e.to_string())?;
    std::fs::write(
        out.join("resumo-publico.json"),
        serde_json::to_vec_pretty(&summary).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok((report, out))
}

fn previous_summary(results: &Path, current: &str) -> Option<Summary> {
    let mut runs: Vec<PathBuf> = std::fs::read_dir(results)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().as_ref() < current)
        })
        .collect();
    runs.sort();
    let last = runs.last()?;
    let r: Report =
        serde_json::from_slice(&std::fs::read(last.join("relatorio.json")).ok()?).ok()?;
    Some(r.summary)
}

/// Tabelas por categoria e por versão.
fn groups(s: &Summary) -> String {
    let table = |title: &str, m: &BTreeMap<String, Counts>| {
        if m.is_empty() {
            return String::new();
        }
        let rows: String = m
            .iter()
            .map(|(k, c)| {
                let pct = |n: usize| n as f64 * 100.0 / c.pages.max(1) as f64;
                format!(
                    "<tr><td>{}</td><td>{}</td><td class=fiel>{:.0}%</td><td class=aceitavel>{:.0}%</td><td class=falha>{:.0}%</td></tr>",
                    esc(k),
                    c.pages,
                    pct(c.fiel),
                    pct(c.aceitavel),
                    pct(c.falha)
                )
            })
            .collect();
        format!(
            "<h2>Por {title}</h2><table><tr><th>{title}</th><th>Páginas</th><th>Fiel</th><th>Aceitável</th><th>Falha</th></tr>{rows}</table>"
        )
    };
    format!(
        "{}{}<h2>Casos</h2>",
        table("categoria", &s.by_category),
        table("versão", &s.by_version)
    )
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Relatório HTML privado (GD-FID-30/31): resumo, tabela por caso e
/// imagens lado a lado (referência | candidato | diferença).
pub fn html(r: &Report) -> String {
    let s = &r.summary;
    let mut h = format!(
        "<!doctype html><html lang=\"pt-BR\"><meta charset=\"utf-8\"><title>Fidelidade .cdr — {}</title>\
<style>body{{font:14px system-ui,sans-serif;margin:24px;color:#1b1b1f}}table{{border-collapse:collapse}}td,th{{border:1px solid #ccc;padding:4px 8px}}\
.fiel{{color:#15803d}}.aceitavel{{color:#b45309}}.falha{{color:#b91c1c;font-weight:600}}img{{max-width:32%;border:1px solid #ccc;margin:2px}}</style>\
<h1>Fidelidade de importação .cdr</h1><p>{} · {} ppp · <b>privado</b>: contém imagens dos trabalhos.</p>\
<p>Casos: {} · páginas: {} · <span class=fiel>Fiel {:.1}%</span> · Fiel ou aceitável {:.1}% · travamentos: {}{}</p>\
<p>Metas MVP (GD-FID-20): ≥ 80% Fiel, ≥ 95% Fiel ou aceitável, 0 travamentos.</p>{}\
<table><tr><th>Caso</th><th>Categoria</th><th>Versão</th><th>Abriu</th><th>ms</th><th>Páginas (ref/cand)</th><th>Texto</th><th>Páginas</th></tr>",
        esc(&r.date),
        esc(&r.date),
        r.dpi,
        s.cases,
        s.pages,
        s.fiel_pct,
        s.fiel_ou_aceitavel_pct,
        s.crashes,
        s.trend_fiel_pp
            .map(|t| format!(" · tendência {t:+.1} pp"))
            .unwrap_or_default(),
        groups(s)
    );
    for c in &r.cases {
        let pages: Vec<String> = c
            .pages
            .iter()
            .map(|p| {
                let class = match p.level {
                    Level::Fiel => "fiel",
                    Level::Aceitavel => "aceitavel",
                    Level::Falha => "falha",
                };
                format!(
                    "<span class={class}>p{} {} ({:.2}%, SSIM {:.3}, geometria {:.2} mm, desvio {}×{} px{}{})</span>",
                    p.page,
                    p.level.label(),
                    p.diff_pct,
                    p.ssim,
                    p.geometry_mm,
                    p.shift[0],
                    p.shift[1],
                    p.cmyk_pp.map(|v| format!(", CMYK {v:.1} pp")).unwrap_or_default(),
                    if p.size_mismatch {
                        ", tamanho diferente"
                    } else {
                        ""
                    }
                )
            })
            .collect();
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}/{}</td><td>{}</td><td>{}</td></tr>",
            esc(&c.case),
            esc(&c.meta.categoria),
            esc(&c.meta.versao_cdr),
            if c.opened { "sim".to_string() } else { format!("<span class=falha>não: {}</span>", esc(c.error.as_deref().unwrap_or(""))) },
            c.open_ms,
            c.ref_pages,
            c.cand_pages,
            {
                let t = match c.text_equal {
                    Some(true) => "igual".to_string(),
                    Some(false) => "<span class=falha>diferente</span>".to_string(),
                    None => "—".to_string(),
                };
                let subs = c.substituted_fonts.iter().chain(&c.missing_fonts).map(|f| esc(f)).collect::<Vec<_>>();
                if subs.is_empty() {
                    t
                } else {
                    format!("{t}<br><span class=falha>fontes: {}</span>", subs.join(", "))
                }
            },
            pages.join("<br>")
        ));
    }
    h.push_str("</table>");
    for c in &r.cases {
        for p in &c.pages {
            let stem = format!("diffs/{}-p{}", esc(&c.case), p.page);
            h.push_str(&format!(
                "<h3>{} · página {} · {}</h3><img src=\"{stem}.ref.png\" alt=\"referência\"><img src=\"{stem}.cand.png\" alt=\"candidato\"><img src=\"{stem}.diff.png\" alt=\"diferença\">",
                esc(&c.case),
                p.page,
                p.level.label()
            ));
        }
    }
    h.push_str("</html>");
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_e2000_reference_pairs() {
        // Pares de teste publicados com a fórmula (Sharma, Wu e Dalal, 2005).
        let cases = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            (
                [60.2574, -34.0099, 36.2677],
                [60.4626, -34.1751, 39.4387],
                1.2644,
            ),
        ];
        for (a, b, want) in cases {
            let got = delta_e2000(a, b);
            assert!((got - want).abs() < 1e-3, "{a:?} {b:?}: {got} × {want}");
        }
        assert_eq!(delta_e2000([50.0, 0.0, 0.0], [50.0, 0.0, 0.0]), 0.0);
    }

    fn flat(w: u32, h: u32, c: [u8; 3]) -> Raster {
        Raster {
            width: w,
            height: h,
            rgb: (0..w * h).flat_map(|_| c).collect(),
        }
    }

    #[test]
    fn compare_levels() {
        let white = flat(40, 40, [255, 255, 255]);
        let (s, _) = compare(&white, &white.clone(), 1, 150);
        assert_eq!((s.diff_pct, s.level), (0.0, Level::Fiel));
        assert!((s.ssim - 1.0).abs() < 1e-9);
        // Um quarto da página trocado de cor: falha.
        let mut half = white.clone();
        for y in 0..20 {
            for x in 0..20 {
                let i = ((y * 40 + x) * 3) as usize;
                half.rgb[i..i + 3].copy_from_slice(&[0, 0, 200]);
            }
        }
        let (s, map) = compare(&white, &half, 1, 150);
        assert!((s.diff_pct - 25.0).abs() < 1e-9);
        assert_eq!(s.level, Level::Falha);
        assert_eq!(map.px(0, 0), [220, 30, 30]);
        assert_eq!(s.shift, [0, 0], "empurrar a tinta para fora não vale");
        // Tamanhos diferentes: reamostra, avisa e conta como geometria.
        let (s, _) = compare(&white, &flat(20, 20, [255, 255, 255]), 1, 254);
        assert!(s.size_mismatch && s.level == Level::Falha);
        assert!((s.geometry_mm - 2.0).abs() < 1e-9, "{}", s.geometry_mm);
    }

    /// GD-FID-10/11: o candidato 2 px deslocado alinha (sem diferença
    /// visual), mas o deslocamento conta na geometria.
    #[test]
    fn alignment_and_geometry() {
        let mut a = flat(60, 60, [255, 255, 255]);
        let mut b = a.clone();
        let paint = |r: &mut Raster, x0: u32, y0: u32| {
            for y in y0..y0 + 10 {
                for x in x0..x0 + 10 {
                    let i = ((y * 60 + x) * 3) as usize;
                    r.rgb[i..i + 3].copy_from_slice(&[0, 0, 0]);
                }
            }
        };
        paint(&mut a, 20, 20);
        paint(&mut b, 22, 21);
        let (s, _) = compare(&a, &b, 1, 254);
        assert_eq!(s.shift, [-2, -1]);
        assert_eq!(s.diff_pct, 0.0);
        assert!((s.geometry_mm - 0.2).abs() < 1e-9, "{}", s.geometry_mm);
        assert_eq!(s.level, Level::Aceitavel);
    }

    /// GD-FID-12: desvio CMYK pela cor mais próxima, em pontos percentuais;
    /// rebaixa o nível acima de 1 e de 3 pp.
    #[test]
    fn cmyk_deviation_and_level() {
        let r = [[0.0, 1.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        let c = [[0.0, 0.98, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        assert!((cmyk_deviation(&r, &c).unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(cmyk_deviation(&[], &c), None);
        assert_eq!(cmyk_deviation(&r, &[]), Some(100.0));
        assert_eq!(Level::Fiel.with_cmyk(Some(2.0)), Level::Aceitavel);
        assert_eq!(Level::Falha.with_cmyk(Some(0.5)), Level::Falha);
        assert_eq!(Level::Fiel.with_cmyk(Some(5.0)), Level::Falha);
    }
}
