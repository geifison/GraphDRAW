//! Gerenciamento de cor com LittleCMS 2 (GD-CMS-01/02).
//!
//! Cada documento escolhe o perfil RGB (padrão: sRGB IEC61966-2.1
//! embutido), o perfil CMYK (sem perfil = conversão por fórmula), a
//! intenção de renderização e a compensação de ponto preto. As cores
//! continuam guardadas no modelo original (GD-COR-02); o motor só entra na
//! exibição (CMYK → tela) e na saída (RGB → CMYK do PDF/X).

use std::sync::{Arc, RwLock};

use lcms2::{Flags, Intent, PixelFormat, Profile, Transform};
use serde::{Deserialize, Serialize};

/// Configuração de cor do documento (`settings.color` no `.gdr`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ColorManagement {
    /// Arquivo ICC RGB; `None` = sRGB IEC61966-2.1 embutido.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rgb_profile: Option<String>,
    /// Arquivo ICC CMYK; `None` = conversão por fórmula (sem perfil).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmyk_profile: Option<String>,
    /// `perceptual`, `relative` (padrão), `saturation`, `absolute`.
    pub intent: String,
    /// Compensação de ponto preto.
    pub black_point: bool,
}

impl Default for ColorManagement {
    fn default() -> Self {
        ColorManagement {
            rgb_profile: None,
            cmyk_profile: None,
            intent: "relative".into(),
            black_point: true,
        }
    }
}

/// Perfil ICC que vem com o GraphDraw (GD-CMS-01). Fica no documento como
/// `builtin:<nome>`, que abre igual em qualquer computador. Licenças em
/// `assets/icc/LICENSES.md`.
pub struct Builtin {
    pub id: &'static str,
    pub name: &'static str,
    /// Para que serve (mostrado na escolha do perfil).
    pub detail: &'static str,
    /// Condição de impressão registrada correspondente (PDF/X).
    pub condition: crate::export::PrintCondition,
    pub data: &'static [u8],
}

/// Offset couché ISO 12647-2 (FOGRA39): padrão dos documentos novos.
pub const FOGRA39: &str = "builtin:fogra39";
/// GRACoL 2013 (CGATS 21-2 CRPC6).
pub const GRACOL2013: &str = "builtin:gracol2013";

pub static BUILTIN_CMYK: [Builtin; 2] = [
    Builtin {
        id: FOGRA39,
        name: "FOGRA39 — offset couché (ISO 12647-2)",
        detail: "Padrão mais aceito pelas gráficas para papel couché; TAC 300–330%",
        condition: crate::export::PrintCondition::Fogra39,
        data: include_bytes!("../assets/icc/FOGRA39L_coated.icc"),
    },
    Builtin {
        id: GRACOL2013,
        name: "GRACoL 2013 — offset couché (CGATS 21-2 CRPC6)",
        detail: "Padrão G7 (EUA/Américas) para couché; TAC 320%",
        condition: crate::export::PrintCondition::Gracol2013,
        data: include_bytes!("../assets/icc/GRACoL2013_CRPC6.icc"),
    },
];

/// Perfil embutido pelo identificador (`builtin:…`).
pub fn builtin(id: &str) -> Option<&'static Builtin> {
    BUILTIN_CMYK.iter().find(|b| b.id == id)
}

/// Nome para mostrar de um perfil escolhido (embutido ou arquivo).
pub fn profile_label(id: &str) -> String {
    match builtin(id) {
        Some(b) => b.name.to_string(),
        None => std::path::Path::new(id)
            .file_name()
            .map_or_else(|| id.to_string(), |n| n.to_string_lossy().into_owned()),
    }
}

pub const INTENTS: [(&str, &str); 4] = [
    ("perceptual", "Perceptual"),
    ("relative", "Colorimétrica relativa"),
    ("saturation", "Saturação"),
    ("absolute", "Colorimétrica absoluta"),
];

fn intent(s: &str) -> Intent {
    match s {
        "perceptual" => Intent::Perceptual,
        "saturation" => Intent::Saturation,
        "absolute" => Intent::AbsoluteColorimetric,
        _ => Intent::RelativeColorimetric,
    }
}

