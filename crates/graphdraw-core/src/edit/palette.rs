//! Paletas (GD-PAL-01..03): cores do documento (`resources/colors.json`),
//! cores globais (objetos com `ref` acompanham a definição), cores usadas,
//! e arquivos de paleta — formato próprio (JSON), GIMP (GPL), CSV e Adobe
//! Swatch Exchange (ASE, formato público de troca de amostras).

use serde::{Deserialize, Serialize};

use super::history::Editor;
use crate::model::{Color, Fill, NamedColor, Object, ObjectKind, new_id};

// ---- cores do documento -----------------------------------------------------

/// Acrescenta uma cor nomeada ao documento; devolve o ID. Com `global`, a
/// cor guardada leva `ref` para si mesma (objetos que a usarem acompanham).
pub fn add_named(ed: &mut Editor, name: &str, color: Color) -> String {
    let id = new_id();
    let mut c = color;
    c.reference = None;
    let nc = NamedColor {
        id: id.clone(),
        name: name.into(),
        color: c,
        extra: Default::default(),
    };
    ed.transaction("Nova cor da paleta", |ed| {
        ed.edit_colors(|cs| cs.colors.push(nc))
    });
    id
}

pub fn remove_named(ed: &mut Editor, id: &str) {
    ed.transaction("Remover cor da paleta", |ed| {
        ed.edit_colors(|cs| cs.colors.retain(|c| c.id != id))
    });
}

pub fn rename_named(ed: &mut Editor, id: &str, name: &str) {
    ed.transaction("Renomear cor", |ed| {
        ed.edit_colors(|cs| {
            if let Some(c) = cs.colors.iter_mut().find(|c| c.id == id) {
                c.name = name.into();
            }
        })
    });
}

/// Cor ligada à cor global `id` (para aplicar em objetos).
pub fn linked(ed: &Editor, id: &str) -> Option<Color> {
    let nc = ed.doc.colors.colors.iter().find(|c| c.id == id)?;
    let mut c = nc.color.clone();
    c.reference = Some(id.into());
    Some(c)
}

fn visit_colors(o: &mut Object, f: &mut dyn FnMut(&mut Color) -> bool) -> bool {
    let mut changed = false;
    match &mut o.fill {
        Some(Fill::Solid { color, .. }) => changed |= f(color),
        Some(Fill::Gradient(g)) => {
            for s in &mut g.stops {
                changed |= f(&mut s.color);
            }
        }
        _ => {}
    }
    if let Some(s) = &mut o.stroke {
        changed |= f(&mut s.color);
    }
    match &mut o.kind {
        ObjectKind::Group(g) => {
            for c in &mut g.children {
                changed |= visit_colors(c, f);
            }
        }
        ObjectKind::Text(t) => {
            for p in &mut t.paragraphs {
                for s in &mut p.spans {
                    if let Some(Fill::Solid { color, .. }) = &mut s.fill {
                        changed |= f(color);
                    }
                }
            }
        }
        _ => {}
    }
    changed
}

/// Converte as cores de todo o documento de `from` para os perfis de `to`
/// e passa a usar `to` (GD-CMS-04). Devolve quantas cores mudaram; um
/// passo de desfazer.
pub fn convert_colors(ed: &mut Editor, to: &crate::cms::ColorManagement) -> usize {
    let from = ed.doc.data.settings.color.clone();
    let conv = crate::cms::Converter::new(&from, to);
    let mut n = 0;
    let to = to.clone();
    ed.transaction("Converter para o perfil de trabalho", |ed| {
        if !conv.is_noop() {
            for page in 0..ed.doc.data.pages.len() {
                for layer in 0..ed.doc.data.pages[page].layers.len() {
                    for index in 0..ed.doc.data.pages[page].layers[layer].objects.len() {
                        let loc = super::history::ObjLoc { page, layer, index };
                        let mut o = ed.object(loc).unwrap().clone();
                        let mut count = 0;
                        if visit_colors(&mut o, &mut |c| {
                            let ch = conv.convert(c);
                            count += ch as usize;
                            ch
                        }) {
                            n += count;
                            ed.replace_object(loc, o);
                        }
                    }
                }
            }
        }
        ed.edit_settings(|s| s.color = to);
    });
    n
}

