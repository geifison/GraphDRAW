//! Fidelidade de importação `.cdr` (doc 25, GD-FID-01..42): compara, caixa
//! preta, o PDF de referência exportado pelo dono do arquivo com o PDF que
//! o GraphDraw gera do mesmo original. Os dois passam pelo **mesmo**
//! rasterizador (`pdftoppm`, do Poppler) e são comparados pixel a pixel.
//!
//! Métricas por página: diferença visual (% de pixels com ΔE2000 > 5,
//! GD-FID-10), SSIM, contagem de páginas (GD-FID-14), texto extraído
//! (GD-FID-13, com `pdftotext`) e robustez/tempo de abertura (GD-FID-15).
//! Geometria por objeto (GD-FID-11) e valores CMYK (GD-FID-12) ainda não
//! são medidos.

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
    pub fn of(diff_pct: f64, ssim: f64) -> Level {
        if diff_pct <= 1.0 && ssim >= 0.98 {
            Level::Fiel
        } else if diff_pct <= 5.0 && ssim >= 0.95 {
            Level::Aceitavel
        } else {
            Level::Falha
        }
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
}

/// Resultado da comparação de duas páginas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageScore {
    pub page: usize,
    /// % de pixels com ΔE2000 > 5.
    pub diff_pct: f64,
    pub ssim: f64,
    /// Tamanhos diferentes (o candidato foi reamostrado).
    pub size_mismatch: bool,
    pub level: Level,
}

/// Compara referência e candidato; devolve a nota e o mapa de diferença
/// (pixels diferentes em vermelho sobre a referência clareada).
pub fn compare(reference: &Raster, candidate: &Raster, page: usize) -> (PageScore, Raster) {
    let size_mismatch = reference.width != candidate.width || reference.height != candidate.height;
    let cand = if size_mismatch {
        candidate.resized(reference.width, reference.height)
    } else {
        candidate.clone()
    };
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
    let level = Level::of(diff_pct, ssim);
    (
        PageScore {
            page,
            diff_pct,
            ssim,
            size_mismatch,
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
    pub missing_fonts: Vec<String>,
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
            for p in &c.pages {
                s.pages += 1;
                match p.level {
                    Level::Fiel => s.fiel += 1,
                    Level::Aceitavel => s.aceitavel += 1,
                    Level::Falha => s.falha += 1,
                }
            }
            // Páginas que faltam no candidato contam como falha.
            s.falha += c.ref_pages.saturating_sub(c.pages.len());
            s.pages += c.ref_pages.saturating_sub(c.pages.len());
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
        let (score, map) = compare(a, b, i + 1);
        let stem = diffs.join(format!("{case}-p{}", i + 1));
        let _ = write_png(a, &stem.with_extension("ref.png"));
        let _ = write_png(b, &stem.with_extension("cand.png"));
        let _ = write_png(&map, &stem.with_extension("diff.png"));
        r.pages.push(score);
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
<p>Metas MVP (GD-FID-20): ≥ 80% Fiel, ≥ 95% Fiel ou aceitável, 0 travamentos.</p>\
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
            .unwrap_or_default()
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
                    "<span class={class}>p{} {} ({:.2}%, SSIM {:.3}{})</span>",
                    p.page,
                    p.level.label(),
                    p.diff_pct,
                    p.ssim,
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
            match c.text_equal {
                Some(true) => "igual",
                Some(false) => "<span class=falha>diferente</span>",
                None => "—",
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
        let (s, _) = compare(&white, &white.clone(), 1);
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
        let (s, map) = compare(&white, &half, 1);
        assert!((s.diff_pct - 25.0).abs() < 1e-9);
        assert_eq!(s.level, Level::Falha);
        assert_eq!(map.px(0, 0), [220, 30, 30]);
        // Tamanhos diferentes: reamostra e avisa.
        let (s, _) = compare(&white, &flat(20, 20, [255, 255, 255]), 1);
        assert!(s.size_mismatch && s.level == Level::Fiel);
    }
}