type T3to4 = Transform<[f64; 3], [f64; 4], lcms2::GlobalContext, lcms2::DisallowCache>;
type T4to3 = Transform<[f64; 4], [f64; 3], lcms2::GlobalContext, lcms2::DisallowCache>;
type T3to3 = Transform<[f64; 3], [f64; 3], lcms2::GlobalContext, lcms2::DisallowCache>;
type T4to4 = Transform<[f64; 4], [f64; 4], lcms2::GlobalContext, lcms2::DisallowCache>;

/// Os dois documentos usam perfis diferentes (GD-CMS-04)?
pub fn profiles_differ(a: &ColorManagement, b: &ColorManagement) -> bool {
    a.rgb_profile != b.rgb_profile || a.cmyk_profile != b.cmyk_profile
}

/// Conversão de valores entre dois conjuntos de perfis (GD-CMS-04): CMYK →
/// CMYK e RGB → RGB, pela intenção do destino. Sem perfil CMYK de um dos
/// lados, o CMYK fica como está (fórmula não tem espaço definido).
pub struct Converter {
    cmyk: Option<T4to4>,
    rgb: Option<T3to3>,
}

impl Converter {
    pub fn new(from: &ColorManagement, to: &ColorManagement) -> Converter {
        let mut warn = Vec::new();
        let it = intent(&to.intent);
        let flags = if to.black_point {
            Flags::BLACKPOINT_COMPENSATION | Flags::NO_CACHE
        } else {
            Flags::NO_CACHE
        };
        let cmyk_of = |c: &ColorManagement, w: &mut Vec<String>| {
            c.cmyk_profile
                .as_deref()
                .and_then(|p| load(p, lcms2::ColorSpaceSignature::CmykData, "CMYK", w))
                .map(|(p, _)| p)
        };
        let rgb_of = |c: &ColorManagement, w: &mut Vec<String>| {
            c.rgb_profile
                .as_deref()
                .and_then(|p| load(p, lcms2::ColorSpaceSignature::RgbData, "RGB", w))
                .map(|(p, _)| p)
                .unwrap_or_else(Profile::new_srgb)
        };
        let cmyk = match (cmyk_of(from, &mut warn), cmyk_of(to, &mut warn)) {
            (Some(a), Some(b)) if from.cmyk_profile != to.cmyk_profile => {
                Transform::new_flags_context(
                    lcms2::GlobalContext::new(),
                    &a,
                    PixelFormat::CMYK_DBL,
                    &b,
                    PixelFormat::CMYK_DBL,
                    it,
                    flags,
                )
                .ok()
            }
            _ => None,
        };
        let rgb = (from.rgb_profile != to.rgb_profile)
            .then(|| {
                Transform::new_flags_context(
                    lcms2::GlobalContext::new(),
                    &rgb_of(from, &mut warn),
                    PixelFormat::RGB_DBL,
                    &rgb_of(to, &mut warn),
                    PixelFormat::RGB_DBL,
                    it,
                    flags,
                )
                .ok()
            })
            .flatten();
        Converter { cmyk, rgb }
    }

    /// Há algo a converter?
    pub fn is_noop(&self) -> bool {
        self.cmyk.is_none() && self.rgb.is_none()
    }

    /// Converte a cor no lugar; `true` se mudou.
    pub fn convert(&self, c: &mut crate::model::Color) -> bool {
        match (c.model.as_str(), &self.cmyk, &self.rgb) {
            ("cmyk", Some(t), _) if c.values.len() >= 4 => {
                let mut out = [[0.0; 4]];
                t.transform_pixels(
                    &[[c.values[0], c.values[1], c.values[2], c.values[3]].map(|v| v * 100.0)],
                    &mut out,
                );
                c.values = out[0].map(|v| (v / 100.0).clamp(0.0, 1.0)).to_vec();
                true
            }
            ("rgb", _, Some(t)) if c.values.len() >= 3 => {
                let mut out = [[0.0; 3]];
                t.transform_pixels(&[[c.values[0], c.values[1], c.values[2]]], &mut out);
                c.values = out[0].map(|v| v.clamp(0.0, 1.0)).to_vec();
                true
            }
            _ => false,
        }
    }
}

/// Transformações prontas para uma configuração.
pub struct Cms {
    pub settings: ColorManagement,
    rgb_to_cmyk: Option<T3to4>,
    cmyk_to_srgb: Option<T4to3>,
    /// Perfil RGB do documento → sRGB da tela (só quando não é sRGB).
    rgb_to_srgb: Option<T3to3>,
    cmyk_icc: Option<Vec<u8>>,
    /// Nome do perfil CMYK (descrição do ICC).
    pub cmyk_name: Option<String>,
    /// Problemas ao carregar perfis (arquivo ausente, espaço de cor errado).
    pub warnings: Vec<String>,
}

