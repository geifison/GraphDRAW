//! Mudar caixa (GD-TXT-11): reescreve os caracteres do texto em um dos
//! cinco modos, pelo mapeamento de caixa Unicode, sem mexer nos trechos e
//! seus estilos. O que vem antes do intervalo decide o começo de frase e
//! de palavra.

use crate::model::TextData;

/// Modo de Mudar caixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum CaseMode {
    /// Primeira letra da frase maiúscula.
    #[default]
    Sentence,
    Lower,
    Upper,
    /// Todas as iniciais maiúsculas.
    Title,
    /// Inverte a caixa de cada letra.
    Toggle,
}

impl CaseMode {
    pub const ALL: [CaseMode; 5] = [
        CaseMode::Sentence,
        CaseMode::Lower,
        CaseMode::Upper,
        CaseMode::Title,
        CaseMode::Toggle,
    ];

    /// Ciclo rápido (GD-TXT-12): minúsculas → MAIÚSCULAS → Todas as
    /// Iniciais Maiúsculas → minúsculas; os outros modos entram em
    /// minúsculas.
    pub fn cycle(self) -> CaseMode {
        match self {
            CaseMode::Lower => CaseMode::Upper,
            CaseMode::Upper => CaseMode::Title,
            _ => CaseMode::Lower,
        }
    }

    /// Identificador estável (configuração do usuário).
    pub fn id(self) -> &'static str {
        match self {
            CaseMode::Sentence => "sentence",
            CaseMode::Lower => "lower",
            CaseMode::Upper => "upper",
            CaseMode::Title => "title",
            CaseMode::Toggle => "toggle",
        }
    }

    pub fn from_id(id: &str) -> Option<CaseMode> {
        CaseMode::ALL.into_iter().find(|m| m.id() == id)
    }
}

/// Idioma com `i` pontuado (turco, azeri).
fn turkic(lang: Option<&str>) -> bool {
    lang.is_some_and(|l| {
        let l = l.to_ascii_lowercase();
        let base = l.split(['-', '_']).next().unwrap_or("");
        base == "tr" || base == "az"
    })
}

fn upper(c: char, tk: bool, out: &mut String) {
    match (tk, c) {
        (true, 'i') => out.push('İ'),
        (true, 'ı') => out.push('I'),
        _ => out.extend(c.to_uppercase()),
    }
}

/// Minúscula; `final_sigma` = Σ no fim de palavra (vira ς).
fn lower(c: char, tk: bool, final_sigma: bool, out: &mut String) {
    match (tk, c) {
        (true, 'I') => out.push('ı'),
        (true, 'İ') => out.push('i'),
        (_, 'Σ') if final_sigma => out.push('ς'),
        _ => out.extend(c.to_lowercase()),
    }
}

/// Caixa de título: os dígrafos têm forma própria (ǆ → ǅ); ß → Ss.
fn title(c: char, tk: bool, out: &mut String) {
    match c {
        'Ǆ' | 'ǅ' | 'ǆ' => out.push('ǅ'),
        'Ǉ' | 'ǈ' | 'ǉ' => out.push('ǈ'),
        'Ǌ' | 'ǋ' | 'ǌ' => out.push('ǋ'),
        'Ǳ' | 'ǲ' | 'ǳ' => out.push('ǲ'),
        'ß' => out.push_str("Ss"),
        _ => upper(c, tk, out),
    }
}

/// Marca combinante (acento digitado à parte): continua a letra.
fn combining(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF)
}

/// Faz parte da palavra (Todas as Iniciais Maiúsculas).
fn word_char(c: char) -> bool {
    c.is_alphanumeric() || combining(c)
}

fn apostrophe(c: char) -> bool {
    matches!(c, '\'' | '’')
}

/// Fecha aspas ou parênteses depois do fim de frase sem desfazê-lo.
fn closing(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '"' | '\'' | '”' | '’' | '»' | '›')
}

/// Caracteres de entrada: o texto corrido, com a quebra de parágrafo como
/// uma posição própria.
struct Input {
    chars: Vec<char>,
    /// A posição é a quebra entre parágrafos (não é caractere de trecho).
    brk: Vec<bool>,
    /// Idioma turco/azeri na posição.
    tk: Vec<bool>,
}

