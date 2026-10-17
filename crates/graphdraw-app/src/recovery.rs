//! Salvamento automático e recuperação depois de uma queda
//! (docs/PRODUCAO.md, P0). Cada execução tem uma etiqueta própria e grava
//! uma cópia `<etiqueta>-<documento>.gdr` (mais os dados em `.json`) dos
//! documentos alterados na pasta de recuperação. Enquanto roda, segura o
//! arquivo `<etiqueta>.alive` aberto: no Windows ele não pode ser apagado,
//! e é assim que outra execução sabe que aquelas cópias não são órfãs.
//! Saída normal apaga tudo da execução; o que sobra de uma queda é
//! oferecido no próximo início.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use graphdraw_core::model::Document;
use serde::{Deserialize, Serialize};

/// Pasta de configuração do GraphDraw (`%APPDATA%\GraphDraw`).
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("GraphDraw"))
}

/// Intervalo do salvamento automático (s).
pub const INTERVAL_S: f64 = 120.0;

/// Pasta das cópias de recuperação (nada nos testes).
pub fn dir() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    config_dir().map(|d| d.join("recuperacao"))
}

/// Dados de uma cópia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub title: String,
    /// Arquivo de origem do documento, se tinha.
    pub original: Option<PathBuf>,
    /// Segundos desde 1970 (UTC).
    pub saved_at: u64,
}

/// Cópia órfã de uma execução que caiu.
#[derive(Debug, Clone, PartialEq)]
pub struct Orphan {
    pub file: PathBuf,
    pub meta: Meta,
}

impl Orphan {
    /// Apaga a cópia e os dados dela.
    pub fn discard(&self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_file(self.file.with_extension("json"));
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Cria e segura o arquivo sem permitir que outro processo o apague.
fn hold(path: &Path) -> Option<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_SHARE_READ: sem FILE_SHARE_DELETE.
        o.share_mode(0x1);
    }
    o.open(path).ok()
}

/// Execução atual.
pub struct Session {
    dir: PathBuf,
    tag: String,
    alive: Option<std::fs::File>,
    /// Revisão já copiada por documento.
    saved: HashMap<String, u64>,
}

impl Session {
    pub fn start(dir: &Path) -> Option<Session> {
        std::fs::create_dir_all(dir).ok()?;
        let tag = format!(
            "{}x{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis())
        );
        let alive = hold(&dir.join(format!("{tag}.alive")));
        Some(Session {
            dir: dir.to_path_buf(),
            tag,
            alive,
            saved: HashMap::new(),
        })
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    fn file(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{}-{key}.gdr", self.tag))
    }

    /// Precisa de cópia nova (a revisão mudou desde a última)?
    pub fn stale(&self, key: &str, rev: u64) -> bool {
        self.saved.get(key) != Some(&rev)
    }

    /// Grava a cópia numa linha de execução à parte (o documento já vem
    /// clonado). Devolve a linha para quem quiser esperar.
    pub fn save(
        &mut self,
        key: &str,
        rev: u64,
        doc: Document,
        meta: Meta,
    ) -> std::thread::JoinHandle<()> {
        self.saved.insert(key.to_string(), rev);
        let file = self.file(key);
        std::thread::spawn(move || {
            let Ok(bytes) = graphdraw_core::gdr::write_bytes(&doc, &Default::default()) else {
                return;
            };
            if graphdraw_core::gdr::write_atomic(&file, &bytes).is_ok()
                && let Ok(json) = serde_json::to_vec_pretty(&meta)
            {
                let _ = graphdraw_core::gdr::write_atomic(&file.with_extension("json"), &json);
            }
        })
    }

    /// Esquece a cópia (documento salvo ou fechado).
    pub fn forget(&mut self, key: &str) {
        if self.saved.remove(key).is_some() {
            let f = self.file(key);
            let _ = std::fs::remove_file(&f);
            let _ = std::fs::remove_file(f.with_extension("json"));
        }
    }

    /// Esquece as cópias de documentos que não estão mais abertos.
    pub fn sweep(&mut self, open: &[String]) {
        let gone: Vec<String> = self
            .saved
            .keys()
            .filter(|k| !open.contains(k))
            .cloned()
            .collect();
        for k in gone {
            self.forget(&k);
        }
    }

    /// Saída normal: apaga tudo desta execução.
    pub fn end(mut self) {
        let keys: Vec<String> = self.saved.keys().cloned().collect();
        for k in keys {
            self.forget(&k);
        }
        self.alive.take();
        let _ = std::fs::remove_file(self.dir.join(format!("{}.alive", self.tag)));
    }
}

/// Cópias deixadas por execuções que caíram (não as de `own` nem as de
/// execuções ainda abertas), mais novas primeiro.
pub fn orphans(dir: &Path, own: &str) -> Vec<Orphan> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut running: HashMap<String, bool> = HashMap::new();
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "gdr") {
            continue;
        }
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some((tag, _)) = name.split_once('-') else {
            continue;
        };
        if tag == own {
            continue;
        }
        // A execução dona ainda segura o `.alive`: não é órfã.
        let alive = *running.entry(tag.to_string()).or_insert_with(|| {
            let a = dir.join(format!("{tag}.alive"));
            a.exists() && std::fs::remove_file(&a).is_err()
        });
        if alive {
            continue;
        }
        let meta = std::fs::read(path.with_extension("json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Meta>(&b).ok())
            .unwrap_or(Meta {
                title: name.clone(),
                original: None,
                saved_at: 0,
            });
        out.push(Orphan { file: path, meta });
    }
    out.sort_by_key(|o| std::cmp::Reverse(o.meta.saved_at));
    out
}