fn load(
    path: &str,
    want: lcms2::ColorSpaceSignature,
    what: &str,
    warnings: &mut Vec<String>,
) -> Option<(Profile, Vec<u8>)> {
    let read = match builtin(path) {
        Some(b) => Ok(b.data.to_vec()),
        None => std::fs::read(path),
    };
    let data = match read {
        Ok(d) => d,
        Err(e) => {
            warnings.push(format!(
                "Perfil {what} não abriu ({path}): {e}; usando o padrão."
            ));
            return None;
        }
    };
    match Profile::new_icc(&data) {
        Ok(p) if p.color_space() == want => Some((p, data)),
        Ok(_) => {
            warnings.push(format!("{path} não é um perfil {what}; usando o padrão."));
            None
        }
        Err(e) => {
            warnings.push(format!(
                "Perfil {what} inválido ({path}): {e}; usando o padrão."
            ));
            None
        }
    }
}

fn description(p: &Profile) -> Option<String> {
    p.info(lcms2::InfoType::Description, lcms2::Locale::none())
        .filter(|s| !s.trim().is_empty())
}

impl Cms {
    pub fn new(settings: &ColorManagement) -> Cms {
        let mut warnings = Vec::new();
        let srgb = Profile::new_srgb();
        let rgb = settings
            .rgb_profile
            .as_deref()
            .and_then(|p| load(p, lcms2::ColorSpaceSignature::RgbData, "RGB", &mut warnings))
            .map(|(p, _)| p);
        let cmyk = settings.cmyk_profile.as_deref().and_then(|p| {
            load(
                p,
                lcms2::ColorSpaceSignature::CmykData,
                "CMYK",
                &mut warnings,
            )
        });
        let it = intent(&settings.intent);
        let flags = if settings.black_point {
            Flags::BLACKPOINT_COMPENSATION | Flags::NO_CACHE
        } else {
            Flags::NO_CACHE
        };
        let doc_rgb = rgb.as_ref().unwrap_or(&srgb);
        let (rgb_to_cmyk, cmyk_to_srgb, cmyk_icc, cmyk_name) = match &cmyk {
            Some((p, data)) => (
                Transform::new_flags_context(
                    lcms2::GlobalContext::new(),
                    doc_rgb,
                    PixelFormat::RGB_DBL,
                    p,
                    PixelFormat::CMYK_DBL,
                    it,
                    flags,
                )
                .ok(),
                Transform::new_flags_context(
                    lcms2::GlobalContext::new(),
                    p,
                    PixelFormat::CMYK_DBL,
                    &srgb,
                    PixelFormat::RGB_DBL,
                    it,
                    flags,
                )
                .ok(),
                Some(data.clone()),
                description(p),
            ),
            None => (None, None, None, None),
        };
        let rgb_to_srgb = rgb.as_ref().and_then(|p| {
            Transform::new_flags_context(
                lcms2::GlobalContext::new(),
                p,
                PixelFormat::RGB_DBL,
                &srgb,
                PixelFormat::RGB_DBL,
                it,
                flags,
            )
            .ok()
        });
        Cms {
            settings: settings.clone(),
            rgb_to_cmyk,
            cmyk_to_srgb,
            rgb_to_srgb,
            cmyk_icc,
            cmyk_name,
            warnings,
        }
    }

    /// Há perfil CMYK (senão, fórmula).
    pub fn has_cmyk_profile(&self) -> bool {
        self.rgb_to_cmyk.is_some()
    }

    /// Bytes do perfil CMYK (OutputIntent do PDF/X).
    pub fn cmyk_icc(&self) -> Option<&[u8]> {
        self.cmyk_icc.as_deref()
    }

    /// RGB (0–1, no perfil RGB do documento) → CMYK (0–1).
    pub fn rgb_to_cmyk(&self, rgb: [f64; 3]) -> Option<[f64; 4]> {
        let t = self.rgb_to_cmyk.as_ref()?;
        let mut out = [[0.0; 4]];
        t.transform_pixels(&[rgb], &mut out);
        // O LittleCMS usa 0–100 para CMYK em ponto flutuante.
        Some(out[0].map(|v| (v / 100.0).clamp(0.0, 1.0)))
    }

