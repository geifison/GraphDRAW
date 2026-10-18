//! Hifenização (GD-TXT-07): pontos onde uma palavra pode ser partida no
//! fim da linha.
//!
//! - Com um dicionário de padrões no formato `hyph_<idioma>.dic` (o dos
//!   pacotes hyphen/Hunspell, algoritmo de Liang) na pasta
//!   `%APPDATA%\GraphDraw\hifenizacao` (ou em `GRAPHDRAW_HYPH_DIR`), valem
//!   os padrões.
//! - Sem dicionário, o português usa regras de divisão silábica: separa
//!   antes de consoante única entre vogais, entre consoantes iguais ou que
//!   não formam grupo (rr, ss, sc, xc, rn…), e mantém juntos ch, lh, nh,
//!   qu, gu e os grupos com l ou r (pr, bl, tr…). Vogais vizinhas não se
//!   separam (ditongos e hiatos ficam juntos, por segurança).
//! - Em qualquer caso, ficam ao menos 2 letras de cada lado.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const MIN_LEFT: usize = 2;
const MIN_RIGHT: usize = 2;

/// Padrões de Liang: letras → números entre letras.
struct Patterns {
    map: HashMap<String, Vec<u8>>,
    max_len: usize,
}

fn parse_patterns(text: &str) -> Patterns {
    let mut map = HashMap::new();
    let mut max_len = 0;
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        // Primeira linha = codificação; linhas com maiúsculas são diretivas.
        if i == 0
            || line.is_empty()
            || line.starts_with('%')
            || line.starts_with("LEFTHYPHENMIN")
            || line.starts_with("RIGHTHYPHENMIN")
            || line.starts_with("COMPOUND")
            || line.starts_with("NOHYPHEN")
        {
            continue;
        }
        let mut letters = String::new();
        let mut nums: Vec<u8> = vec![0];
        for c in line.chars() {
            if let Some(d) = c.to_digit(10) {
                *nums.last_mut().unwrap() = d as u8;
            } else {
                letters.push(c);
                nums.push(0);
            }
        }
        max_len = max_len.max(letters.chars().count());
        map.insert(letters, nums);
    }
    Patterns { map, max_len }
}

fn hyph_dir() -> Option<std::path::PathBuf> {
    if let Ok(d) = std::env::var("GRAPHDRAW_HYPH_DIR") {
        return Some(d.into());
    }
    std::env::var_os("APPDATA").map(|a| {
        std::path::PathBuf::from(a)
            .join("GraphDraw")
            .join("hifenizacao")
    })
}

/// Dicionário do idioma (em cache; `None` se não houver arquivo).
fn patterns_for(lang: &str) -> Option<std::sync::Arc<Patterns>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<std::sync::Arc<Patterns>>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = lang.replace('-', "_");
    if let Some(v) = cache.lock().ok()?.get(&key) {
        return v.clone();
    }
    let dir = hyph_dir();
    let short = key.split('_').next().unwrap_or(&key).to_string();
    let found = dir.and_then(|d| {
        [format!("hyph_{key}.dic"), format!("hyph_{short}.dic")]
            .iter()
            .find_map(|n| std::fs::read(d.join(n)).ok())
    });
    let p = found.map(|b| std::sync::Arc::new(parse_patterns(&String::from_utf8_lossy(&b))));
    if let Ok(mut c) = cache.lock() {
        c.insert(key, p.clone());
    }
    p
}

fn liang(p: &Patterns, word: &[char]) -> Vec<usize> {
    let w: Vec<char> = std::iter::once('.')
        .chain(word.iter().flat_map(|c| c.to_lowercase()))
        .chain(std::iter::once('.'))
        .collect();
    let mut score = vec![0u8; w.len() + 1];
    for i in 0..w.len() {
        for len in 1..=p.max_len.min(w.len() - i) {
            let s: String = w[i..i + len].iter().collect();
            if let Some(nums) = p.map.get(&s) {
                for (k, n) in nums.iter().enumerate() {
                    score[i + k] = score[i + k].max(*n);
                }
            }
        }
    }
    // score[j] fica entre w[j-1] e w[j]; a palavra começa em w[1].
    (1..word.len()).filter(|&k| score[k + 1] % 2 == 1).collect()
}

