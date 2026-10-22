//! Copy and paste through the system clipboard, and external bitmap
//! editing.
//!
//! Copy keeps the selection on the internal clipboard and publishes it
//! for every program: the private payload (full fidelity, for GraphDraw
//! windows), SVG, the SVG as text and a picture. Paste takes the best
//! flavour the system clipboard offers (see [`crate::clip_offer`]); the
//! internal clipboard serves while the system one still holds our own
//! copy, or nothing. Content from outside is sanitised: SVG reads no
//! files or network, pictures are size-limited. Bitmaps > Edit Bitmap
//! hands a PNG to the system's image editor and Update from Link reads
//! it back.

use crate::app::App;
use crate::clip_offer::{Choice, Offer};
use crate::i18n::{tr, trf};
use graphdraw_core::{
    document::{ShapeKind, TextSpan},
    geometry::{Affine, Point},
    Command, ShapeId,
};

/// Largest picture side accepted from the clipboard, pixels.
pub(crate) const MAX_PICTURE_SIDE: u32 = 16_384;

/// Decode a picture from outside, refusing oversize ones.
fn decode_picture(bytes: &[u8]) -> Option<image::RgbaImage> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_PICTURE_SIDE);
    limits.max_image_height = Some(MAX_PICTURE_SIDE);
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().ok().map(|i| i.to_rgba8())
}

impl App {
    /// The selection as a standalone SVG document (page-sized viewport so
    /// positions survive a round trip).
    pub fn selection_svg(&self) -> Option<String> {
        let shapes = self.selected_shapes_whole();
        if shapes.is_empty() {
            return None;
        }
        let mut tmp = graphdraw_core::Document::new("clipboard", self.page_size());
        tmp.symbols = self.doc().symbols.clone();
        let layer = tmp.pages[0].layers[0].id;
        if let Ok(l) = tmp.layer_mut(layer) {
            l.shapes = shapes;
        }
        Some(graphdraw_io::svg::page_to_svg(&tmp, 0))
    }

    /// Copy to the internal clipboard and to the system clipboard: the
    /// private payload, SVG, the SVG as text and a picture, as Options >
    /// Workspace > Edit asks, leaving out the heaviest formats past the
    /// size limit.
    pub fn copy_with_system(&mut self) {
        self.copy();
        let shapes = self.selected_shapes_whole();
        if shapes.is_empty() {
            return;
        }
        let prefs = self.settings.clipboard.clone();
        let doc = self.doc();
        let native = crate::clip_native::encode(&shapes, &doc.object_styles, &doc.symbols)
            .map_err(|e| log::warn!("clipboard payload: {e}"))
            .ok();
        let mut p = crate::clip_system::Publication {
            native,
            svg: self.selection_svg(),
            svg_as_text: prefs.svg_as_text,
            png: if prefs.png {
                self.clipboard_png(prefs.png_max_side)
            } else {
                None
            },
        };
        fit_publication(&mut p, prefs.max_publish_bytes);
        // When publishing fails the counter still tells whether anything
        // was copied elsewhere since.
        let sequence = crate::clip_system::publish(&p).or_else(crate::clip_system::sequence);
        self.published = Some(Published {
            sequence,
            svg_hash: p.svg.as_deref().filter(|_| p.svg_as_text).map(text_hash),
        });
    }

    /// A copy that stays on the internal clipboard only (curve segments):
    /// it is the newest copy until the system clipboard changes.
    pub(crate) fn mark_internal_copy(&mut self) {
        self.published = Some(Published {
            sequence: crate::clip_system::sequence(),
            svg_hash: None,
        });
    }

    /// Run `f` and keep everything it recorded as one undo step.
    pub(crate) fn as_one_step(&mut self, label: &'static str, f: impl FnOnce(&mut App)) {
        let mark = self.engine.revision();
        f(self);
        self.engine.join_steps_since(mark, label);
    }

    /// The selection as a PNG at screen resolution, smaller when its
    /// longest side would pass `max_side` pixels.
    fn clipboard_png(&self, max_side: u32) -> Option<Vec<u8>> {
        let b = self.selection_bounds()?;
        let longest_mm = b.width().max(b.height());
        let mut dpi = 96.0;
        if longest_mm > 0.0 {
            dpi = f64::min(dpi, f64::from(max_side.max(1)) * 25.4 / longest_mm);
        }
        self.render_selection_png(dpi, true).map(|(png, ..)| png)
    }
}