    /// CMYK (0–1) → sRGB da tela (0–1).
    pub fn cmyk_to_srgb(&self, cmyk: [f64; 4]) -> Option<[f64; 3]> {
        let t = self.cmyk_to_srgb.as_ref()?;
        let mut out = [[0.0; 3]];
        t.transform_pixels(&[cmyk.map(|v| v * 100.0)], &mut out);
        Some(out[0].map(|v| v.clamp(0.0, 1.0)))
    }

    /// RGB do documento → sRGB da tela (identidade quando o perfil é sRGB).
    pub fn rgb_to_srgb(&self, rgb: [f64; 3]) -> [f64; 3] {
        match &self.rgb_to_srgb {
            Some(t) => {
                let mut out = [[0.0; 3]];
                t.transform_pixels(&[rgb], &mut out);
                out[0].map(|v| v.clamp(0.0, 1.0))
            }
            None => rgb,
        }
    }
}

/// sRGB (0–1) → CIELAB D65 (para medir diferença de cor, ΔE).
pub fn srgb_to_lab(c: [f64; 3]) -> [f64; 3] {
    let lin = |v: f64| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b] = c.map(lin);
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

/// Diferença de cor CIEDE2000 entre duas cores sRGB (1 ≈ limite do que se
/// percebe; até 2–3 é o aceito em prova de cor).
pub fn delta_e(a: [f64; 3], b: [f64; 3]) -> f64 {
    crate::fidelity::delta_e2000(srgb_to_lab(a), srgb_to_lab(b))
}

impl Cms {
    /// Como o RGB sai impresso neste perfil CMYK, de volta em sRGB
    /// (RGB → CMYK → tela). `None` sem perfil CMYK.
    pub fn printed(&self, rgb: [f64; 3]) -> Option<[f64; 3]> {
        self.cmyk_to_srgb(self.rgb_to_cmyk(rgb)?)
    }

    /// O RGB cabe na gama de impressão (ΔE2000 da ida e volta até `tol`)?
    pub fn in_print_gamut(&self, rgb: [f64; 3], tol: f64) -> Option<bool> {
        Some(delta_e(self.rgb_to_srgb(rgb), self.printed(rgb)?) <= tol)
    }

    /// Cor RGB equivalente à impressa, já dentro da gama: repete a ida e
    /// volta até estabilizar (o mapeamento de gama não é idempotente nas
    /// cores muito saturadas).
    pub fn to_print_gamut(&self, rgb: [f64; 3]) -> Option<[f64; 3]> {
        let mut p = self.printed(rgb)?;
        for _ in 0..8 {
            let q = self.printed(p)?;
            let d = delta_e(p, q);
            p = q;
            if d < 0.2 {
                break;
            }
        }
        Some(p)
    }
}

static CURRENT: RwLock<Option<Arc<Cms>>> = RwLock::new(None);

/// Ativa a configuração do documento em uso (exibição e exportação).
/// Recria as transformações só quando a configuração muda.
pub fn activate(settings: &ColorManagement) -> Arc<Cms> {
    if let Some(c) = CURRENT.read().ok().and_then(|g| g.clone())
        && c.settings == *settings
    {
        return c;
    }
    let c = Arc::new(Cms::new(settings));
    if let Ok(mut g) = CURRENT.write() {
        *g = Some(c.clone());
    }
    c
}