/// Nova forma de cada posição em `[a, b)` (`None` = fora do intervalo ou
/// quebra de parágrafo). `artistic` = o texto é uma linha contínua única
/// para a primeira letra da frase.
fn convert(inp: &Input, a: usize, b: usize, mode: CaseMode, artistic: bool) -> Vec<Option<String>> {
    let n = inp.chars.len();
    let b = b.min(n);
    let mut out = vec![None; n];
    let letter = |i: usize| inp.chars.get(i).is_some_and(|c| c.is_alphabetic()) && !inp.brk[i];
    // Σ no fim de palavra: letra antes e nenhuma depois.
    let final_sigma = |i: usize| i > 0 && letter(i - 1) && !letter(i + 1);
    // Frase: começa no início do bloco e depois de . ! ? + espaço.
    let mut start = true;
    let mut pending = false;
    // Palavra: a posição anterior era parte de palavra.
    let mut in_word = false;
    for (i, &c) in inp.chars[..b].iter().enumerate() {
        let tk = inp.tk[i];
        let mut s = String::new();
        if inp.brk[i] {
            if !artistic {
                start = true;
            }
            pending = false;
            in_word = false;
            continue;
        }
        match mode {
            CaseMode::Sentence => {
                if c.is_alphabetic() {
                    if start {
                        upper(c, tk, &mut s);
                    } else {
                        lower(c, tk, final_sigma(i), &mut s);
                    }
                    start = false;
                    pending = false;
                } else {
                    s.push(c);
                    if matches!(c, '.' | '!' | '?' | '…') {
                        pending = true;
                    } else if c == '\n' {
                        // Quebra de linha dentro do parágrafo: como a de parágrafo.
                        if !artistic {
                            start = true;
                        }
                        pending = false;
                    } else if c.is_whitespace() {
                        if pending && !artistic {
                            start = true;
                        }
                        pending = false;
                    } else if !closing(c) {
                        pending = false;
                    }
                }
            }
            CaseMode::Title => {
                if c.is_alphabetic() {
                    if in_word {
                        lower(c, tk, final_sigma(i), &mut s);
                    } else {
                        title(c, tk, &mut s);
                    }
                    in_word = true;
                } else {
                    s.push(c);
                    in_word = word_char(c) || (apostrophe(c) && in_word);
                }
            }
            CaseMode::Lower => lower(c, tk, final_sigma(i), &mut s),
            CaseMode::Upper => upper(c, tk, &mut s),
            CaseMode::Toggle => {
                if c.is_uppercase() {
                    lower(c, tk, final_sigma(i), &mut s)
                } else if c.is_lowercase() {
                    upper(c, tk, &mut s)
                } else {
                    s.push(c)
                }
            }
        }
        if i >= a {
            out[i] = Some(s);
        }
    }
    out
}

fn input(t: &TextData) -> Input {
    let mut inp = Input {
        chars: Vec::new(),
        brk: Vec::new(),
        tk: Vec::new(),
    };
    for (pi, p) in t.paragraphs.iter().enumerate() {
        let tk = turkic(p.lang.as_deref());
        if pi > 0 {
            inp.chars.push('\n');
            inp.brk.push(true);
            inp.tk.push(tk);
        }
        for s in &p.spans {
            for c in s.text.chars() {
                inp.chars.push(c);
                inp.brk.push(false);
                inp.tk.push(tk);
            }
        }
    }
    inp
}

