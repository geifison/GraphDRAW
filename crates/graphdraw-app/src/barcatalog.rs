//! Catálogo dos contextos da barra de propriedades (doc 31, GD-BPC-*):
//! para cada contexto, os grupos na ordem da spec. A barra monta o
//! contexto a partir daqui e os testes conferem a ordem.
//!
//! Grupos `B-*` são os blocos reutilizáveis do doc 31 (seção 1); os outros
//! são os controles próprios do contexto. Controles da spec que o
//! GraphDraw ainda não tem ficam em [`MISSING`] (D-31-14).

/// Contexto da ferramenta Seleção por tipo de seleção (doc 31, seção 2).
pub struct Context {
    /// Requisito (conferido nos testes).
    #[cfg_attr(not(test), expect(dead_code))]
    pub spec: &'static str,
    pub groups: &'static [&'static str],
}

/// Grupos de um contexto da Seleção. Tipo sem contexto próprio na spec
/// (cota, conector, forma básica, imagem…) usa o genérico.
pub fn selection(kind: &str) -> Context {
    let (spec, groups): (&str, &[&str]) = match kind {
        "none" => ("GD-BPC-01", &["PAG", "PAG-SETAS", "B-FOC"]),
        "multi" => (
            "GD-BPC-02",
            &["B-TRF", "CMB", "B-CTN", "GRP-TUDO", "B-FOC", "ALI"],
        ),
        "group" => ("GD-BPC-03", &["B-TRF", "B-CTN", "DESAGR", "B-FOC", "B-ORG"]),
        "rect" => (
            "GD-BPC-04",
            &[
                "B-TRF",
                "RET-CANTOS",
                "B-CTN",
                "B-CTV",
                "B-FOC",
                "B-ORG",
                "B-CNV",
            ],
        ),
        "ellipse" => (
            "GD-BPC-05",
            &[
                "B-TRF", "ELI-MODO", "ELI-ANG", "B-CTN", "B-CTV", "B-FOC", "B-ORG", "B-CNV",
            ],
        ),
        "polygon" => (
            "GD-BPC-06",
            &["B-TRF", "POL", "B-CTN", "B-CTV", "B-FOC", "B-ORG", "B-CNV"],
        ),
        "star" => (
            "GD-BPC-07",
            &["B-TRF", "POL", "B-CTN", "B-FOC", "B-ORG", "B-CNV"],
        ),
        // D-31-03: a espiral também começa com B-TRF.
        "spiral" => (
            "GD-BPC-08",
            &["B-TRF", "ESP", "B-CTN", "B-SET", "B-ORG", "B-FOC"],
        ),
        "curve" => (
            "GD-BPC-10",
            &[
                "B-TRF", "B-CTN", "B-CTV", "B-SET", "B-CUR", "B-FOC", "B-ORG",
            ],
        ),
        "text" => ("GD-BPC-14", &["B-TRF", "B-FON", "B-FOC"]),
        _ => ("GD-BPR-02", &["B-TRF", "B-CTN", "B-FOC", "B-ORG"]),
    };
    Context { spec, groups }
}

/// Grupos próprios do GraphDraw, sempre depois dos do contexto (D-31-04).
#[cfg_attr(not(test), expect(dead_code))]
pub const EXTRAS: &[&str] = &[
    "ORD",
    "CANETA",
    "TXT-ESP",
    "TXT-CAM",
    "TXT-HIF",
    "SIM",
    "BOOL-VIVA",
    "B-EFX",
];

/// Controles do doc 31 que ainda não existem no GraphDraw (D-31-14):
/// (grupo, controle). Entram junto com o recurso.
#[cfg_attr(not(test), expect(dead_code))]
pub const MISSING: &[(&str, &str)] = &[
    ("B-CTN", "lista de espessuras"),
    ("B-CUR", "suavização à mão livre (fica na ferramenta)"),
    ("B-CUR", "caixa de limites"),
    ("B-CUR", "desenho paralelo"),
    ("B-FOC", "entrar no grupo (usa o Modo foco)"),
    (
        "PAG",
        "aplicar a: todas as páginas / só esta (hoje vale para todas, GD-PAG-01)",
    ),
    ("PAG", "unidade de desenho"),
    ("PAG", "distância de duplicação X/Y"),
    ("PAG", "tratar vazados como preenchidos"),
    ("RET-CANTOS", "raio relativo ao tamanho"),
    ("ESP", "fechar"),
    ("B-FON", "eixos de fonte variável, sublinhado"),
    (
        "GD-BPC-14",
        "marcadores, numeração, capitular, recuos, OpenType, painel de texto, vertical",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// D-31-03: todo objeto selecionado começa com B-TRF; sem seleção, a
    /// página.
    #[test]
    fn every_object_context_starts_with_transform() {
        for k in [
            "multi", "group", "rect", "ellipse", "polygon", "star", "spiral", "curve", "text",
            "image",
        ] {
            assert_eq!(selection(k).groups[0], "B-TRF", "{k}");
        }
        assert_eq!(selection("none").groups[0], "PAG");
    }

    /// Cada contexto cita o seu requisito e cada controle que falta é de
    /// um grupo que algum contexto usa (ou do próprio contexto).
    #[test]
    fn missing_controls_belong_to_known_groups() {
        let kinds = [
            "none", "multi", "group", "rect", "ellipse", "polygon", "star", "spiral", "curve",
            "text",
        ];
        let specs: Vec<&str> = kinds.iter().map(|k| selection(k).spec).collect();
        assert!(specs.iter().all(|s| s.starts_with("GD-BPC-")));
        for (g, what) in MISSING {
            let known = specs.contains(g) || kinds.iter().any(|k| selection(k).groups.contains(g));
            assert!(known, "{g}: {what}");
        }
    }

    /// Nenhum grupo extra repete um grupo de contexto (D-31-04).
    #[test]
    fn extras_do_not_repeat_context_groups() {
        for k in [
            "none", "multi", "group", "rect", "ellipse", "spiral", "curve", "text",
        ] {
            for g in selection(k).groups {
                assert!(!EXTRAS.contains(g), "{k}: {g}");
            }
        }
    }
}
