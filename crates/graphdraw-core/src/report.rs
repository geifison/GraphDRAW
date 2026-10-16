//! Relatório de importação (GD-CDR-03, GD-EXP-05) e avisos de leitura
//! tolerante (GD-GDR-10).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub severity: Severity,
    /// Código estável (ex.: `cdr.color.rgb-only`), útil para filtrar.
    pub code: String,
    /// Mensagem em linguagem simples (GD-UI-14).
    pub message: String,
    /// Página (1-based), quando aplicável.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    /// Quantas ocorrências foram agrupadas neste item.
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// `gdr`, `cdr`, `cmx`.
    pub source_format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_version: Option<String>,
    pub items: Vec<ReportItem>,
}

impl ImportReport {
    pub fn new(source_format: &str) -> ImportReport {
        ImportReport {
            source_format: source_format.into(),
            detected_version: None,
            items: Vec::new(),
        }
    }

    /// Adiciona um item; itens iguais (código, página, objeto) são agrupados.
    pub fn push(
        &mut self,
        severity: Severity,
        code: &str,
        message: impl Into<String>,
        page: Option<usize>,
        object_id: Option<String>,
    ) {
        let message = message.into();
        if let Some(it) = self.items.iter_mut().find(|i| {
            i.code == code && i.page == page && i.object_id == object_id && i.message == message
        }) {
            it.count += 1;
            return;
        }
        self.items.push(ReportItem {
            severity,
            code: code.into(),
            message,
            page,
            object_id,
            count: 1,
        });
    }

    pub fn warn(&mut self, code: &str, message: impl Into<String>, page: Option<usize>) {
        self.push(Severity::Warning, code, message, page, None);
    }

    pub fn info(&mut self, code: &str, message: impl Into<String>, page: Option<usize>) {
        self.push(Severity::Info, code, message, page, None);
    }

    pub fn has(&self, code: &str) -> bool {
        self.items.iter().any(|i| i.code == code)
    }

    pub fn merge(&mut self, other: ImportReport) {
        for i in other.items {
            for _ in 0..i.count {
                self.push(
                    i.severity,
                    &i.code,
                    i.message.clone(),
                    i.page,
                    i.object_id.clone(),
                );
            }
        }
    }

    /// Texto legível para terminal.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        for i in &self.items {
            let sev = match i.severity {
                Severity::Info => "info",
                Severity::Warning => "aviso",
                Severity::Error => "erro",
            };
            let page = i.page.map(|p| format!(" [pág. {p}]")).unwrap_or_default();
            let count = if i.count > 1 {
                format!(" (×{})", i.count)
            } else {
                String::new()
            };
            s.push_str(&format!("{sev}{page}: {}{count}\n", i.message));
        }
        s
    }
}
