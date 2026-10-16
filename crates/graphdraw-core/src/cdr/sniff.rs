//! Detecção de `.cdr`/`.cmx` pelo conteúdo (GD-CDR-01) e da versão do
//! formato (GD-CDR-06). A regra de versão segue o comportamento documentado
//! no código aberto da libcdr: assinatura RIFF `CDR?`, onde o 4º caractere
//! codifica a versão; arquivos recentes são ZIP com o RIFF em
//! `content/riffData.cdr` ou `content/root.dat`.

use std::io::{Cursor, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniffed {
    /// `cdr` ou `cmx`.
    pub format: &'static str,
    /// Versão numérica do formato (ex.: 13), quando detectável.
    pub version: Option<u32>,
}

impl Sniffed {
    pub fn version_label(&self) -> String {
        match self.version {
            Some(v) => format!("{} versão {v}", self.format.to_uppercase()),
            None => format!("{} versão desconhecida", self.format.to_uppercase()),
        }
    }
}

fn riff_version(b: &[u8]) -> Option<u32> {
    if b.len() >= 4 && b[0] == b'W' && b[1] == b'L' {
        return Some(2);
    }
    if b.len() < 12 || &b[0..4] != b"RIFF" || !b[8..11].eq_ignore_ascii_case(b"CDR") {
        return None;
    }
    let c = b[11];
    match c {
        0x20 => Some(3),
        b'1'..=b'9' => Some((c - b'0') as u32),
        b'A'..=b'Z' => Some((c - b'A') as u32 + 10),
        b'a'..=b'z' => Some((c - b'a') as u32 + 10),
        _ => None,
    }
}

pub fn sniff(bytes: &[u8]) -> Option<Sniffed> {
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..11] == b"CMX" {
        return Some(Sniffed {
            format: "cmx",
            version: None,
        });
    }
    if let Some(v) = riff_version(bytes) {
        return Some(Sniffed {
            format: "cdr",
            version: Some(v),
        });
    }
    if bytes.starts_with(b"PK\x03\x04") {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
        for name in ["content/riffData.cdr", "content/root.dat"] {
            if let Ok(mut f) = zip.by_name(name) {
                let mut head = [0u8; 12];
                let n = f.read(&mut head).unwrap_or(0);
                return Some(Sniffed {
                    format: "cdr",
                    version: riff_version(&head[..n]),
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riff_versions() {
        let mk = |c: u8| {
            let mut v = b"RIFF\0\0\0\0CDR".to_vec();
            v.push(c);
            v
        };
        assert_eq!(sniff(&mk(b'9')).unwrap().version, Some(9));
        assert_eq!(sniff(&mk(b'D')).unwrap().version, Some(13));
        assert_eq!(sniff(&mk(b' ')).unwrap().version, Some(3));
        assert_eq!(sniff(b"RIFF\0\0\0\0CMX1").unwrap().format, "cmx");
        assert!(sniff(b"garbage-bytes!").is_none());
    }
}