/// Troca a definição da cor global `id` (GD-PAL-03): a paleta e todas as
/// cores de objetos com `ref` = `id`, em todas as páginas. Um passo.
pub fn set_named(ed: &mut Editor, id: &str, color: Color) {
    let mut def = color;
    def.reference = None;
    let mut used = def.clone();
    used.reference = Some(id.into());
    ed.transaction("Alterar cor global", |ed| {
        ed.edit_colors(|cs| {
            if let Some(c) = cs.colors.iter_mut().find(|c| c.id == id) {
                c.color = def.clone();
            }
        });
        let pages = ed.doc.data.pages.len();
        for p in 0..pages {
            let layers = ed.doc.data.pages[p].layers.len();
            for l in 0..layers {
                let n = ed.doc.data.pages[p].layers[l].objects.len();
                for i in 0..n {
                    let loc = super::history::ObjLoc {
                        page: p,
                        layer: l,
                        index: i,
                    };
                    let Some(mut o) = ed.object(loc).cloned() else {
                        continue;
                    };
                    let changed = visit_colors(&mut o, &mut |c| {
                        if c.reference.as_deref() == Some(id) && *c != used {
                            *c = used.clone();
                            true
                        } else {
                            false
                        }
                    });
                    if changed {
                        ed.replace_object(loc, o);
                    }
                }
            }
        }
    });
}

/// Cores usadas no documento (preenchimentos, contornos, paradas e
/// trechos de texto), sem repetição, na ordem em que aparecem (GD-PAL-01).
pub fn colors_used(ed: &Editor) -> Vec<Color> {
    let mut out: Vec<Color> = Vec::new();
    for page in &ed.doc.data.pages {
        for layer in &page.layers {
            for o in &layer.objects {
                let mut o = o.clone();
                visit_colors(&mut o, &mut |c| {
                    let mut k = c.clone();
                    k.reference = None;
                    if !out.contains(&k) {
                        out.push(k);
                    }
                    false
                });
            }
        }
    }
    out
}

// ---- paletas prontas para produção (GD-PAL-06) -------------------------------

/// Cores de processo em CMYK: os valores vão direto para as chapas, sem
/// conversão, então saem iguais em qualquer gráfica que imprima no padrão
/// (a tela mostra pela simulação do perfil CMYK do documento). Tons de cada
/// tinta e das secundárias, cinzas em K, preto rico e cores comuns; nenhuma
/// passa de 300% de tinta total (limite do FOGRA39).
pub fn process_cmyk_palette() -> Palette {
    let mut colors = Vec::new();
    let mut add = |name: String, c: f64, m: f64, y: f64, k: f64| {
        colors.push(PaletteColor {
            name,
            color: Color::cmyk(c / 100.0, m / 100.0, y / 100.0, k / 100.0),
        });
    };
    let inks: [(&str, [f64; 4]); 7] = [
        ("Ciano", [100.0, 0.0, 0.0, 0.0]),
        ("Magenta", [0.0, 100.0, 0.0, 0.0]),
        ("Amarelo", [0.0, 0.0, 100.0, 0.0]),
        ("Vermelho", [0.0, 100.0, 100.0, 0.0]),
        ("Verde", [100.0, 0.0, 100.0, 0.0]),
        ("Azul", [100.0, 100.0, 0.0, 0.0]),
        ("Preto", [0.0, 0.0, 0.0, 100.0]),
    ];
    for (name, v) in inks {
        for pct in [100.0, 80.0, 60.0, 40.0, 20.0, 10.0] {
            let s = pct / 100.0;
            let label = if pct == 100.0 {
                name.to_string()
            } else {
                format!("{name} {pct:.0}%")
            };
            add(label, v[0] * s, v[1] * s, v[2] * s, v[3] * s);
        }
    }
    for (name, [c, m, y, k]) in [
        ("Preto rico", [60.0, 40.0, 40.0, 100.0]),
        ("Laranja", [0.0, 50.0, 100.0, 0.0]),
        ("Ouro", [0.0, 20.0, 100.0, 10.0]),
        ("Vinho", [0.0, 100.0, 60.0, 40.0]),
        ("Marrom", [0.0, 60.0, 100.0, 50.0]),
        ("Rosa", [0.0, 60.0, 0.0, 0.0]),
        ("Roxo", [70.0, 100.0, 0.0, 0.0]),
        ("Azul-marinho", [100.0, 80.0, 0.0, 30.0]),
        ("Azul-celeste", [60.0, 10.0, 0.0, 0.0]),
        ("Verde-limão", [40.0, 0.0, 100.0, 0.0]),
        ("Verde-bandeira", [90.0, 0.0, 100.0, 20.0]),
        ("Verde-escuro", [90.0, 30.0, 100.0, 40.0]),
        ("Turquesa", [80.0, 0.0, 40.0, 0.0]),
        ("Bege", [0.0, 10.0, 30.0, 5.0]),
    ] {
        add(name.to_string(), c, m, y, k);
    }
    Palette {
        name: "CMYK de processo".into(),
        colors,
    }
}

