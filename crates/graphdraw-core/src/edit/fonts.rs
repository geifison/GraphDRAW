//! Fontes ausentes ao abrir (GD-FNT-02/03): quais famílias dos textos não
//! estão instaladas (nem incorporadas no arquivo), uma sugestão de troca
//! pela instalada mais parecida, e as duas saídas — trocar a família nos
//! textos ou converter esses textos em curvas.

use super::history::Editor;
use crate::model::{Document, ObjectKind};

/// Uma família ausente: quantos textos a usam e a troca sugerida.
#[derive(Debug, Clone, PartialEq)]
pub struct MissingFont {
    pub family: String,
    /// IDs dos objetos de texto que a usam.
    pub texts: Vec<String>,
    pub suggestion: Option<String>,
}

/// Famílias usadas nos textos que não estão disponíveis.
pub fn missing_fonts(doc: &Document) -> Vec<MissingFont> {
    let mut map: std::collections::BTreeMap<String, (Vec<String>, u16, bool)> = Default::default();
    doc.walk_objects(&mut |o| {
        if let ObjectKind::Text(t) = &o.kind {
            for p in &t.paragraphs {
                for s in &p.spans {
                    let e = map.entry(s.font.family.clone()).or_insert((
                        Vec::new(),
                        s.font.weight,
                        s.font.italic,
                    ));
                    if !e.0.contains(&o.id) {
                        e.0.push(o.id.clone());
                    }
                }
            }
        }
    });
    let installed = crate::render::text::font_families();
    map.into_iter()
        .filter(|(f, (_, w, i))| {
            crate::render::text::resolve_family(f, *w, *i)
                .is_none_or(|(_, substituted)| substituted)
        })
        .map(|(family, (texts, _, _))| MissingFont {
            suggestion: suggest(&family, &installed),
            family,
            texts,
        })
        .collect()
}

/// Palavras do nome sem as de estilo (Bold, Regular, MT…).
fn tokens(name: &str) -> Vec<String> {
    const NOISE: [&str; 14] = [
        "regular",
        "bold",
        "italic",
        "light",
        "medium",
        "semibold",
        "black",
        "condensed",
        "mt",
        "std",
        "pro",
        "lt",
        "ms",
        "new",
    ];
    name.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty() && !NOISE.contains(&w.as_str()))
        .collect()
}

fn trigrams(s: &str) -> std::collections::HashSet<String> {
    let s: Vec<char> = format!("  {}  ", s.to_lowercase().replace(' ', ""))
        .chars()
        .collect();
    s.windows(3).map(|w| w.iter().collect()).collect()
}

/// Classe da família pelo nome: serifada, monoespaçada ou sem serifa.
fn class(name: &str) -> &'static str {
    let n = name.to_lowercase();
    const SERIF: [&str; 10] = [
        "serif",
        "times",
        "garamond",
        "georgia",
        "roman",
        "book",
        "minion",
        "caslon",
        "baskerville",
        "bodoni",
    ];
    const MONO: [&str; 5] = ["mono", "courier", "code", "consol", "typewriter"];
    if MONO.iter().any(|k| n.contains(k)) {
        "mono"
    } else if SERIF.iter().any(|k| n.contains(k)) && !n.contains("sans") {
        "serif"
    } else {
        "sans"
    }
}

/// A instalada mais parecida com `family`: mesmo radical do nome (ex.:
/// "Helvetica Neue" → "Helvetica"), senão nome parecido da mesma classe,
/// senão uma comum da classe.
pub fn suggest(family: &str, installed: &[String]) -> Option<String> {
    let want = tokens(family);
    let tg = trigrams(family);
    let cls = class(family);
    let mut best: Option<(f64, &String)> = None;
    for f in installed {
        let t = tokens(f);
        let shared = want.iter().filter(|w| t.contains(w)).count() as f64;
        let tf = trigrams(f);
        let inter = tg.intersection(&tf).count() as f64;
        let jac = inter / (tg.len() + tf.len()) as f64 * 2.0;
        let mut score = shared * 2.0 + jac;
        if class(f) == cls {
            score += 0.5;
        }
        if best.is_none_or(|(b, _)| score > b) {
            best = Some((score, f));
        }
    }
    if let Some((s, f)) = best
        && s >= 1.5
    {
        return Some(f.clone());
    }
    let common: &[&str] = match cls {
        "serif" => &[
            "Times New Roman",
            "Georgia",
            "DejaVu Serif",
            "Liberation Serif",
        ],
        "mono" => &[
            "Consolas",
            "Courier New",
            "DejaVu Sans Mono",
            "Liberation Mono",
        ],
        _ => &["Arial", "Segoe UI", "DejaVu Sans", "Liberation Sans"],
    };
    common
        .iter()
        .find_map(|c| {
            installed
                .iter()
                .find(|f| f.eq_ignore_ascii_case(c))
                .cloned()
        })
        .or_else(|| best.map(|(_, f)| f.clone()))
}