/// Muda a caixa dos caracteres `[a, b)` do texto corrido (índices de
/// [`crate::edit::text::len`]). Cada caractere continua no seu trecho, com o
/// estilo dele. Devolve o novo fim do intervalo (`ß` → `SS` alonga o texto).
pub fn change_case(t: &mut TextData, a: usize, b: usize, mode: CaseMode) -> usize {
    let (a, b) = (a.min(b), a.max(b));
    let inp = input(t);
    let b = b.min(inp.chars.len());
    if a >= b {
        return b;
    }
    let outs = convert(&inp, a, b, mode, t.text_kind != "paragraph");
    let mut grown = 0usize;
    let mut i = 0usize;
    for (pi, p) in t.paragraphs.iter_mut().enumerate() {
        if pi > 0 {
            i += 1;
        }
        for s in &mut p.spans {
            let mut text = String::with_capacity(s.text.len());
            for c in s.text.chars() {
                match &outs[i] {
                    Some(o) => {
                        text.push_str(o);
                        grown += o.chars().count();
                    }
                    None => text.push(c),
                }
                i += 1;
            }
            s.text = text;
        }
    }
    // Quebras de parágrafo dentro do intervalo contam uma posição cada.
    let breaks = (a..b).filter(|&k| inp.brk[k]).count();
    a + grown + breaks
}

/// Muda a caixa do texto inteiro.
pub fn change_case_all(t: &mut TextData, mode: CaseMode) {
    let n = crate::edit::text::len(t);
    change_case(t, 0, n, mode);
}

