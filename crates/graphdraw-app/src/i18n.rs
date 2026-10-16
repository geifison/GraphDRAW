//! Textos da interface (GD-UI-20): pt-BR (padrão) e en.
//! Idioma escolhido por `GRAPHDRAW_LANG` ou `LANG` (`en*` → inglês).
//! Cada texto aparece junto, nas duas línguas: `tr("Abrir…", "Open…")`.

use std::sync::OnceLock;

pub fn english() -> bool {
    static LANG: OnceLock<bool> = OnceLock::new();
    *LANG.get_or_init(|| {
        let lang = std::env::var("GRAPHDRAW_LANG")
            .or_else(|_| std::env::var("LANG"))
            .unwrap_or_default();
        lang.to_ascii_lowercase().starts_with("en")
    })
}

pub fn tr<'a>(pt: &'a str, en: &'a str) -> &'a str {
    if english() { en } else { pt }
}
