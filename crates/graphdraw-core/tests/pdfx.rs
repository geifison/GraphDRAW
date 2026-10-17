//! GD-EXP-07: PDF comum × PDF/X-1a:2001. Aceite: um documento com retângulo
//! RGB semitransparente, gradiente RGB e imagem PNG com alfa vira um PDF/X-1a
//! sem RGB, sem transparência, com caixas de produção, OutputIntent, chaves
//! GTS, Trapped, /ID e relatório das conversões.

use graphdraw_core::export::{self, ExportOptions, PdfStandard, PrintCondition};
use graphdraw_core::model::{
    Color, Document, Fill, Gradient, GradientStop, ImageData, Object, ObjectKind, Page, RectData,
    Transform,
};
use lopdf::content::Content;
use lopdf::{Document as Pdf, Object as O};
use serde_json::Map;

const MM: f64 = 1000.0;
const BLEED: f64 = 3.0 * MM;

/// PNG 20×20 com metade transparente.
fn png_with_alpha() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 20, 20);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().unwrap();
        let mut data = Vec::new();
        for y in 0..20 {
            for _ in 0..20 {
                data.extend_from_slice(&[0, 200, 255, if y < 10 { 255 } else { 80 }]);
            }
        }
        w.write_image_data(&data).unwrap();
    }
    out
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Object {
    let mut o = Object::new(ObjectKind::Rectangle(RectData {
        width: w,
        height: h,
        corners: None,
        extra: Map::new(),
    }));
    o.transform = Transform::translate(x, y);
    o
}

fn sample() -> Document {
    let mut d = Document::new();
    d.metadata.title = Some("Teste PDF/X".into());
    d.data.settings.bleed = BLEED;
    let mut p = Page::new(100.0 * MM, 80.0 * MM);
    // Retângulo RGB semitransparente.
    let mut a = rect(10.0 * MM, 10.0 * MM, 30.0 * MM, 30.0 * MM);
    a.fill = Some(Fill::Solid {
        color: Color::rgb(0.1, 0.9, 0.2),
        opacity: 0.5,
        overprint: false,
        extra: Map::new(),
    });
    // Gradiente RGB opaco (sai vetorial, em CMYK).
    let mut g = rect(50.0 * MM, 10.0 * MM, 40.0 * MM, 20.0 * MM);
    g.fill = Some(Fill::Gradient(Gradient {
        kind: "linear".into(),
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: Color::rgb(1.0, 0.0, 0.0),
                opacity: 1.0,
                extra: Map::new(),
            },
            GradientStop {
                offset: 1.0,
                color: Color::rgb(0.0, 0.0, 1.0),
                opacity: 1.0,
                extra: Map::new(),
            },
        ],
        start: Some([0.0, 0.0]),
        end: Some([40.0 * MM, 0.0]),
        center: None,
        radius: None,
        spread: "pad".into(),
        extra: Map::new(),
    }));
    // Imagem com alfa: 20 px em 40 mm ≈ 12,7 ppp (abaixo de 300).
    let src = d.add_image(png_with_alpha(), "png");
    let mut img = Object::new(ObjectKind::Image(ImageData {
        width: 40.0 * MM,
        height: 40.0 * MM,
        src,
        extra: Map::new(),
    }));
    img.transform = Transform::translate(50.0 * MM, 35.0 * MM);
    // Objeto CMYK puro: precisa sair igual.
    let mut k = rect(10.0 * MM, 50.0 * MM, 20.0 * MM, 20.0 * MM);
    k.fill = Some(Fill::solid(Color::cmyk(0.0, 1.0, 1.0, 0.0)));
    p.layers[0].objects = vec![a, g, img, k];
    d.data.pages.push(p);
    d
}

fn export(d: &Document, pdf: PdfStandard) -> (Vec<u8>, Vec<String>) {
    let opts = ExportOptions {
        pdf,
        ..Default::default()
    };
    let (bytes, report) = export::pdf_document_report(d, &opts).unwrap();
    (bytes, report.messages())
}