/// Cores RGB que imprimem como aparecem: cada matiz em vários tons, levada
/// para dentro da gama do perfil CMYK (ida e volta pelo perfil), sem
/// repetidas. Sem perfil CMYK disponível, usa o FOGRA39 embutido.
pub fn print_safe_rgb_palette(cms: Option<&crate::cms::Cms>) -> Palette {
    let own;
    let cms = match cms.filter(|c| c.has_cmyk_profile()) {
        Some(c) => c,
        None => {
            own = crate::cms::Cms::new(&crate::cms::ColorManagement {
                cmyk_profile: Some(crate::cms::FOGRA39.into()),
                ..Default::default()
            });
            &own
        }
    };
    let hues = [
        (0.0, "Vermelho"),
        (30.0, "Laranja"),
        (55.0, "Amarelo"),
        (90.0, "Verde-limão"),
        (130.0, "Verde"),
        (170.0, "Turquesa"),
        (195.0, "Ciano"),
        (215.0, "Azul-celeste"),
        (240.0, "Azul"),
        (270.0, "Violeta"),
        (300.0, "Roxo"),
        (330.0, "Rosa"),
    ];
    let tones = [
        (1.0, 1.0, ""),
        (0.85, 0.75, " escuro"),
        (0.7, 0.5, " profundo"),
        (0.55, 1.0, " médio"),
        (0.3, 1.0, " claro"),
    ];
    let mut out: Vec<PaletteColor> = Vec::new();
    let mut push = |name: String, rgb: [f64; 3]| {
        let Some(p) = cms.to_print_gamut(rgb) else {
            return;
        };
        if out.iter().all(|c| {
            let q = [c.color.v(0), c.color.v(1), c.color.v(2)];
            crate::cms::delta_e(q, p) > 3.0
        }) {
            out.push(PaletteColor {
                name,
                color: Color::rgb(p[0], p[1], p[2]),
            });
        }
    };
    for (h, name) in hues {
        for (s, v, suffix) in tones {
            push(format!("{name}{suffix}"), hsv(h, s, v));
        }
    }
    for k in [0.0, 0.2, 0.4, 0.6, 0.8, 0.9, 1.0] {
        let g = 1.0 - k;
        push(format!("Cinza {:.0}%", k * 100.0), [g, g, g]);
    }
    Palette {
        name: "RGB seguro para impressão".into(),
        colors: out,
    }
}

