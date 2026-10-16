//! Documento aberto: editor (com histórico), arquivo, seleção e caches.

use std::path::PathBuf;
use std::sync::Arc;

use graphdraw_core::ImportReport;
use graphdraw_core::edit::nodes::NodeRef;
use graphdraw_core::edit::{Editor, MASTER};
use graphdraw_core::model::{Color, Document, Layer, Object, Page, Stroke};
use graphdraw_core::render::raster::{self, RasterOptions};
use graphdraw_core::render::{ImageCache, Scene, SceneOptions, build_scene};
use kurbo::Affine;

pub struct DocState {
    pub editor: Editor,
    /// Arquivo de origem; `None` = documento novo.
    pub path: Option<PathBuf>,
    /// Origem não é `.gdr` (ex.: importado de `.cdr`): "Salvar" pede um nome.
    pub needs_save_as: bool,
    pub report: ImportReport,
    /// Página em edição; `MASTER` = página mestra (GD-PGT-21).
    pub page: usize,
    /// Última página comum visitada: dá o tamanho da página mestra na tela.
    pub last_page: usize,
    /// Seleção ordenada (GD-SEL-01); a seleção não entra no histórico (GD-UND-06).
    pub selection: Vec<String>,
    /// Nós selecionados na ferramenta Forma.
    pub nodes: Vec<NodeRef>,
    /// Camada ativa escolhida no painel (por ID, para sobreviver a desfazer).
    pub layer_id: Option<String>,
    /// Cópia da página mestra como `Page`, para as consultas que pedem uma
    /// página (acerto de clique, encaixe, camadas). Refeita a cada alteração.
    master_view: Page,
    master_rev: u64,
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
        let mut s = DocState {
            editor: Editor::new(doc),
            path,
            needs_save_as,
            report,
            page: 0,
            last_page: 0,
            selection: Vec::new(),
            nodes: Vec::new(),
            layer_id: None,
            master_view: Page::new(1.0, 1.0),
            master_rev: u64::MAX,
            scene: None,
            images: ImageCache::default(),
        };
        s.sync();
        s
    }

    pub fn doc(&self) -> &Document {
        &self.editor.doc
    }

    pub fn is_master(&self) -> bool {
        self.page == MASTER
    }

    pub fn page(&self) -> &Page {
        if self.page == MASTER {
            &self.master_view
        } else {
            &self.editor.doc.data.pages[self.page]
        }
    }

    pub fn page_size(&self) -> (f64, f64) {
        let p = self.page();
        (p.width, p.height)
    }

    /// Página bloqueada (GD-PGT-18): nada nela é selecionável ou editável.
    pub fn page_locked(&self) -> bool {
        self.page().locked
    }

    /// Mantém a página atual válida (após excluir ou desfazer páginas) e a
    /// cópia da mestra em dia. Chamar uma vez por quadro.
    pub fn sync(&mut self) {
        let n = self.editor.doc.data.pages.len();
        if self.page != MASTER && self.page >= n {
            self.page = n.saturating_sub(1);
            self.selection.clear();
            self.nodes.clear();
        }
        if self.last_page >= n {
            self.last_page = n.saturating_sub(1);
        }
        if self.page != MASTER {
            self.last_page = self.page;
        }
        let rev = self.editor.revision();
        let base = &self.editor.doc.data.pages[self.last_page];
        let (w, h) = (base.width, base.height);
        if self.master_rev != rev || self.master_view.width != w || self.master_view.height != h {
            let mut v = Page::new(w, h);
            v.id = graphdraw_core::edit::history::MASTER_KEY.into();
            v.name = Some("Mestra".into());
            // As camadas da mestra já são a própria página aqui; não desenhar duas vezes.
            v.hide_master = true;
            v.layers = self.editor.doc.data.master_page.layers.clone();
            self.master_view = v;
            self.master_rev = rev;
            self.scene = None;
        }
    }

    /// Cena da página atual (tela: mostra também o que não imprime).
    pub fn scene(&mut self) -> Arc<Scene> {
        let rev = self.editor.revision();
        if let Some((r, p, s)) = &self.scene
            && *r == rev
            && *p == self.page
        {
            return s.clone();
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

    /// Miniatura da página `index` (ou da mestra) com lado maior `max_px`.
    pub fn thumbnail(&self, index: usize, max_px: f64) -> eframe::egui::ColorImage {
        let page = if index == MASTER {
            &self.master_view
        } else {
            &self.editor.doc.data.pages[index]
        };
        let scene = build_scene(
            self.doc(),
            page,
            SceneOptions { print: false },
            &self.images,
        );
        let s = max_px / page.width.max(page.height).max(1.0);
        let (w, h) = (
            (page.width * s).round().max(1.0) as u32,
            (page.height * s).round().max(1.0) as u32,
        );
        let view = Affine::new([s, 0.0, 0.0, -s, 0.0, page.height * s]);
        let opts = RasterOptions {
            antialias: true,
            background: Some([255, 255, 255, 255]),
        };
        match raster::render(&scene, view, w, h, &opts) {
            Some(pm) => eframe::egui::ColorImage::from_rgba_premultiplied(
                [w as usize, h as usize],
                pm.data(),
            ),
            None => eframe::egui::ColorImage::new([1, 1], vec![eframe::egui::Color32::WHITE]),
        }
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

    /// Camada onde novos objetos entram: a escolhida no painel de camadas,
    /// se estiver visível e destravada; senão a mais alta que estiver.
    pub fn active_layer(&self) -> Option<usize> {
        if self.page_locked() {
            return None;
        }
        let layers = &self.page().layers;
        let usable = |l: &Layer| l.visible && !l.locked;
        self.layer_id
            .as_ref()
            .and_then(|id| layers.iter().position(|l| &l.id == id))
            .filter(|i| usable(&layers[*i]))
            .or_else(|| layers.iter().rposition(usable))
    }

    /// Camada para um objeto novo. A página mestra começa sem camadas: na
    /// primeira vez, cria uma.
    pub fn layer_for_new_object(&mut self) -> Option<usize> {
        if let Some(l) = self.active_layer() {
            return Some(l);
        }
        if self.is_master() && self.page().layers.is_empty() {
            let layer = Layer::new(crate::i18n::tr("Fundo (mestra)", "Background (master)"));
            self.layer_id = Some(layer.id.clone());
            self.editor.transaction(
                crate::i18n::tr("Criar camada da mestra", "Create master layer"),
                |ed| ed.insert_layer(MASTER, 0, layer),
            );
            self.sync();
            return Some(0);
        }
        None
    }

    /// Camada marcada no painel (mesmo que esteja travada ou oculta).
    pub fn chosen_layer(&self) -> Option<usize> {
        let layers = &self.page().layers;
        self.layer_id
            .as_ref()
            .and_then(|id| layers.iter().position(|l| &l.id == id))
            .or_else(|| self.active_layer())
    }

    pub fn selected_objects(&self) -> Vec<&Object> {
        graphdraw_core::edit::ops::selected_objects(&self.editor, &self.selection)
    }

    /// Remove da seleção IDs que deixaram de existir (após desfazer, por ex.)
    /// ou cuja camada ou página ficou oculta ou bloqueada.
    pub fn prune_selection(&mut self) {
        self.sync();
        if self.page_locked() {
            self.selection.clear();
            self.nodes.clear();
            return;
        }
        let ed = &self.editor;
        let page = self.page;
        self.selection.retain(|id| {
            ed.find(id).is_some_and(|r| {
                let l = &ed.layers(r.loc.page)[r.loc.layer];
                r.loc.page == page && l.visible && !l.locked
            })
        });
        if self.selection.len() != 1 {
            self.nodes.clear();
        }
    }

    pub fn set_page(&mut self, page: usize) {
        let valid = page == MASTER || page < self.doc().data.pages.len();
        if valid && page != self.page {
            self.page = page;
            self.selection.clear();
            self.nodes.clear();
            self.layer_id = None;
            self.sync();
        }
    }
}

/// Estilo padrão de objetos novos: sem preenchimento, contorno preto 0,2 mm.
pub fn default_stroke() -> Stroke {
    Stroke::new(200.0, Color::cmyk(0.0, 0.0, 0.0, 1.0))
}