/// What a copy published, so a paste knows its own publication.
#[derive(Debug, Clone, PartialEq)]
pub struct Published {
    /// The clipboard's change counter right after publishing.
    pub sequence: Option<u32>,
    /// The SVG published (systems without a change counter).
    pub svg_hash: Option<u64>,
}

pub(crate) fn text_hash(t: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// Leave formats out, the picture first, then the SVG text, until `p`
/// fits in `limit` bytes; the private payload goes last.
fn fit_publication(p: &mut crate::clip_system::Publication, limit: usize) {
    let size = |p: &crate::clip_system::Publication| {
        let svg = p.svg.as_ref().map_or(0, String::len);
        p.native.as_ref().map_or(0, Vec::len)
            + p.png.as_ref().map_or(0, Vec::len)
            + if p.svg_as_text { svg * 3 } else { svg }
    };
    if size(p) > limit {
        p.png = None;
    }
    if size(p) > limit {
        p.svg_as_text = false;
    }
    if size(p) > limit {
        p.svg = None;
    }
    if size(p) > limit {
        p.native = None;
    }
}

impl App {

    /// Paste text as artistic text at the page centre, one undo step.
    pub fn paste_as_text(&mut self, text: &str) -> bool {
        let c = self.page_rect().center();
        let span = TextSpan {
            bold: self.text_bold,
            italic: self.text_italic,
            ..TextSpan::new(text, self.text_font.clone(), self.text_size_pt)
        };
        let kind = ShapeKind::Text {
            spans: vec![span],
            origin: Point::new(c.x, c.y),
            frame: None,
            align: graphdraw_core::TextAlign::Left,
            para: Default::default(),
            on_path: None,
        };
        let Some(id) = self.new_shape(kind) else {
            return false;
        };
        for cmd in [
            Command::SetFill {
                shapes: vec![id],
                fill: graphdraw_core::Fill::Solid(graphdraw_core::Color::BLACK),
            },
            Command::SetStroke {
                shapes: vec![id],
                stroke: None,
            },
        ] {
            if let Err(e) = self.engine.amend(&cmd) {
                self.status = e.to_string();
            }
        }
        self.select(vec![id]);
        true
    }

    /// Paste SVG markup as objects. The markup comes from outside, so it
    /// is read sanitised.
    pub fn paste_as_svg(&mut self, svg: &str) -> bool {
        let ids = &mut graphdraw_core::id::IdSource::default();
        match graphdraw_io::svg_import::parse_untrusted(svg, ids) {
            Ok(imported) if !imported.shapes.is_empty() => {
                !self.paste_shapes(imported.shapes, &[]).is_empty()
            }
            Ok(_) => false,
            Err(e) => {
                self.status = trf("status.import_failed", &[("e", &e)]);
                false
            }
        }
    }

    /// Paste a picture as a bitmap object at the page centre.
    pub fn paste_as_bitmap(&mut self, img: image::RgbaImage) -> bool {
        match self.place_image(img, Some(tr("kind.bitmap_short"))) {
            Some(id) => {
                self.select(vec![id]);
                true
            }
            None => false,
        }
    }

    /// Ctrl+V: the best flavour on the clipboard, placed as Options >
    /// Workspace > Edit asks (one undo step with the paste).
    pub fn paste_any(&mut self) {
        self.as_one_step("Paste", |app| {
            let before = app.selection.clone();
            app.paste_best();
            let pasted = app.selection != before && !app.selection.is_empty();
            if pasted && app.settings.clipboard.paste_place == crate::prefs::PastePlace::ViewCenter
            {
                app.centre_selection_in_view();
            }
        });
    }

    /// Paste the internal clipboard while the system clipboard holds our
    /// own copy or nothing; otherwise the best flavour the system offers,
    /// falling back to the next when one cannot be read.
    pub(crate) fn paste_best(&mut self) {
        let offer = crate::clip_system::read();
        let candidates = crate::clip_offer::candidates(&offer);
        if self.clipboard.is_some() && (candidates.is_empty() || self.is_own_publication(&offer)) {
            self.paste();
            return;
        }
        if candidates.is_empty() {
            if crate::clip_offer::choose(&offer) == Choice::Unsupported {
                self.alert_clipboard_unsupported();
            }
            return;
        }
        if !self.paste_with(&offer, &candidates) {
            self.alert_clipboard_unsupported();
        }
    }

    /// Paste the first of `choices` that `offer` holds and that reads.
    pub(crate) fn paste_with(&mut self, offer: &Offer, choices: &[Choice]) -> bool {
        choices.iter().any(|c| self.paste_choice(offer, *c))
    }

    fn paste_choice(&mut self, offer: &Offer, choice: Choice) -> bool {
        match choice {
            Choice::Native => match offer.native.as_deref().map(crate::clip_native::decode) {
                Some(Ok(p)) => !self.paste_shapes(p.shapes, &p.styles).is_empty(),
                Some(Err(e)) => {
                    log::warn!("clipboard payload: {e}");
                    false
                }
                None => false,
            },
            Choice::Svg => offer.svg.as_deref().is_some_and(|s| self.paste_as_svg(s)),
            Choice::SvgText => offer.text.as_deref().is_some_and(|s| self.paste_as_svg(s)),
            Choice::Files => self.paste_files(&offer.files),
            Choice::Png => offer
                .png
                .as_deref()
                .and_then(decode_picture)
                .is_some_and(|img| self.paste_as_bitmap(img)),
            Choice::Bitmap => offer
                .image
                .clone()
                .filter(|i| i.width().max(i.height()) <= MAX_PICTURE_SIDE)
                .is_some_and(|img| self.paste_as_bitmap(img)),
            Choice::HtmlImage => offer
                .html
                .as_deref()
                .and_then(crate::clip_offer::html_image_data)
                .and_then(|b| decode_picture(&b))
                .is_some_and(|img| self.paste_as_bitmap(img)),
            Choice::Text => offer.text.as_deref().is_some_and(|t| self.paste_as_text(t)),
            Choice::Unsupported | Choice::Nothing => false,
        }
    }

    /// Files copied in the file manager, imported one by one (unsupported
    /// ones are named in one alert); all that came in end up selected.
    fn paste_files(&mut self, files: &[std::path::PathBuf]) -> bool {
        let mut all = Vec::new();
        for f in files {
            if self.import_path(f) {
                all.extend(self.selection.iter().copied());
            }
        }
        if all.is_empty() {
            return false;
        }
        self.select(all);
        true
    }

    /// Whether the system clipboard still holds what our last copy put
    /// there: by the change counter, else by the SVG text. With neither
    /// to compare, the copy made here is taken as the newest.
    fn is_own_publication(&self, offer: &Offer) -> bool {
        let Some(p) = &self.published else {
            return false;
        };
        match (p.sequence, crate::clip_system::sequence(), p.svg_hash) {
            (Some(ours), Some(now), _) => ours == now,
            (_, _, Some(hash)) => {
                let svg = offer.svg.as_deref().or(offer.text.as_deref());
                svg.map(text_hash) == Some(hash)
            }
            _ => true,
        }
    }

    fn alert_clipboard_unsupported(&mut self) {
        if matches!(self.dialog, crate::ui::dialogs::Dialog::None) {
            self.dialog = crate::ui::dialogs::Dialog::Alert(tr("alert.clipboard_unsupported"));
        }
    }

    /// Move the selection's centre to the centre of the view, inside the
    /// undo step that made it.
    pub(crate) fn centre_selection_in_view(&mut self) {
        let Some(b) = self.selection_bounds() else {
            return;
        };
        let d = self.view.visible_page_rect(self.canvas_rect).center() - b.center();
        if !(d.x.is_finite() && d.y.is_finite()) {
            return;
        }
        let cmd = Command::TransformShapes {
            shapes: self.selection.clone(),
            transform: Affine::translate(d),
        };
        if let Err(e) = self.engine.amend(&cmd) {
            self.status = e.to_string();
        }
    }

    // ----- external bitmap editing -------------------------------------------------

    fn bitmap_link(&self, id: ShapeId) -> Option<String> {
        self.doc()
            .find_shape(id)?
            .data
            .iter()
            .find(|(k, _)| k == "bitmap.link")
            .map(|(_, v)| v.clone())
    }

    pub fn selected_bitmap(&self) -> Option<ShapeId> {
        let id = *self.selection.first()?;
        let s = self.doc().find_shape(id)?;
        matches!(s.kind, ShapeKind::Bitmap { .. }).then_some(id)
    }

    pub fn selected_bitmap_is_linked(&self) -> bool {
        self.selected_bitmap()
            .and_then(|id| self.bitmap_link(id))
            .is_some()
    }

    /// The selected bitmap's PNG, its effects included.
    pub fn selected_bitmap_png(&self) -> Option<Vec<u8>> {
        let id = self.selected_bitmap()?;
        match &self.doc().find_shape(id)?.kind {
            ShapeKind::Bitmap { png, .. } => Some(png.clone()),
            _ => None,
        }
    }

    /// Save Bitmap As (the bitmap's context menu): its pixels, effects
    /// included, to a PNG file.
    pub fn save_bitmap_as(&mut self) {
        let Some(png) = self.selected_bitmap_png() else {
            return;
        };
        let name = format!("{}.png", self.document_name());
        crate::files::Dialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name(name)
            .save_file(self, move |app, path| {
                app.status = match crate::files::write(&path, &png) {
                    Ok(()) => crate::i18n::trf(
                        "status.exported",
                        &[("path", &path.display().to_string())],
                    ),
                    Err(e) => crate::i18n::trf("status.export_failed", &[("e", &e.to_string())]),
                };
            });
    }

    /// Bitmaps > Edit Bitmap: write the pixels to a PNG next to the system's
    /// temporary files, link the object to it and open the system editor.
    pub fn edit_bitmap_externally(&mut self) {
        if crate::files::WEB {
            // A browser cannot hand a file to another program.
            self.status = tr("status.not_in_browser");
            return;
        }
        let Some(id) = self.selected_bitmap() else {
            return;
        };
        let Some(s) = self.doc().find_shape(id).cloned() else {
            return;
        };
        let ShapeKind::Bitmap { png, .. } = &s.kind else {
            return;
        };
        let dir = std::env::temp_dir().join("graphdraw-edit");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = e.to_string();
            return;
        }
        let path = dir.join(format!("bitmap-{}.png", id.raw()));
        if let Err(e) = std::fs::write(&path, png) {
            self.status = e.to_string();
            return;
        }
        let mut data = s.data.clone();
        data.retain(|(k, _)| k != "bitmap.link");
        data.push(("bitmap.link".into(), path.display().to_string()));
        self.run(Command::SetObjectData { shape: id, data });
        match crate::export::open_with_system(&path) {
            Ok(()) => self.status = tr("status.bitmap_sent_to_editor"),
            Err(e) => self.status = e,
        }
    }

    /// Bitmaps > Update from Link: re-read the linked file into the object.
    pub fn update_bitmap_from_link(&mut self) {
        let Some(id) = self.selected_bitmap() else {
            return;
        };
        let Some(path) = self.bitmap_link(id) else {
            return;
        };
        let img = match crate::files::open_image(&path) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                self.status = trf(
                    "status.could_not_read",
                    &[("p", &path), ("e", &e.to_string())],
                );
                return;
            }
        };
        let Some(s) = self.doc().find_shape(id).cloned() else {
            return;
        };
        let ShapeKind::Bitmap { rect, .. } = s.kind else {
            return;
        };
        let (w, h) = img.dimensions();
        let mut png = Vec::new();
        if image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .is_err()
        {
            return;
        }
        self.run(Command::SetShapeKind {
            shape: id,
            kind: ShapeKind::Bitmap {
                rect,
                width_px: w,
                height_px: h,
                png,
                fx: None,
            },
        });
        self.status = tr("status.bitmap_updated");
    }

    /// Bitmaps > Break Link: forget the file; the pixels stay embedded.
    pub fn break_bitmap_link(&mut self) {
        let Some(id) = self.selected_bitmap() else {
            return;
        };
        let Some(s) = self.doc().find_shape(id).cloned() else {
            return;
        };
        let mut data = s.data;
        data.retain(|(k, _)| k != "bitmap.link");
        self.run(Command::SetObjectData { shape: id, data });
    }
}
