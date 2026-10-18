//! Corretor ortográfico (GD-TXT-08) pelo corretor do sistema operacional:
//! no Windows, `ISpellChecker`, com os dicionários de idioma instalados no
//! Windows (Configurações → Idioma). Em outros sistemas fica desligado.

/// Palavra com erro: posição (caractere) e comprimento no texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Miss {
    pub start: usize,
    pub len: usize,
}

#[cfg(windows)]
mod imp {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use windows::Win32::Globalization::{ISpellChecker, ISpellCheckerFactory, SpellCheckerFactory};
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree,
    };
    use windows::core::{HSTRING, PWSTR};

    thread_local! {
        static CHECKERS: RefCell<HashMap<String, Option<ISpellChecker>>> = RefCell::new(HashMap::new());
    }

    fn checker(lang: &str) -> Option<ISpellChecker> {
        CHECKERS.with(|m| {
            m.borrow_mut()
                .entry(lang.to_string())
                .or_insert_with(|| unsafe {
                    // Já inicializado pela janela (S_FALSE ou outro modo): segue.
                    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                    let f: ISpellCheckerFactory =
                        CoCreateInstance(&SpellCheckerFactory, None, CLSCTX_INPROC_SERVER).ok()?;
                    let tag = HSTRING::from(lang);
                    if !f.IsSupported(&tag).ok()?.as_bool() {
                        return None;
                    }
                    f.CreateSpellChecker(&tag).ok()
                })
                .clone()
        })
    }

    pub fn available(lang: &str) -> bool {
        checker(lang).is_some()
    }

    pub fn check(text: &str, lang: &str) -> Vec<super::Miss> {
        let Some(c) = checker(lang) else {
            return Vec::new();
        };
        // Índices UTF-16 do Windows → caracteres.
        let mut u16_to_char = Vec::new();
        for (ci, ch) in text.chars().enumerate() {
            for _ in 0..ch.len_utf16() {
                u16_to_char.push(ci);
            }
        }
        u16_to_char.push(text.chars().count());
        let mut out = Vec::new();
        unsafe {
            let Ok(errs) = c.Check(&HSTRING::from(text)) else {
                return out;
            };
            loop {
                let mut e = None;
                if errs.Next(&mut e).is_err() {
                    break;
                }
                let Some(e) = e else { break };
                let (Ok(s), Ok(l)) = (e.StartIndex(), e.Length()) else {
                    break;
                };
                let (s, l) = (s as usize, l as usize);
                if s + l < u16_to_char.len() {
                    let a = u16_to_char[s];
                    let b = u16_to_char[s + l];
                    out.push(super::Miss {
                        start: a,
                        len: b - a,
                    });
                }
            }
        }
        out
    }

    pub fn suggest(word: &str, lang: &str) -> Vec<String> {
        let Some(c) = checker(lang) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        unsafe {
            let Ok(en) = c.Suggest(&HSTRING::from(word)) else {
                return out;
            };
            loop {
                let mut item = [PWSTR::null()];
                let mut n = 0u32;
                if en.Next(&mut item, Some(&mut n)).is_err() || n == 0 {
                    break;
                }
                if let Ok(s) = item[0].to_string() {
                    out.push(s);
                }
                CoTaskMemFree(Some(item[0].0 as *const _));
                if out.len() >= 8 {
                    break;
                }
            }
        }
        out
    }

    pub fn add(word: &str, lang: &str) {
        if let Some(c) = checker(lang) {
            unsafe {
                let _ = c.Add(&HSTRING::from(word));
            }
        }
    }

    pub fn ignore(word: &str, lang: &str) {
        if let Some(c) = checker(lang) {
            unsafe {
                let _ = c.Ignore(&HSTRING::from(word));
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn available(_: &str) -> bool {
        false
    }
    pub fn check(_: &str, _: &str) -> Vec<super::Miss> {
        Vec::new()
    }
    pub fn suggest(_: &str, _: &str) -> Vec<String> {
        Vec::new()
    }
    pub fn add(_: &str, _: &str) {}
    pub fn ignore(_: &str, _: &str) {}
}

pub use imp::{add, available, check, ignore, suggest};

#[cfg(test)]
mod tests {
    use super::*;

    /// Com o português instalado no Windows, "casa" passa e "cazza" não.
    #[test]
    fn checks_when_the_system_has_the_language() {
        if !available("pt-BR") {
            return;
        }
        let m = check("A casa cazza azul.", "pt-BR");
        assert!(m.iter().any(|e| e.start == 7 && e.len == 5), "{m:?}");
        assert!(!m.iter().any(|e| e.start == 2), "casa está certa: {m:?}");
        assert!(!suggest("cazza", "pt-BR").is_empty());
    }
}