fn is_vowel(c: char) -> bool {
    "aeiouáéíóúâêôãõàüy".contains(c)
}

/// Grupos de duas consoantes que não se separam.
fn inseparable(a: char, b: char) -> bool {
    matches!(
        (a, b),
        ('c', 'h') | ('l', 'h') | ('n', 'h') | ('q', 'u') | ('g', 'u')
    ) || (matches!(b, 'l' | 'r')
        && "pbtdcgfv".contains(a)
        && !(a == 'd' && b == 'l')
        && !(a == 'v' && b == 'l'))
}

fn pt_rules(word: &[char]) -> Vec<usize> {
    let w: Vec<char> = word
        .iter()
        .map(|c| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let n = w.len();
    // "u" de qu/gu antes de vogal conta como consoante.
    let vowel: Vec<bool> = (0..n)
        .map(|i| {
            if w[i] == 'u'
                && i > 0
                && matches!(w[i - 1], 'q' | 'g')
                && i + 1 < n
                && is_vowel(w[i + 1])
            {
                false
            } else {
                is_vowel(w[i])
            }
        })
        .collect();
    if !w.iter().all(|c| c.is_alphabetic()) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    // Pula até a primeira vogal.
    while i < n && !vowel[i] {
        i += 1;
    }
    while i < n {
        // Fim do grupo de vogais.
        while i < n && vowel[i] {
            i += 1;
        }
        let c0 = i;
        while i < n && !vowel[i] {
            i += 1;
        }
        if i >= n {
            break; // consoantes finais ficam com a sílaba
        }
        let cs = &w[c0..i];
        let at = match cs.len() {
            1 => c0,
            2 if inseparable(cs[0], cs[1]) => c0,
            2 => c0 + 1,
            k if inseparable(cs[k - 2], cs[k - 1]) => i - 2,
            _ => i - 1,
        };
        out.push(at);
    }
    out
}

/// Posições (índice de caractere) onde `word` pode receber hífen no idioma
/// `lang` (ex.: `pt-BR`).
pub fn points(word: &str, lang: &str) -> Vec<usize> {
    let chars: Vec<char> = word.chars().collect();
    let n = chars.len();
    if n < MIN_LEFT + MIN_RIGHT {
        return Vec::new();
    }
    let pts = match patterns_for(lang) {
        Some(p) => liang(&p, &chars),
        None if lang.to_lowercase().starts_with("pt") => pt_rules(&chars),
        None => Vec::new(),
    };
    pts.into_iter()
        .filter(|&k| k >= MIN_LEFT && n - k >= MIN_RIGHT)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(w: &str) -> String {
        let p = points(w, "pt-BR");
        let mut s = String::new();
        for (i, c) in w.chars().enumerate() {
            if p.contains(&i) {
                s.push('-');
            }
            s.push(c);
        }
        s
    }

    /// GD-TXT-07: regras do português (sem dicionário instalado).
    #[test]
    fn gd_txt_07_portuguese_rules() {
        if patterns_for("pt_BR").is_some() {
            return; // com dicionário instalado valem os padrões dele
        }
        assert_eq!(split("casa"), "ca-sa");
        assert_eq!(split("carro"), "car-ro");
        assert_eq!(split("pássaro"), "pás-sa-ro");
        assert_eq!(split("chocolate"), "cho-co-la-te");
        assert_eq!(split("pequeno"), "pe-que-no");
        assert_eq!(split("abstrato"), "abs-tra-to");
        assert_eq!(split("perspectiva"), "pers-pec-ti-va");
        assert_eq!(split("trabalho"), "tra-ba-lho");
        assert_eq!(split("impressão"), "im-pres-são");
        assert_eq!(split("saudade"), "sau-da-de");
        assert_eq!(split("ao"), "ao");
        assert!(points("casa", "en").is_empty(), "sem regra nem dicionário");
    }

    /// Padrões de Liang no formato hyphen.
    #[test]
    fn liang_patterns() {
        let p = parse_patterns("UTF-8\n1ba\n1ca\n.ab4\n");
        let w: Vec<char> = "abacaxi".chars().collect();
        // 1ba: antes do b (1); 1ca: antes do c (3); .ab4 proíbe antes do 2º a.
        assert_eq!(liang(&p, &w), vec![1, 3]);
    }
}