/// Pânico: grava versão, sistema, mensagem e pilha em
/// `falhas/falha-<segundos>.txt` (docs/PRODUCAO.md, P0).
pub fn install_panic_log() {
    let Some(dir) = config_dir().map(|d| d.join("falhas")) else {
        return;
    };
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::create_dir_all(&dir);
        let text = format!(
            "GraphDraw {}\n{} {}\n\n{info}\n\n{}\n",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::backtrace::Backtrace::force_capture()
        );
        let _ = std::fs::write(dir.join(format!("falha-{}.txt", now_secs())), text);
        prev(info);
    }));
}

/// Registro de falha ainda não mostrado; marca como visto (`.visto`).
pub fn take_crash_log() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let dir = config_dir()?.join("falhas");
    let mut logs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    logs.sort();
    let last = logs.pop()?;
    let seen = last.with_extension("visto.txt");
    for l in logs {
        let _ = std::fs::rename(&l, l.with_extension("visto.txt"));
    }
    std::fs::rename(&last, &seen).ok()?;
    Some(seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("graphdraw-rec-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn meta(t: &str) -> Meta {
        Meta {
            title: t.into(),
            original: None,
            saved_at: now_secs(),
        }
    }

    /// Cópia da execução que caiu é oferecida; a saída normal não deixa nada.
    #[test]
    fn crash_leaves_orphan_and_clean_exit_does_not() {
        let d = tmp("orfa");
        let mut a = Session::start(&d).unwrap();
        a.save("doc1", 3, Document::new(), meta("Cartaz"))
            .join()
            .unwrap();
        assert!(!a.stale("doc1", 3) && a.stale("doc1", 4));
        // Queda: a execução some sem `end` (o `.alive` fica, mas solto).
        let tag_a = a.tag().to_string();
        drop(a);
        let b = Session::start(&d).unwrap();
        let o = orphans(&d, b.tag());
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].meta.title, "Cartaz");
        assert!(
            o[0].file
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&tag_a)
        );
        let (doc, _) = graphdraw_core::gdr::read_file(&o[0].file).unwrap();
        assert_eq!(doc.data.pages.len(), Document::new().data.pages.len());
        o[0].discard();
        assert!(orphans(&d, b.tag()).is_empty());
        // Saída normal: nada sobra.
        let mut c = b;
        c.save("doc2", 1, Document::new(), meta("Folheto"))
            .join()
            .unwrap();
        c.end();
        let left: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().collect();
        assert!(left.is_empty(), "{left:?}");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Salvar (ou fechar) o documento apaga a cópia; o que fechou sai na varredura.
    #[test]
    fn forget_and_sweep_remove_copies() {
        let d = tmp("varre");
        let mut s = Session::start(&d).unwrap();
        s.save("a", 1, Document::new(), meta("A")).join().unwrap();
        s.save("b", 1, Document::new(), meta("B")).join().unwrap();
        s.forget("a");
        s.sweep(&[]);
        let gdr = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "gdr"))
            .count();
        assert_eq!(gdr, 0);
        s.end();
        let _ = std::fs::remove_dir_all(&d);
    }

    /// No Windows, cópias de uma execução ainda aberta não são órfãs.
    #[cfg(windows)]
    #[test]
    fn running_session_is_not_orphan() {
        let d = tmp("viva");
        let mut a = Session::start(&d).unwrap();
        a.save("x", 1, Document::new(), meta("X")).join().unwrap();
        let b = Session::start(&d).unwrap();
        assert!(orphans(&d, b.tag()).is_empty());
        a.end();
        b.end();
        let _ = std::fs::remove_dir_all(&d);
    }
}