/// Prévia de [`change_case`] em `[a, b)`: o trecho antes e depois.
pub fn preview(t: &TextData, a: usize, b: usize, mode: CaseMode) -> (String, String) {
    let (a, b) = (a.min(b), a.max(b));
    let slice = |t: &TextData, a: usize, b: usize| -> String {
        crate::render::text::plain_text(t)
            .chars()
            .skip(a)
            .take(b.saturating_sub(a))
            .collect()
    };
    let mut c = t.clone();
    let nb = change_case(&mut c, a, b, mode);
    (slice(t, a, b), slice(&c, a, nb))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::text;
    use crate::model::ObjectKind;
    use crate::render::text::plain_text;

    fn make(kind: &str, s: &str) -> TextData {
        let frame = (kind == "paragraph").then_some([100_000.0, 50_000.0]);
        let ObjectKind::Text(mut t) = text::new_text(0.0, 0.0, frame, "Arial", 10_000.0).kind
        else {
            unreachable!()
        };
        text::insert(&mut t, 0, s);
        t
    }

    fn conv(kind: &str, s: &str, mode: CaseMode) -> String {
        let mut t = make(kind, s);
        change_case_all(&mut t, mode);
        plain_text(&t)
    }

    /// Matriz da GD-TXT-11 (texto de parágrafo e artístico).
    #[test]
    fn matrix_of_the_five_modes() {
        use CaseMode::*;
        for (mode, kind, input, output) in [
            (
                Sentence,
                "paragraph",
                "ESTE É UM TESTE. OUTRA FRASE AQUI!",
                "Este é um teste. Outra frase aqui!",
            ),
            (
                Sentence,
                "artistic",
                "corte a laser em acrílico 3mm",
                "Corte a laser em acrílico 3mm",
            ),
            (
                Lower,
                "artistic",
                "Logotipo VETORIAL & Design Gráfico",
                "logotipo vetorial & design gráfico",
            ),
            (
                Upper,
                "artistic",
                "manual técnico de impressão",
                "MANUAL TÉCNICO DE IMPRESSÃO",
            ),
            (
                Title,
                "artistic",
                "comunicação visual e corte vinílico",
                "Comunicação Visual E Corte Vinílico",
            ),
            (
                Title,
                "artistic",
                "sub-base multi-eixo",
                "Sub-Base Multi-Eixo",
            ),
            (
                Toggle,
                "artistic",
                "eDITOR vETORIAL pROFISSIONAL",
                "Editor Vetorial Profissional",
            ),
            (
                Toggle,
                "artistic",
                "bOTÃO dIREITO dO mOUSE",
                "Botão Direito Do Mouse",
            ),
        ] {
            assert_eq!(conv(kind, input, mode), output, "{mode:?} {input}");
        }
    }

    /// Artístico: uma linha contínua, só a primeira letra; parágrafo: cada
    /// frase e cada parágrafo.
    #[test]
    fn sentence_case_artistic_vs_paragraph() {
        let s = "ESTE É UM TESTE. OUTRA FRASE?\nNOVO PARÁGRAFO (FIM.) depois";
        assert_eq!(
            conv("artistic", s, CaseMode::Sentence),
            "Este é um teste. outra frase?\nnovo parágrafo (fim.) depois"
        );
        assert_eq!(
            conv("paragraph", s, CaseMode::Sentence),
            "Este é um teste. Outra frase?\nNovo parágrafo (fim.) Depois"
        );
        // Ponto sem espaço (número, abreviação colada) não começa frase.
        assert_eq!(
            conv("paragraph", "VALOR 3.5MM. FIM", CaseMode::Sentence),
            "Valor 3.5mm. Fim"
        );
    }

    #[test]
    fn title_case_words() {
        assert_eq!(
            conv("artistic", "CORTE/VINCO — 3MM d'água", CaseMode::Title),
            "Corte/Vinco — 3mm D'água"
        );
    }

    /// Unicode: ç, é, ß → SS (o texto alonga), sigma final, i turco.
    #[test]
    fn unicode_mappings() {
        assert_eq!(
            conv("artistic", "ação é straße", CaseMode::Upper),
            "AÇÃO É STRASSE"
        );
        assert_eq!(
            conv("artistic", "ΟΔΟΣ ΣΟΦΙΑΣ", CaseMode::Lower),
            "οδο\u{3c2} σοφια\u{3c2}"
        );
        assert_eq!(conv("artistic", "ǆungla", CaseMode::Title), "ǅungla");
        let mut t = make("artistic", "istanbul IRMAK");
        t.paragraphs[0].lang = Some("tr-TR".into());
        change_case_all(&mut t, CaseMode::Toggle);
        assert_eq!(plain_text(&t), "İSTANBUL ırmak");
        let mut t = make("artistic", "istanbul");
        change_case_all(&mut t, CaseMode::Upper);
        assert_eq!(plain_text(&t), "ISTANBUL");
    }

    /// Só o intervalo muda; o contexto antes dele decide a palavra; o fim
    /// devolvido acompanha o texto que alongou.
    #[test]
    fn range_with_context_and_growth() {
        let mut t = make("artistic", "rua straße nova");
        // "straße" = [4, 10)
        let end = change_case(&mut t, 4, 10, CaseMode::Upper);
        assert_eq!(plain_text(&t), "rua STRASSE nova");
        assert_eq!(end, 11);
        // Meio de palavra: continua minúscula no modo Iniciais.
        let mut t = make("artistic", "PALAVRA");
        change_case(&mut t, 3, 7, CaseMode::Title);
        assert_eq!(plain_text(&t), "PALavra");
        // Intervalo que atravessa parágrafos.
        let mut t = make("paragraph", "ab\ncd\nef");
        let end = change_case(&mut t, 1, 7, CaseMode::Upper);
        assert_eq!(plain_text(&t), "aB\nCD\nEf");
        assert_eq!(end, 7);
    }

    /// Estilos ficam nos seus caracteres: o negrito continua só na palavra.
    #[test]
    fn styles_stay_on_their_characters() {
        let mut t = make("artistic", "um straße dois");
        text::apply_style_range(
            &mut t,
            3,
            9,
            &text::TextStyle {
                bold: Some(true),
                ..Default::default()
            },
        );
        change_case_all(&mut t, CaseMode::Upper);
        let spans: Vec<(&str, u16)> = t.paragraphs[0]
            .spans
            .iter()
            .map(|s| (s.text.as_str(), s.font.weight))
            .collect();
        assert_eq!(spans, [("UM ", 400), ("STRASSE", 700), (" DOIS", 400)]);
    }

    #[test]
    fn cycle_and_preview() {
        assert_eq!(CaseMode::Lower.cycle(), CaseMode::Upper);
        assert_eq!(CaseMode::Upper.cycle(), CaseMode::Title);
        assert_eq!(CaseMode::Title.cycle(), CaseMode::Lower);
        assert_eq!(CaseMode::Toggle.cycle(), CaseMode::Lower);
        let t = make("artistic", "abc def");
        assert_eq!(
            preview(&t, 4, 7, CaseMode::Upper),
            ("def".to_string(), "DEF".to_string())
        );
        for m in CaseMode::ALL {
            assert_eq!(CaseMode::from_id(m.id()), Some(m));
        }
    }
}