/// Configuração ativa, se houver (sem documento: fórmulas).
pub fn current() -> Option<Arc<Cms>> {
    CURRENT.read().ok().and_then(|g| g.clone())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Perfil CMYK para testes: \`GRAPHDRAW_CMYK_ICC\` ou o RSWOP do Windows.
    pub fn test_cmyk_profile() -> Option<String> {
        if let Ok(p) = std::env::var("GRAPHDRAW_CMYK_ICC") {
            return Some(p);
        }
        let win = r"C:\Windows\System32\spool\drivers\color\RSWOP.icm";
        std::path::Path::new(win).exists().then(|| win.to_string())
    }

    /// GD-CMS-01: perfis embutidos abrem sem arquivo, são CMYK e convertem.
    #[test]
    fn gd_cms_01_builtin_profiles() {
        for b in &BUILTIN_CMYK {
            let c = Cms::new(&ColorManagement {
                cmyk_profile: Some(b.id.into()),
                ..Default::default()
            });
            assert!(c.warnings.is_empty(), "{}: {:?}", b.id, c.warnings);
            assert!(c.has_cmyk_profile(), "{}", b.id);
            assert!(c.cmyk_icc().is_some_and(|d| d.len() == b.data.len()));
            // Ciano 100% fica azul-esverdeado na tela; branco do papel ~ branco.
            let cyan = c.cmyk_to_srgb([1.0, 0.0, 0.0, 0.0]).unwrap();
            assert!(cyan[0] < 0.3 && cyan[2] > 0.7, "{}: {cyan:?}", b.id);
            let paper = c.cmyk_to_srgb([0.0; 4]).unwrap();
            assert!(paper.iter().all(|v| *v > 0.9), "{}: {paper:?}", b.id);
            // RGB puro sai da gama do offset; cinza médio cabe.
            assert_eq!(c.in_print_gamut([0.0, 0.0, 1.0], 3.0), Some(false));
            assert_eq!(c.in_print_gamut([0.5, 0.5, 0.5], 3.0), Some(true));
        }
        assert_eq!(profile_label(FOGRA39), BUILTIN_CMYK[0].name);
        assert_eq!(profile_label("C:/x/Meu.icc"), "Meu.icc");
    }

    #[test]
    fn delta_e_basics() {
        assert!(delta_e([1.0; 3], [1.0; 3]) < 1e-9);
        let l = srgb_to_lab([1.0; 3]);
        assert!(
            (l[0] - 100.0).abs() < 0.1 && l[1].abs() < 0.1 && l[2].abs() < 0.1,
            "{l:?}"
        );
        assert!(delta_e([0.0; 3], [1.0; 3]) > 90.0);
    }

    #[test]
    fn gd_cms_profile_conversions() {
        let Some(path) = test_cmyk_profile() else {
            return;
        };
        let cms = Cms::new(&ColorManagement {
            cmyk_profile: Some(path),
            ..Default::default()
        });
        assert!(cms.warnings.is_empty(), "{:?}", cms.warnings);
        assert!(cms.has_cmyk_profile() && cms.cmyk_icc().is_some());
        // Branco → quase sem tinta; preto → K alto.
        let w = cms.rgb_to_cmyk([1.0, 1.0, 1.0]).unwrap();
        assert!(w.iter().all(|v| *v < 0.05), "{w:?}");
        let k = cms.rgb_to_cmyk([0.0, 0.0, 0.0]).unwrap();
        assert!(k[3] > 0.8, "{k:?}");
        // É o perfil, não a fórmula: um tom médio sai diferente.
        let f = crate::export::pdf::formula_cmyk([0.2, 0.5, 0.8]);
        let p = cms.rgb_to_cmyk([0.2, 0.5, 0.8]).unwrap();
        assert!(
            (0..4).any(|i| (p[i] - f.values[i]).abs() > 0.02),
            "{p:?} × {:?}",
            f.values
        );
        // Ida e volta aproximada num tom médio.
        let c = cms.rgb_to_cmyk([0.2, 0.5, 0.8]).unwrap();
        let back = cms.cmyk_to_srgb(c).unwrap();
        assert!(
            (back[2] - 0.8).abs() < 0.15 && back[2] > back[0],
            "{back:?}"
        );
        // Intenções e ponto preto mudam o resultado sem quebrar.
        for (i, _) in INTENTS {
            let c2 = Cms::new(&ColorManagement {
                cmyk_profile: cms.settings.cmyk_profile.clone(),
                intent: i.into(),
                black_point: false,
                ..Default::default()
            });
            assert!(c2.rgb_to_cmyk([0.2, 0.5, 0.8]).is_some());
        }
    }

    #[test]
    fn missing_or_wrong_profile_falls_back_with_warning() {
        let cms = Cms::new(&ColorManagement {
            cmyk_profile: Some("nao-existe.icc".into()),
            ..Default::default()
        });
        assert!(!cms.has_cmyk_profile());
        assert_eq!(cms.warnings.len(), 1);
        assert_eq!(cms.rgb_to_srgb([0.3, 0.4, 0.5]), [0.3, 0.4, 0.5]);
        // JSON: padrão sem campos opcionais.
        let j = serde_json::to_string(&ColorManagement::default()).unwrap();
        assert_eq!(j, r#"{"intent":"relative","blackPoint":true}"#);
    }
}
