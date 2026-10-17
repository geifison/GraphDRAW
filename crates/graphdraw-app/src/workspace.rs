//! Área de trabalho (GD-UI-03, DESIGN-SYSTEM 6): painéis encaixados à
//! direita, recolhidos numa faixa de ícones ou flutuando em janelas; a
//! posição, a largura e o estado ficam salvos no perfil do usuário
//! (`%APPDATA%\GraphDraw\area-de-trabalho.json`) e voltam ao abrir.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::app::RightTab;
use crate::canvas::Tool;
use crate::i18n::tr;

impl RightTab {
    pub const ALL: [RightTab; 7] = [
        RightTab::Transform,
        RightTab::Align,
        RightTab::Layers,
        RightTab::Palettes,
        RightTab::History,
        RightTab::Pages,
        RightTab::Report,
    ];

    /// Chave estável no arquivo.
    pub fn key(self) -> &'static str {
        match self {
            RightTab::Transform => "transformar",
            RightTab::Align => "alinhar",
            RightTab::Layers => "camadas",
            RightTab::Palettes => "paletas",
            RightTab::History => "historico",
            RightTab::Pages => "paginas",
            RightTab::Report => "relatorio",
        }
    }

    pub fn from_key(k: &str) -> Option<RightTab> {
        RightTab::ALL.into_iter().find(|t| t.key() == k)
    }

    pub fn label(self) -> &'static str {
        match self {
            RightTab::Transform => tr("Transformar", "Transform"),
            RightTab::Align => tr("Alinhar", "Align"),
            RightTab::Layers => tr("Camadas", "Layers"),
            RightTab::Palettes => tr("Paletas", "Palettes"),
            RightTab::History => tr("Histórico", "History"),
            RightTab::Pages => tr("Páginas", "Pages"),
            RightTab::Report => tr("Relatório", "Report"),
        }
    }

    /// Ícone na faixa do painel recolhido.
    pub fn icon(self) -> &'static str {
        match self {
            RightTab::Transform => "rotate-cw",
            RightTab::Align => "align-left",
            RightTab::Layers => "layers",
            RightTab::Palettes => "palette",
            RightTab::History => "history",
            RightTab::Pages => "page-list",
            RightTab::Report => "report",
        }
    }
}

/// Painel solto numa janela própria.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Floating {
    pub tab: String,
    /// Canto superior esquerdo e tamanho, em pontos da tela.
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

/// Estado salvo da área de trabalho.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    /// Coluna de painéis visível.
    pub panels: bool,
    /// Coluna recolhida na faixa de ícones.
    pub collapsed: bool,
    /// Largura da coluna aberta.
    pub width: f32,
    /// Aba ativa da coluna.
    pub tab: String,
    pub floating: Vec<Floating>,
    /// Ferramenta ativa (pelo nome do ícone).
    pub tool: String,
    /// Densidade dos painéis (`compacta`, `padrao`, `ampla`).
    pub density: String,
}

impl Default for Workspace {
    fn default() -> Self {
        Workspace {
            panels: true,
            collapsed: false,
            width: crate::theme::PANEL_WIDTH,
            tab: RightTab::Transform.key().into(),
            floating: Vec::new(),
            tool: Tool::Select.info().0.into(),
            density: crate::theme::Density::Default.key().into(),
        }
    }
}

impl Workspace {
    pub fn tab(&self) -> RightTab {
        RightTab::from_key(&self.tab).unwrap_or(RightTab::Transform)
    }

    pub fn tool(&self) -> Tool {
        Tool::ALL
            .into_iter()
            .find(|t| t.info().0 == self.tool)
            .unwrap_or(Tool::Select)
    }

    pub fn is_floating(&self, t: RightTab) -> bool {
        self.floating.iter().any(|f| f.tab == t.key())
    }

    /// Abas que ficam na coluna (as soltas saem dela).
    pub fn docked(&self) -> Vec<RightTab> {
        RightTab::ALL
            .into_iter()
            .filter(|t| !self.is_floating(*t))
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Lê o arquivo; abas desconhecidas e repetidas são descartadas.
    pub fn from_json(text: &str) -> Option<Workspace> {
        let mut w: Workspace = serde_json::from_str(text).ok()?;
        let mut seen = Vec::new();
        w.floating.retain(|f| {
            let ok = RightTab::from_key(&f.tab).is_some() && !seen.contains(&f.tab);
            seen.push(f.tab.clone());
            ok
        });
        w.width = w
            .width
            .clamp(crate::theme::PANEL_MIN_WIDTH, crate::theme::PANEL_MAX_WIDTH);
        Some(w)
    }

    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("GraphDraw").join("area-de-trabalho.json"))
    }

    /// A área salva; sem arquivo (ou com arquivo inválido), a padrão.
    pub fn load() -> Workspace {
        Workspace::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| Workspace::from_json(&t))
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Workspace::path().ok_or("pasta de configuração não encontrada")?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(&p, self.to_json()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A largura lida da coluna é a mesma passada como inicial: reabrir o
    /// programa não faz a coluna crescer.
    #[test]
    fn panel_width_is_stable_across_restarts() {
        use eframe::egui;
        for want in [crate::theme::PANEL_WIDTH, 333.0] {
            let ctx = crate::theme::test_ctx();
            let mut got = 0.0;
            for _ in 0..2 {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(1366.0, 768.0),
                    )),
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let r = egui::Panel::right("paineis")
                        .default_size(want)
                        .size_range(crate::theme::PANEL_MIN_WIDTH..=crate::theme::PANEL_MAX_WIDTH)
                        .show(ui, |ui| {
                            // Como os painéis reais: o conteúdo ocupa a largura.
                            ui.allocate_space(egui::Vec2::new(ui.available_width(), 10.0));
                        });
                    got = r.response.rect.width().round();
                });
                out.textures_delta.clear();
            }
            assert_eq!(got, want);
        }
    }

    #[test]
    fn roundtrip_and_cleanup() {
        let w = Workspace {
            panels: true,
            collapsed: true,
            width: 300.0,
            tab: "camadas".into(),
            floating: vec![Floating {
                tab: "historico".into(),
                pos: [100.0, 120.0],
                size: [240.0, 320.0],
            }],
            tool: "tool-rect".into(),
            density: "ampla".into(),
        };
        let back = Workspace::from_json(&w.to_json()).unwrap();
        assert_eq!(back, w);
        assert_eq!((back.tab(), back.tool()), (RightTab::Layers, Tool::Rect));
        assert!(!back.docked().contains(&RightTab::History));
        // Arquivo antigo/estragado: campos que faltam usam o padrão; abas
        // desconhecidas ou repetidas saem; a largura fica nos limites.
        let odd = r#"{"width": 5000, "floating": [
            {"tab": "nada", "pos": [0,0], "size": [1,1]},
            {"tab": "alinhar", "pos": [0,0], "size": [1,1]},
            {"tab": "alinhar", "pos": [9,9], "size": [1,1]}]}"#;
        let w = Workspace::from_json(odd).unwrap();
        assert!(w.panels && !w.collapsed);
        assert_eq!(w.floating.len(), 1);
        assert_eq!(w.width, crate::theme::PANEL_MAX_WIDTH);
        assert!(Workspace::from_json("lixo").is_none());
    }
}
