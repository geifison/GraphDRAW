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
    use super::*;
    use crate::model::{Document, Page, RectData, Stroke};

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