fn hsv(h: f64, s: f64, v: f64) -> [f64; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

// ---- tons derivados (GD-PAL-07) ----------------------------------------------

/// Colunas do mosaico de tons derivados.
pub const TONE_COLUMNS: usize = 6;

/// A cor é só preto (K puro, cinza, registro ou RGB neutro)?
fn black_only(c: &Color) -> bool {
    let v = |i| c.v(i);
    match c.model.as_str() {
        "cmyk" => v(0) < 1e-9 && v(1) < 1e-9 && v(2) < 1e-9,
        "gray" | "registration" => true,
        "rgb" => (v(0) - v(1)).abs() < 1e-9 && (v(1) - v(2)).abs() < 1e-9,
        _ => false,
    }
}

/// Mesma cor com `s` (0–1) da sua tinta: CMYK, cor especial e registro
/// escalam as tintas; cinza e RGB clareiam em direção ao branco.
fn with_strength(c: &Color, s: f64) -> Color {
    let mut n = c.clone();
    n.reference = None;
    match c.model.as_str() {
        "gray" | "rgb" => {
            for v in &mut n.values {
                *v = 1.0 - (1.0 - *v) * s;
            }
        }
        _ => {
            for v in &mut n.values {
                *v *= s;
            }
        }
    }
    n
}

/// Tons derivados de `c` (GD-PAL-07), do mais claro ao mais escuro.
///
/// - Cor só de preto (e cor especial, que não escurece sem trocar de tinta):
///   6 tons, da cor como está até zero dela.
/// - As outras: 36 (mosaico 6×6) — 17 clareadas, a própria cor e 18
///   escurecidas. CMYK fica em CMYK (escurece com K, sem passar de 300% de
///   tinta); os demais modelos viram RGB.
pub fn derived_tones(c: &Color) -> Vec<Color> {
    if black_only(c) || c.model == "spot" {
        return (0..TONE_COLUMNS)
            .map(|n| with_strength(c, 1.0 - n as f64 / (TONE_COLUMNS - 1) as f64))
            .collect();
    }
    const LIGHT: usize = 17;
    const DARK: usize = 18;
    if c.model == "cmyk" {
        let [cy, m, y, k] = [c.v(0), c.v(1), c.v(2), c.v(3)];
        let mut out: Vec<Color> = (0..LIGHT)
            .map(|i| with_strength(c, (i + 1) as f64 / (LIGHT + 1) as f64))
            .collect();
        out.push(Color::cmyk(cy, m, y, k));
        for j in 1..=DARK {
            let d = j as f64 / DARK as f64 * 0.85;
            let k2 = k + (1.0 - k) * d;
            // Tinta total até 300%: o que passar sai do CMY.
            let cmy = cy + m + y;
            let room = (3.0 - k2).max(0.0);
            let f = if cmy > room { room / cmy } else { 1.0 };
            out.push(Color::cmyk(cy * f, m * f, y * f, k2));
        }
        return out;
    }
    let rgb = if c.model == "rgb" {
        [c.v(0), c.v(1), c.v(2)]
    } else {
        c.to_srgb()
    };
    let base = Color::rgb(rgb[0], rgb[1], rgb[2]);
    let mut out: Vec<Color> = (0..LIGHT)
        .map(|i| with_strength(&base, (i + 1) as f64 / (LIGHT + 1) as f64))
        .collect();
    out.push(base);
    for j in 1..=DARK {
        let d = j as f64 / DARK as f64 * 0.85;
        out.push(Color::rgb(
            rgb[0] * (1.0 - d),
            rgb[1] * (1.0 - d),
            rgb[2] * (1.0 - d),
        ));
    }
    out
}

// ---- arquivos de paleta (GD-PAL-02) ------------------------------------------

/// Paleta de arquivo: nome e cores nomeadas (o modelo original é mantido).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub name: String,
    pub colors: Vec<PaletteColor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteColor {
    pub name: String,
    pub color: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteFormat {
    Json,
    Gpl,
    Csv,
    Ase,
}

impl PaletteFormat {
    pub fn from_extension(ext: &str) -> Option<PaletteFormat> {
        Some(match ext.to_ascii_lowercase().as_str() {
            "json" | "gdpal" => PaletteFormat::Json,
            "gpl" => PaletteFormat::Gpl,
            "csv" => PaletteFormat::Csv,
            "ase" => PaletteFormat::Ase,
            _ => return None,
        })
    }
}

pub fn read_palette(
    bytes: &[u8],
    fmt: PaletteFormat,
    fallback_name: &str,
) -> Result<Palette, String> {
    match fmt {
        PaletteFormat::Json => serde_json::from_slice(bytes).map_err(|e| e.to_string()),
        PaletteFormat::Gpl => read_gpl(&String::from_utf8_lossy(bytes), fallback_name),
        PaletteFormat::Csv => read_csv(&String::from_utf8_lossy(bytes), fallback_name),
        PaletteFormat::Ase => read_ase(bytes, fallback_name),
    }
}

pub fn write_palette(p: &Palette, fmt: PaletteFormat) -> Vec<u8> {
    match fmt {
        PaletteFormat::Json => serde_json::to_vec_pretty(p).unwrap_or_default(),
        PaletteFormat::Gpl => write_gpl(p).into_bytes(),
        PaletteFormat::Csv => write_csv(p).into_bytes(),
        PaletteFormat::Ase => write_ase(p),
    }
}

fn read_gpl(t: &str, fallback: &str) -> Result<Palette, String> {
    let mut lines = t.lines();
    if !lines.next().is_some_and(|l| l.trim() == "GIMP Palette") {
        return Err("não é uma paleta GIMP (falta \"GIMP Palette\")".into());
    }
    let mut name = fallback.to_string();
    let mut colors = Vec::new();
    for l in lines {
        let l = l.trim();
        if let Some(n) = l.strip_prefix("Name:") {
            name = n.trim().into();
            continue;
        }
        if l.is_empty() || l.starts_with('#') || l.starts_with("Columns:") {
            continue;
        }
        let mut it = l.split_whitespace();
        let (Some(r), Some(g), Some(b)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let v = |s: &str| s.parse::<f64>().map(|v| (v / 255.0).clamp(0.0, 1.0));
        if let (Ok(r), Ok(g), Ok(b)) = (v(r), v(g), v(b)) {
            let n: Vec<&str> = it.collect();
            colors.push(PaletteColor {
                name: n.join(" "),
                color: Color::rgb(r, g, b),
            });
        }
    }
    Ok(Palette { name, colors })
}

fn write_gpl(p: &Palette) -> String {
    let mut s = format!("GIMP Palette\nName: {}\n#\n", p.name);
    for c in &p.colors {
        let [r, g, b] = c.color.to_srgb();
        let k = |v: f64| (v * 255.0).round() as i64;
        s.push_str(&format!("{:3} {:3} {:3}\t{}\n", k(r), k(g), k(b), c.name));
    }
    s
}

/// CSV: `nome,modelo,v1,v2,…` (valores na faixa do modelo, ver formato).
fn read_csv(t: &str, fallback: &str) -> Result<Palette, String> {
    let mut colors = Vec::new();
    for (i, l) in t.lines().enumerate() {
        let f: Vec<&str> = l.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if f.len() < 3
            || (i == 0 && f[0].eq_ignore_ascii_case("nome") || f[0].eq_ignore_ascii_case("name"))
        {
            continue;
        }
        let values: Option<Vec<f64>> = f[2..].iter().map(|v| v.parse().ok()).collect();
        let Some(values) = values else { continue };
        colors.push(PaletteColor {
            name: f[0].into(),
            color: Color::new(&f[1].to_ascii_lowercase(), values),
        });
    }
    Ok(Palette {
        name: fallback.into(),
        colors,
    })
}

fn write_csv(p: &Palette) -> String {
    let mut s = String::from("nome,modelo,valores\n");
    for c in &p.colors {
        let v: Vec<String> = c.color.values.iter().map(|v| format!("{v}")).collect();
        let name = c.name.replace(',', " ");
        s.push_str(&format!("{name},{},{}\n", c.color.model, v.join(",")));
    }
    s
}

fn be16(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?))
}
fn be32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}
fn bef32(b: &[u8], i: usize) -> Option<f32> {
    Some(f32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// ASE: "ASEF", versão 1.0, nº de blocos; cada bloco = tipo (0x0001 cor,
/// 0xC001 início de grupo, 0xC002 fim), tamanho, nome UTF-16BE com
/// terminador, modelo (`RGB `, `CMYK`, `Gray`, `LAB `), valores float32 e
/// tipo de cor (0 global, 1 especial, 2 normal).
fn read_ase(b: &[u8], fallback: &str) -> Result<Palette, String> {
    if b.get(0..4) != Some(b"ASEF") {
        return Err("não é um arquivo ASE".into());
    }
    let count = be32(b, 8).ok_or("ASE curto")? as usize;
    let mut i = 12;
    let mut colors = Vec::new();
    let mut name = fallback.to_string();
    for _ in 0..count {
        let kind = be16(b, i).ok_or("ASE truncado")?;
        let len = be32(b, i + 2).ok_or("ASE truncado")? as usize;
        let body = i + 6;
        let next = body + len;
        if next > b.len() {
            return Err("ASE truncado".into());
        }
        if kind == 0x0001 || kind == 0xC001 {
            let n = be16(b, body).ok_or("ASE truncado")? as usize;
            let units: Vec<u16> = (0..n).filter_map(|k| be16(b, body + 2 + 2 * k)).collect();
            let label = String::from_utf16_lossy(&units)
                .trim_end_matches('\0')
                .to_string();
            if kind == 0xC001 {
                if !label.is_empty() {
                    name = label;
                }
            } else {
                let at = body + 2 + 2 * n;
                let model = b.get(at..at + 4).ok_or("ASE truncado")?;
                let v = |k: usize| bef32(b, at + 4 + 4 * k).map(|x| x as f64);
                let (m, vals) = match model {
                    b"RGB " => ("rgb", vec![v(0), v(1), v(2)]),
                    b"CMYK" => ("cmyk", vec![v(0), v(1), v(2), v(3)]),
                    b"Gray" => ("gray", vec![v(0)]),
                    // Lab no ASE: L 0–1 (×100), a e b em valores CIE.
                    b"LAB " => ("lab", vec![v(0).map(|l| l * 100.0), v(1), v(2)]),
                    _ => continue,
                };
                let vals: Option<Vec<f64>> = vals.into_iter().collect();
                let Some(vals) = vals else { continue };
                let ty = be16(b, at + 4 + 4 * vals.len()).unwrap_or(2);
                let mut color = Color::new(m, vals);
                if ty == 1 {
                    // Cor especial: a tinta leva o nome, o valor vira a prévia.
                    let mut spot = Color::new("spot", vec![1.0]);
                    spot.name = Some(label.clone());
                    spot.alternate = Some(Box::new(color));
                    color = spot;
                }
                colors.push(PaletteColor { name: label, color });
            }
        }
        i = next;
    }
    Ok(Palette { name, colors })
}

fn write_ase(p: &Palette) -> Vec<u8> {
    let mut blocks: Vec<Vec<u8>> = Vec::new();
    let name16 = |s: &str| {
        let mut v: Vec<u8> = Vec::new();
        let units: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        v.extend((units.len() as u16).to_be_bytes());
        for u in units {
            v.extend(u.to_be_bytes());
        }
        v
    };
    let block = |kind: u16, body: Vec<u8>| {
        let mut v = kind.to_be_bytes().to_vec();
        v.extend((body.len() as u32).to_be_bytes());
        v.extend(body);
        v
    };
    blocks.push(block(0xC001, name16(&p.name)));
    for c in &p.colors {
        let (base, ty) = match c.color.model.as_str() {
            "spot" => (
                c.color
                    .alternate
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(Color::black),
                1u16,
            ),
            _ => (c.color.clone(), 2),
        };
        let (model, vals): (&[u8; 4], Vec<f64>) = match base.model.as_str() {
            "cmyk" => (b"CMYK", base.values.clone()),
            "gray" => (b"Gray", base.values.clone()),
            "lab" => (
                b"LAB ",
                vec![
                    base.values.first().copied().unwrap_or(0.0) / 100.0,
                    base.values.get(1).copied().unwrap_or(0.0),
                    base.values.get(2).copied().unwrap_or(0.0),
                ],
            ),
            "rgb" => (b"RGB ", base.values.clone()),
            _ => (b"RGB ", base.to_srgb().to_vec()),
        };
        let mut body = name16(&c.name);
        body.extend(model);
        for v in vals {
            body.extend((v as f32).to_be_bytes());
        }
        body.extend(ty.to_be_bytes());
        blocks.push(block(0x0001, body));
    }
    blocks.push(block(0xC002, Vec::new()));
    let mut out = b"ASEF".to_vec();
    out.extend(1u16.to_be_bytes());
    out.extend(0u16.to_be_bytes());
    out.extend((blocks.len() as u32).to_be_bytes());
    for b in blocks {
        out.extend(b);
    }
    out
}

/// Paleta com as cores nomeadas do documento.
pub fn document_palette(ed: &Editor, name: &str) -> Palette {
    Palette {
        name: name.into(),
        colors: ed
            .doc
            .colors
            .colors
            .iter()
            .map(|c| PaletteColor {
                name: c.name.clone(),
                color: c.color.clone(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    /// GD-CMS-04: converter para outro perfil RGB muda os valores RGB, passa
    /// a usar o perfil novo e é um passo de desfazer; perfis iguais não
    /// mudam nada.
    #[test]
    fn gd_cms_04_convert_to_working_profile() {
        use crate::cms::ColorManagement;
        use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};
        let d65 = CIExyY {
            x: 0.3127,
            y: 0.3290,
            Y: 1.0,
        };
        let prim = CIExyYTRIPLE {
            Red: CIExyY {
                x: 0.64,
                y: 0.33,
                Y: 1.0,
            },
            Green: CIExyY {
                x: 0.30,
                y: 0.60,
                Y: 1.0,
            },
            Blue: CIExyY {
                x: 0.15,
                y: 0.06,
                Y: 1.0,
            },
        };
        let linear = ToneCurve::new(1.0);
        let icc = Profile::new_rgb(&d65, &prim, &[&linear, &linear, &linear])
            .unwrap()
            .icc()
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("graphdraw-linear-{}.icc", std::process::id()));
        std::fs::write(&path, icc).unwrap();
        let mut d = crate::model::Document::new();
        d.data.pages.push(crate::model::Page::new(10.0, 10.0));
        let mut ed = Editor::new(d);
        let mut o = Object::new(ObjectKind::Rectangle(crate::model::RectData {
            width: 1.0,
            height: 1.0,
            corners: None,
            extra: Default::default(),
        }));
        o.fill = Some(Fill::solid(Color::rgb(0.5, 0.5, 0.5)));
        super::super::ops::add_object(&mut ed, 0, 0, o, "r");
        let to = ColorManagement {
            rgb_profile: Some(path.to_string_lossy().into_owned()),
            ..Default::default()
        };
        assert!(crate::cms::profiles_differ(
            &ed.doc.data.settings.color,
            &to
        ));
        let undo = ed.undo_len();
        assert_eq!(convert_colors(&mut ed, &to), 1);
        assert_eq!(ed.undo_len(), undo + 1);
        let v = match &ed.doc.data.pages[0].layers[0].objects[0].fill {
            Some(Fill::Solid { color, .. }) => color.values[0],
            _ => panic!(),
        };
        // 50% sRGB (≈ 21% de luz) num perfil linear fica perto de 0,21.
        assert!((v - 0.214).abs() < 0.02, "{v}");
        assert_eq!(ed.doc.data.settings.color, to);
        // De novo para o mesmo perfil: nada a converter.
        assert_eq!(convert_colors(&mut ed, &to), 0);
        ed.undo();
        ed.undo();
        assert_eq!(ed.doc.data.settings.color, ColorManagement::default());
        let _ = std::fs::remove_file(path);
    }

    use super::*;
    use crate::model::{Document, Page, RectData, Stroke};

    /// Claridade (L*) pela fórmula do modelo, sem perfil.
    fn lightness(c: &Color) -> f64 {
        let rgb = match c.model.as_str() {
            "cmyk" => {
                let k = c.v(3);
                [0, 1, 2].map(|i| (1.0 - c.v(i)) * (1.0 - k))
            }
            "gray" => [c.v(0); 3],
            _ => [c.v(0), c.v(1), c.v(2)],
        };
        crate::cms::srgb_to_lab(rgb)[0]
    }

    /// GD-PAL-07: 6×6 do mais claro ao mais escuro, com a própria cor no
    /// meio e o modelo mantido; CMYK sem passar de 300%.
    #[test]
    fn gd_pal_07_derived_tones_mosaic() {
        for c in [
            Color::cmyk(0.0, 1.0, 1.0, 0.0),
            Color::cmyk(0.6, 0.4, 0.4, 0.3),
            Color::rgb(0.2, 0.5, 0.9),
            Color::new("hsb", vec![0.3, 0.8, 0.8]),
        ] {
            let t = derived_tones(&c);
            assert_eq!(t.len(), TONE_COLUMNS * TONE_COLUMNS, "{c:?}");
            let want = if c.model == "cmyk" { "cmyk" } else { "rgb" };
            assert!(t.iter().all(|x| x.model == want));
            for w in t.windows(2) {
                assert!(lightness(&w[0]) >= lightness(&w[1]) - 1e-6, "{c:?}: {w:?}");
            }
            assert!(
                lightness(&t[0]) > 90.0,
                "canto superior esquerdo quase branco"
            );
            if c.model == "cmyk" {
                assert_eq!(t[17].values, c.values, "a própria cor no meio");
                assert!(t.iter().all(|x| x.values.iter().sum::<f64>() <= 3.0 + 1e-9));
            }
        }
    }

    /// GD-PAL-07: cor só de preto (e cor especial) dá 6 tons, dela até zero.
    #[test]
    fn gd_pal_07_black_only_scale() {
        let k = derived_tones(&Color::cmyk(0.0, 0.0, 0.0, 1.0));
        let ks: Vec<f64> = k.iter().map(|c| (c.values[3] * 100.0).round()).collect();
        assert_eq!(ks, [100.0, 80.0, 60.0, 40.0, 20.0, 0.0]);
        assert!(
            k.iter()
                .all(|c| c.model == "cmyk" && c.values[..3].iter().all(|v| *v == 0.0))
        );
        let k60 = derived_tones(&Color::cmyk(0.0, 0.0, 0.0, 0.6));
        assert!((k60[0].values[3] - 0.6).abs() < 1e-9 && k60[5].values[3] == 0.0);
        let g = derived_tones(&Color::gray(0.0));
        assert_eq!(g.len(), 6);
        assert!(g[0].values[0] < 1e-9 && (g[5].values[0] - 1.0).abs() < 1e-9);
        assert_eq!(derived_tones(&Color::rgb(0.3, 0.3, 0.3)).len(), 6);
        let mut spot = Color::new("spot", vec![1.0]);
        spot.name = Some("Ouro".into());
        let s = derived_tones(&spot);
        assert_eq!(s.len(), 6);
        assert!(
            s.iter()
                .all(|c| c.model == "spot" && c.name.as_deref() == Some("Ouro"))
        );
    }

    /// GD-PAL-06: a paleta CMYK é toda em CMYK, sem passar de 300% de tinta.
    #[test]
    fn gd_pal_06_process_cmyk_palette() {
        let p = process_cmyk_palette();
        assert!(p.colors.len() > 50);
        for c in &p.colors {
            assert_eq!(c.color.model, "cmyk", "{}", c.name);
            let tac: f64 = c.color.values.iter().sum();
            assert!(tac <= 3.0 + 1e-9, "{}: {tac}", c.name);
        }
        let names: Vec<&str> = p.colors.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"Ciano") && names.contains(&"Preto rico"));
        let mut uniq = names.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(uniq.len(), names.len(), "nomes repetidos");
    }

    /// GD-PAL-06: cada cor da paleta RGB segura imprime como aparece
    /// (ida e volta pelo FOGRA39 com ΔE pequeno) e não há quase-repetidas.
    #[test]
    fn gd_pal_06_print_safe_rgb_palette() {
        let cms = crate::cms::Cms::new(&crate::cms::ColorManagement {
            cmyk_profile: Some(crate::cms::FOGRA39.into()),
            ..Default::default()
        });
        let p = print_safe_rgb_palette(Some(&cms));
        assert!(p.colors.len() > 30, "{}", p.colors.len());
        for c in &p.colors {
            assert_eq!(c.color.model, "rgb");
            let rgb = [c.color.v(0), c.color.v(1), c.color.v(2)];
            assert_eq!(
                cms.in_print_gamut(rgb, 1.0),
                Some(true),
                "{} {rgb:?}",
                c.name
            );
        }
        // Sem perfil, usa o FOGRA39 embutido: mesmo resultado.
        assert_eq!(print_safe_rgb_palette(None), p);
    }

    fn sample() -> Palette {
        let mut spot = Color::new("spot", vec![1.0]);
        spot.name = Some("Laranja".into());
        spot.alternate = Some(Box::new(Color::cmyk(0.0, 0.5, 1.0, 0.0)));
        Palette {
            name: "Teste".into(),
            colors: vec![
                PaletteColor {
                    name: "Vermelho".into(),
                    color: Color::rgb(1.0, 0.0, 0.0),
                },
                PaletteColor {
                    name: "Ciano".into(),
                    color: Color::cmyk(1.0, 0.0, 0.0, 0.0),
                },
                PaletteColor {
                    name: "Cinza".into(),
                    color: Color::gray(0.5),
                },
                PaletteColor {
                    name: "Laranja".into(),
                    color: spot,
                },
            ],
        }
    }

    #[test]
    fn gd_pal_02_formats_round_trip() {
        let p = sample();
        for fmt in [PaletteFormat::Json, PaletteFormat::Ase, PaletteFormat::Csv] {
            let bytes = write_palette(&p, fmt);
            let back = read_palette(&bytes, fmt, "Teste").unwrap();
            assert_eq!(back.colors.len(), 4, "{fmt:?}");
            assert_eq!(
                back.colors[1].color.model, "cmyk",
                "modelo original mantido em {fmt:?}"
            );
            if fmt != PaletteFormat::Csv {
                assert_eq!(back.colors[3].color.model, "spot", "{fmt:?}");
                assert_eq!(back.colors[3].color.name.as_deref(), Some("Laranja"));
            }
        }
        // ASE: valores float32 voltam iguais o bastante.
        let back = read_palette(
            &write_palette(&p, PaletteFormat::Ase),
            PaletteFormat::Ase,
            "x",
        )
        .unwrap();
        assert_eq!(back.name, "Teste");
        assert!((back.colors[0].color.values[0] - 1.0).abs() < 1e-6);
        // GPL é só RGB.
        let g = read_palette(
            &write_palette(&p, PaletteFormat::Gpl),
            PaletteFormat::Gpl,
            "x",
        )
        .unwrap();
        assert_eq!((g.name.as_str(), g.colors.len()), ("Teste", 4));
        assert_eq!(g.colors[0].color, Color::rgb(1.0, 0.0, 0.0));
        assert!(read_palette(b"lixo", PaletteFormat::Ase, "x").is_err());
    }

    #[test]
    fn gd_pal_03_global_color_updates_objects() {
        let mut d = Document::new();
        d.data.pages.push(Page::new(100.0, 100.0));
        d.data.pages.push(Page::new(100.0, 100.0));
        let mut ed = Editor::new(d);
        let id = add_named(&mut ed, "Marca", Color::cmyk(0.0, 1.0, 0.0, 0.0));
        let c = linked(&ed, &id).unwrap();
        for page in 0..2 {
            let mut o = Object::new(ObjectKind::Rectangle(RectData {
                width: 1.0,
                height: 1.0,
                corners: None,
                extra: Default::default(),
            }));
            o.fill = Some(Fill::solid(c.clone()));
            o.stroke = Some(Stroke::new(1.0, Color::black()));
            crate::edit::ops::add_object(&mut ed, page, 0, o, "r");
        }
        set_named(&mut ed, &id, Color::cmyk(1.0, 0.0, 0.0, 0.0));
        for page in 0..2 {
            let o = &ed.doc.data.pages[page].layers[0].objects[0];
            let Some(Fill::Solid { color, .. }) = &o.fill else {
                panic!()
            };
            assert_eq!(color.values, vec![1.0, 0.0, 0.0, 0.0], "página {page}");
            assert_eq!(color.reference.as_deref(), Some(id.as_str()));
            assert_eq!(
                o.stroke.as_ref().unwrap().color,
                Color::black(),
                "contorno sem vínculo não muda"
            );
        }
        assert_eq!(
            ed.doc.colors.colors[0].color.values,
            vec![1.0, 0.0, 0.0, 0.0]
        );
        ed.undo();
        let o = &ed.doc.data.pages[1].layers[0].objects[0];
        let Some(Fill::Solid { color, .. }) = &o.fill else {
            panic!()
        };
        assert_eq!(
            color.values,
            vec![0.0, 1.0, 0.0, 0.0],
            "desfazer volta tudo num passo"
        );
        // Cores usadas: sem repetição.
        assert_eq!(colors_used(&ed).len(), 2);
    }
}
