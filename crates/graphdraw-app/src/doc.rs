//! Documento aberto: editor (com histórico), arquivo, seleção e caches.

use std::path::PathBuf;
use std::sync::Arc;

use graphdraw_core::ImportReport;
use graphdraw_core::edit::Editor;
use graphdraw_core::edit::nodes::NodeRef;
use graphdraw_core::model::{Color, Document, Object, Page, Stroke};
use graphdraw_core::render::{ImageCache, Scene, SceneOptions, build_scene};

pub struct DocState {
    pub editor: Editor,
    /// Arquivo de origem; `None` = documento novo.
    pub path: Option<PathBuf>,
    /// Origem não é `.gdr` (ex.: importado de `.cdr`): "Salvar" pede um nome.
    pub needs_save_as: bool,
    pub report: ImportReport,
    pub page: usize,
    /// Seleção ordenada (GD-SEL-01); a seleção não entra no histórico (GD-UND-06).
    pub selection: Vec<String>,
    /// Nós selecionados na ferramenta Forma.
    pub nodes: Vec<NodeRef>,
    scene: Option<(u64, usize, Arc<Scene>)>,
    images: ImageCache,
}

impl DocState {
    pub fn new_blank() -> DocState {
        let mut doc = Document::new();
        let mut page = Page::new(210_000.0, 297_000.0);
        page.name = Some("Página 1".into());
        doc.data.pages.push(page);
        DocState::from_document(doc, None, ImportReport::new("gdr"))
    }

    pub fn from_document(doc: Document, path: Option<PathBuf>, report: ImportReport) -> DocState {
        let needs_save_as = path
            .as_ref()
            .is_some_and(|p| !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdr")));
        DocState {
            editor: Editor::new(doc),
            path,
            needs_save_as,
            report,
            page: 0,
            selection: Vec::new(),
            nodes: Vec::new(),
            scene: None,
            images: ImageCache::default(),
        }
    }

    pub fn doc(&self) -> &Document {
        &self.editor.doc
    }

    pub fn page(&self) -> &Page {
        &self.editor.doc.data.pages[self.page]
    }

    pub fn page_size(&self) -> (f64, f64) {
        let p = self.page();
        (p.width, p.height)
    }

    /// Cena da página atual (tela: mostra também o que não imprime).
    pub fn scene(&mut self) -> Arc<Scene> {
        let rev = self.editor.revision();
        if let Some((r, p, s)) = &self.scene {
            if *r == rev && *p == self.page {
                return s.clone();
            }
        }
        let s = Arc::new(build_scene(
            self.doc(),
            self.page(),
            SceneOptions { print: false },
            &self.images,
        ));
        self.scene = Some((rev, self.page, s.clone()));
        s
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| crate::i18n::tr("Sem título", "Untitled").into());
        if self.editor.is_modified() {
            format!("{name} •")
        } else {
            name
        }
    }

    /// Camada onde novos objetos entram: a mais alta visível e destravada.
    pub fn active_layer(&self) -> Option<usize> {
        self.page()
            .layers
            .iter()
            .rposition(|l| l.visible && !l.locked)
    }

    pub fn selected_objects(&self) -> Vec<&Object> {
        graphdraw_core::edit::ops::selected_objects(&self.editor, &self.selection)
    }

    /// Remove da seleção IDs que deixaram de existir (após desfazer, por ex.).
    pub fn prune_selection(&mut self) {
        let ed = &self.editor;
        let page = self.page;
        self.selection
            .retain(|id| ed.locate(id).is_some_and(|l| l.page == page));
        if self.selection.len() != 1 {
            self.nodes.clear();
        }
    }

    pub fn set_page(&mut self, page: usize) {
        if page < self.doc().data.pages.len() && page != self.page {
            self.page = page;
            self.selection.clear();
            self.nodes.clear();
        }
    }
}

/// Estilo padrão de objetos novos: sem preenchimento, contorno preto 0,2 mm.
pub fn default_stroke() -> Stroke {
    Stroke::new(200.0, Color::cmyk(0.0, 0.0, 0.0, 1.0))
}