/// Todos os nomes e chaves do arquivo (para procurar DeviceRGB, SMask…).
fn all_names(pdf: &Pdf) -> Vec<String> {
    fn walk(o: &O, out: &mut Vec<String>) {
        match o {
            O::Name(n) => out.push(String::from_utf8_lossy(n).into_owned()),
            O::Array(a) => a.iter().for_each(|x| walk(x, out)),
            O::Dictionary(d) => {
                for (k, v) in d.iter() {
                    out.push(String::from_utf8_lossy(k).into_owned());
                    walk(v, out);
                }
            }
            O::Stream(s) => {
                for (k, v) in s.dict.iter() {
                    out.push(String::from_utf8_lossy(k).into_owned());
                    walk(v, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for o in pdf.objects.values() {
        walk(o, &mut out);
    }
    out
}

fn as_rect(o: &O) -> [f64; 4] {
    let a = o.as_array().unwrap();
    let v: Vec<f64> = a
        .iter()
        .map(|x| match x {
            O::Integer(i) => *i as f64,
            O::Real(r) => *r as f64,
            _ => panic!("número esperado"),
        })
        .collect();
    [v[0], v[1], v[2], v[3]]
}

#[test]
fn gd_exp_07_pdfx1a_is_print_safe() {
    let d = sample();
    let (bytes, msgs) = export(&d, PdfStandard::X1a(PrintCondition::Fogra39));
    let pdf = Pdf::load_mem(&bytes).expect("PDF ilegível");

    // 6. Versão e identificação.
    assert_eq!(pdf.version, "1.3");
    assert!(pdf.trailer.get(b"ID").is_ok(), "falta /ID");
    let info_ref = pdf.trailer.get(b"Info").unwrap().as_reference().unwrap();
    let info = pdf.get_dictionary(info_ref).unwrap();
    let text =
        |k: &[u8]| String::from_utf8_lossy(info.get(k).unwrap().as_str().unwrap()).into_owned();
    assert_eq!(text(b"GTS_PDFXVersion"), "PDF/X-1:2001");
    assert_eq!(text(b"GTS_PDFXConformance"), "PDF/X-1a:2001");
    assert_eq!(text(b"Title"), "Teste PDF/X");
    assert!(info.get(b"CreationDate").is_ok() && info.get(b"ModDate").is_ok());
    assert_eq!(info.get(b"Trapped").unwrap().as_name().unwrap(), b"False");

    // 7. OutputIntent GTS_PDFX.
    let cat = pdf.catalog().unwrap();
    let intents = cat.get(b"OutputIntents").unwrap().as_array().unwrap();
    let oi = match &intents[0] {
        O::Dictionary(d) => d.clone(),
        O::Reference(r) => pdf.get_dictionary(*r).unwrap().clone(),
        _ => panic!(),
    };
    assert_eq!(oi.get(b"S").unwrap().as_name().unwrap(), b"GTS_PDFX");
    assert_eq!(
        oi.get(b"OutputConditionIdentifier")
            .unwrap()
            .as_str()
            .unwrap(),
        b"FOGRA39"
    );
    assert_eq!(
        oi.get(b"RegistryName").unwrap().as_str().unwrap(),
        b"http://www.color.org"
    );

    // 5. Caixas: TrimBox = página, BleedBox = MediaBox = página + sangria.
    let k = 72.0 / 25_400.0;
    for (_, id) in pdf.get_pages() {
        let page = pdf.get_dictionary(id).unwrap();
        let media = as_rect(page.get(b"MediaBox").unwrap());
        let bleed = as_rect(page.get(b"BleedBox").unwrap());
        let trim = as_rect(page.get(b"TrimBox").unwrap());
        assert_eq!(media, bleed);
        let b = BLEED * k;
        assert!(
            (trim[0] - b).abs() < 0.01 && (trim[1] - b).abs() < 0.01,
            "{trim:?}"
        );
        assert!((trim[2] - trim[0] - 100.0 * MM * k).abs() < 0.01);
        assert!((media[2] - (100.0 * MM + 2.0 * BLEED) * k).abs() < 0.01);
        // 4. Nada interativo na página.
        assert!(page.get(b"Annots").is_err());
    }

    // 1 e 2. Sem RGB e sem transparência em lugar nenhum.
    let names = all_names(&pdf);
    for bad in [
        "DeviceRGB",
        "SMask",
        "CA",
        "ca",
        "JavaScript",
        "AA",
        "Interpolate",
    ] {
        assert!(!names.iter().any(|n| n == bad), "{bad} no PDF/X-1a");
    }
    assert!(names.iter().any(|n| n == "DeviceCMYK"));
    for (_, id) in pdf.get_pages() {
        let ops = Content::decode(&pdf.get_page_content(id)).unwrap();
        for op in &ops.operations {
            assert!(
                !["rg", "RG", "gs"].contains(&op.operator.as_str()),
                "operador {} no PDF/X-1a",
                op.operator
            );
        }
        // O CMYK original (0 1 1 0) chega intacto.
        assert!(ops.operations.iter().any(|op| {
            op.operator == "k"
                && op
                    .operands
                    .iter()
                    .map(|o| o.as_float().unwrap())
                    .collect::<Vec<_>>()
                    == [0.0, 1.0, 1.0, 0.0]
        }));
    }

    // 9. Relatório.
    let all = msgs.join("\n");
    assert!(all.contains("RGB convertida"), "{all}");
    assert!(all.contains("gradiente"), "{all}");
    assert!(all.contains("imagem"), "{all}");
    assert!(all.contains("achatado"), "{all}");
    assert!(all.contains("aparência igual"), "{all}");
    assert!(all.contains("ppp"), "{all}");
}

#[test]
fn gd_exp_07_screen_pdf_keeps_rgb_and_transparency() {
    let (bytes, msgs) = export(&sample(), PdfStandard::Screen);
    let pdf = Pdf::load_mem(&bytes).unwrap();
    let names = all_names(&pdf);
    assert!(names.iter().any(|n| n == "DeviceRGB"));
    assert!(
        names.iter().any(|n| n == "SMask"),
        "alfa da imagem preservado"
    );
    assert!(names.iter().any(|n| n == "ca"), "opacidade preservada");
    assert!(msgs.is_empty(), "PDF comum não converte nada");
    let page = pdf.get_pages().values().next().copied().unwrap();
    assert!(pdf.get_dictionary(page).unwrap().get(b"TrimBox").is_err());
}

#[test]
fn print_conditions_are_registered_names() {
    for c in PrintCondition::ALL {
        assert!(!c.identifier().is_empty() && !c.description().is_empty());
        assert_eq!(PrintCondition::from_name(c.identifier()), Some(c));
    }
}

/// GD-CMS-01/02: com perfil CMYK no documento, as conversões usam o
/// LittleCMS e o PDF/X leva o perfil no OutputIntent (DestOutputProfile).
#[test]
fn gd_cms_pdfx_uses_document_cmyk_profile() {
    let path = std::env::var("GRAPHDRAW_CMYK_ICC").ok().or_else(|| {
        let win = r"C:\Windows\System32\spool\drivers\color\RSWOP.icm";
        std::path::Path::new(win).exists().then(|| win.to_string())
    });
    let Some(path) = path else {
        return;
    };
    let mut d = sample();
    d.data.settings.color.cmyk_profile = Some(path);
    let (bytes, msgs) = export(&d, PdfStandard::X1a(PrintCondition::Fogra39));
    let pdf = Pdf::load_mem(&bytes).unwrap();
    let has_dest = all_names(&pdf).iter().any(|n| n == "DestOutputProfile");
    assert!(has_dest, "OutputIntent com o perfil");
    assert!(
        msgs.iter().any(|m| m.contains("com o perfil")),
        "relatório cita o perfil: {msgs:?}"
    );
    // Sem perfil: fórmula, sem DestOutputProfile.
    let (bytes, msgs) = export(&sample(), PdfStandard::X1a(PrintCondition::Fogra39));
    let pdf = Pdf::load_mem(&bytes).unwrap();
    assert!(!all_names(&pdf).iter().any(|n| n == "DestOutputProfile"));
    assert!(msgs.iter().any(|m| m.contains("por fórmula")));
}
