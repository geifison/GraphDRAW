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
    let data = match std::fs::read(path) {
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
