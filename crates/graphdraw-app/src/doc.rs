//! Documento aberto: editor (com histórico), arquivo, seleção e caches.

use std::path::PathBuf;
use std::sync::Arc;

use graphdraw_core::ImportReport;
use graphdraw_core::edit::Editor;
use graphdraw_core::edit::nodes::NodeRef;
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
    /// Página em edição.
    pub page: usize,
    /// Seleção ordenada (GD-SEL-01); a seleção não entra no histórico (GD-UND-06).
    pub selection: Vec<String>,
    /// Nós selecionados na ferramenta Forma.
    pub nodes: Vec<NodeRef>,
    /// Camada ativa escolhida no painel (por ID, para sobreviver a desfazer).
    pub layer_id: Option<String>,
    /// Identifica o documento nesta execução (cópia de recuperação).
    pub key: String,
    /// Verificação prévia da página (revisão, página, problemas).
    preflight: Option<(u64, usize, Arc<Vec<graphdraw_core::preflight::Issue>>)>,
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
            selection: Vec::new(),
            nodes: Vec::new(),
            layer_id: None,
            key: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
                format!(
                    "d{}",
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                )
            },
            preflight: None,
            scene: None,
            images: ImageCache::default(),
        };
        s.sync();
        s
    }

    /// Problemas da verificação prévia da página atual (GD-PRE-01), refeitos
    /// só quando o documento ou a página mudam.
    pub fn preflight(&mut self) -> Arc<Vec<graphdraw_core::preflight::Issue>> {
        let rev = self.editor.revision();
        if let Some((r, p, v)) = &self.preflight
            && *r == rev
            && *p == self.page
        {
            return v.clone();
        }
        let v = Arc::new(graphdraw_core::preflight::check(
            &self.editor.doc,
            self.page,
        ));
        self.preflight = Some((rev, self.page, v.clone()));
        v
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

    /// Página bloqueada (GD-PGT-18): nada nela é selecionável ou editável.
    pub fn page_locked(&self) -> bool {
        self.page().locked
    }

    /// Mantém a página atual válida (após excluir ou desfazer páginas).
    /// Chamar uma vez por quadro.
    pub fn sync(&mut self) {
        let n = self.editor.doc.data.pages.len();
        if self.page >= n {
            self.page = n.saturating_sub(1);
            self.selection.clear();
            self.nodes.clear();
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

    /// Miniatura da página `index` com lado maior `max_px`.
    pub fn thumbnail(&self, index: usize, max_px: f64) -> eframe::egui::ColorImage {
        let page = &self.editor.doc.data.pages[index];
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

    /// Miniatura 1:1 de um objeto (painel de camadas): o objeto sozinho,
    /// centrado e ajustado num quadrado de `px` pixels com fundo transparente.
    /// Objetos ocultos também aparecem, para serem reconhecidos.
    pub fn object_thumbnail(
        &self,
        r: &graphdraw_core::edit::ObjRef,
        px: u32,
    ) -> Option<eframe::egui::ColorImage> {
        use graphdraw_core::edit::query;
        let page = &self.editor.doc.data.pages[r.loc.page];
        let mut o = self.editor.get(r)?.clone();
        o.transform = graphdraw_core::model::Transform::from_affine(
            self.editor.parent_affine(r) * o.transform.affine(),
        );
        o.visible = true;
        let b = query::bounds(&o, Affine::IDENTITY, true)?;
        let mut p = page.props_only();
        let mut layer = Layer::new("miniatura");
        layer.objects.push(o);
        p.layers.push(layer);
        let scene = build_scene(self.doc(), &p, SceneOptions { print: false }, &self.images);
        // Margem de 1 px; formas sem largura ou altura (linhas) ainda aparecem.
        let side = b.width().max(b.height()).max(1e-6);
        let s = (px as f64 - 2.0) / side;
        let c = b.center();
        let half = px as f64 / 2.0;
        // Mesma convenção da miniatura da página: y da página para cima.
        let view = Affine::new([s, 0.0, 0.0, -s, half - s * c.x, half + s * c.y]);
        let opts = RasterOptions {
            antialias: true,
            background: None,
        };
        let pm = raster::render(&scene, view, px, px, &opts)?;
        Some(eframe::egui::ColorImage::from_rgba_premultiplied(
            [px as usize, px as usize],
            pm.data(),
        ))
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

    /// Camada para um objeto novo. Uma página sem camadas ganha uma.
    pub fn layer_for_new_object(&mut self) -> Option<usize> {
        if let Some(l) = self.active_layer() {
            return Some(l);
        }
        if !self.page_locked() && self.page().layers.is_empty() {
            let layer = Layer::new(crate::i18n::tr("Camada 1", "Layer 1"));
            self.layer_id = Some(layer.id.clone());
            let page = self.page;
            self.editor
                .transaction(crate::i18n::tr("Criar camada", "Create layer"), |ed| {
                    ed.insert_layer(page, 0, layer)
                });
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
        if page < self.doc().data.pages.len() && page != self.page {
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
