//! Importar para a página atual (comando Importar, Ctrl+I): desenhos
//! (.gdr/.cdr/.cmx, objetos da primeira página num grupo) ou imagens.
//! O resultado entra centralizado na página, num passo do histórico.

use std::path::Path;
use std::sync::Arc;

use graphdraw_core::edit::ops;
use graphdraw_core::model::{GroupData, ImageData, Object, ObjectKind, Transform};
use graphdraw_core::render::scene::decode_image;
use kurbo::Affine;

use crate::doc::DocState;
use crate::i18n::tr;

const IMAGES: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "tif", "tiff", "webp"];

/// Importa o arquivo e devolve o ID do objeto criado.
pub fn into_page(doc: &mut DocState, path: &Path) -> Result<String, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut obj = if IMAGES.contains(&ext.as_str()) {
        image_object(doc, path, &ext)?
    } else {
        drawing_object(doc, path)?
    };
    // Centraliza na página.
    let (w, h) = doc.page_size();
    if let Some(b) = graphdraw_core::edit::query::bounds(&obj, Affine::IDENTITY, false) {
        let d = kurbo::Point::new(w / 2.0, h / 2.0) - b.center();
        obj.transform = Transform::from_affine(Affine::translate(d) * obj.transform.affine());
    }
    let layer = doc.layer_for_new_object().ok_or_else(|| {
        tr(
            "A página está bloqueada ou sem camada editável",
            "The page is locked or has no editable layer",
        )
        .to_string()
    })?;
    let page = doc.page;
    let name = format!(
        "{} {}",
        tr("Importar", "Import"),
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    );
    Ok(ops::add_object(&mut doc.editor, page, layer, obj, &name))
}

fn image_object(doc: &mut DocState, path: &Path, ext: &str) -> Result<Object, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let decoded = decode_image(Arc::new(bytes.clone())).ok_or_else(|| {
        tr(
            "Formato de imagem não reconhecido",
            "Unrecognized image format",
        )
        .to_string()
    })?;
    // Tamanho físico pela resolução do documento.
    let dpi = doc.doc().data.settings.resolution.max(1.0);
    let k = graphdraw_core::units::UM_PER_INCH / dpi;
    let src = doc.editor.doc.add_image(bytes, ext);
    let mut o = Object::new(ObjectKind::Image(ImageData {
        width: decoded.width as f64 * k,
        height: decoded.height as f64 * k,
        src,
        extra: Default::default(),
    }));
    o.name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    Ok(o)
}

fn drawing_object(doc: &mut DocState, path: &Path) -> Result<Object, String> {
    let (src, _) = graphdraw_core::open_path(path).map_err(|e| e.to_string())?;
    let first = src
        .data
        .pages
        .first()
        .ok_or_else(|| tr("O arquivo não tem páginas", "The file has no pages").to_string())?;
    let mut children: Vec<Object> = first
        .layers
        .iter()
        .flat_map(|l| l.objects.iter().cloned())
        .collect();
    if children.is_empty() {
        return Err(tr("A primeira página está vazia", "The first page is empty").into());
    }
    // IDs novos: importar o mesmo arquivo duas vezes não pode repetir IDs.
    for c in &mut children {
        ops::renew_ids(c);
    }
    // Imagens usadas pelo desenho vêm junto (endereçadas por hash).
    for (k, v) in &src.images {
        doc.editor
            .doc
            .images
            .entry(k.clone())
            .or_insert_with(|| v.clone());
    }
    let mut g = Object::new(ObjectKind::Group(GroupData {
        children,
        boolean: None,
        symmetry: None,
        effect: None,
        symbol: None,
        extra: Default::default(),
    }));
    g.name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    Ok(g)
}