/// Troca a família `from` por `to` em todos os trechos dos textos `ids`.
pub fn replace_font(ed: &mut Editor, ids: &[String], from: &str, to: &str) {
    super::ops::modify(ed, ids, "Trocar fonte", |o| {
        let mut any = false;
        if let ObjectKind::Text(t) = &mut o.kind {
            for p in &mut t.paragraphs {
                for s in &mut p.spans {
                    if s.font.family == from {
                        s.font.family = to.into();
                        any = true;
                    }
                }
            }
        }
        any
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn suggestions() {
        let inst = list(&[
            "Arial",
            "Helvetica",
            "Times New Roman",
            "Courier New",
            "Segoe UI",
            "Garamond Premier",
        ]);
        assert_eq!(
            suggest("Helvetica Neue LT Std", &inst).as_deref(),
            Some("Helvetica")
        );
        assert_eq!(
            suggest("Adobe Garamond Pro", &inst).as_deref(),
            Some("Garamond Premier")
        );
        assert_eq!(suggest("Fira Code", &inst).as_deref(), Some("Courier New"));
        assert_eq!(suggest("Minion", &inst).as_deref(), Some("Times New Roman"));
        assert_eq!(suggest("Zzyzx Display", &inst).as_deref(), Some("Arial"));
    }

    #[test]
    fn missing_and_replace() {
        use crate::model::{Document, Object, Page, TextData};
        let mut d = Document::new();
        d.data.pages.push(Page::new(100_000.0, 100_000.0));
        let span = crate::model::TextSpan {
            text: "Oi".into(),
            font: crate::model::FontSpec {
                family: "Fonte Que Nao Existe GD".into(),
                size: 12_000.0,
                weight: 400,
                italic: false,
                extra: Default::default(),
            },
            fill: None,
            extra: Default::default(),
        };
        let t = Object::new(ObjectKind::Text(TextData {
            text_kind: "artistic".into(),
            frame: None,
            paragraphs: vec![crate::model::Paragraph {
                align: "left".into(),
                spans: vec![span],
                hyphenate: false,
                lang: None,
                extra: Default::default(),
            }],
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            path_memory: None,
            extra: Default::default(),
        }));
        let id = t.id.clone();
        d.data.pages[0].layers[0].objects.push(t);
        let mut ed = Editor::new(d);
        let m = missing_fonts(&ed.doc);
        if crate::render::text::font_families().is_empty() {
            return;
        }
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].texts, vec![id.clone()]);
        let to = m[0].suggestion.clone().unwrap();
        replace_font(&mut ed, &m[0].texts, &m[0].family, &to);
        assert!(missing_fonts(&ed.doc).is_empty());
        assert_eq!(ed.undo_len(), 1);
    }
}

#[cfg(test)]
mod embed_tests {
    use crate::model::{
        Document, FontSpec, Object, ObjectKind, Page, Paragraph, TextData, TextSpan,
    };

    /// GD-FNT-03 (a): com a opção ligada, a fonte usada vai inteira para
    /// `resources/fonts/`; desligada, não; ao ler, o membro é preservado.
    #[test]
    fn embeds_whole_font_when_enabled() {
        let families = crate::render::text::font_families();
        let Some(family) = families
            .iter()
            .find(|f| crate::render::text::font_file(f, 400, false).is_some_and(|(_, _, ok)| ok))
            .cloned()
        else {
            return;
        };
        let mut d = Document::new();
        d.data.pages.push(Page::new(100_000.0, 100_000.0));
        let t = Object::new(ObjectKind::Text(TextData {
            text_kind: "artistic".into(),
            frame: None,
            paragraphs: vec![Paragraph {
                align: "left".into(),
                spans: vec![TextSpan {
                    text: "Oi".into(),
                    font: FontSpec {
                        family,
                        size: 12_000.0,
                        weight: 400,
                        italic: false,
                        extra: Default::default(),
                    },
                    fill: None,
                    extra: Default::default(),
                }],
                hyphenate: false,
                lang: None,
                extra: Default::default(),
            }],
            line_spacing: 1.2,
            path: None,
            exclusions: Default::default(),
            path_memory: None,
            extra: Default::default(),
        }));
        d.data.pages[0].layers[0].objects.push(t);
        let opts = crate::gdr::WriteOptions {
            thumbnail: false,
            ..Default::default()
        };
        let off = crate::gdr::write_bytes(&d, &opts).unwrap();
        let (r, _) = crate::gdr::read_bytes(&off).unwrap();
        assert!(
            !r.extra_members
                .keys()
                .any(|k| k.starts_with("resources/fonts/"))
        );
        d.data.settings.embed_fonts = true;
        let on = crate::gdr::write_bytes(&d, &opts).unwrap();
        assert!(on.len() > off.len());
        let (r, _) = crate::gdr::read_bytes(&on).unwrap();
        assert!(
            r.extra_members
                .keys()
                .any(|k| k.starts_with("resources/fonts/"))
        );
    }
}
