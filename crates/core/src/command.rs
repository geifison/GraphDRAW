//! Commands: every mutation of a document is one of these. The UI, the CLI
//! and the control channel all build commands and hand them to the
//! [`crate::Engine`], which applies them and records history.

use crate::document::{
    ColorStyle, Guide, Layer, MasterScope, Metadata, ObjectStyle, Page, Shadow, Shape, ShapeKind,
    Symbol,
};
use crate::geometry::{Affine, BezPath, PathEl, Size};
use crate::id::{LayerId, PageId, ShapeId};
use crate::style::{Fill, Stroke};
use crate::Color;
use crate::{Document, Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    // Document
    SetTitle {
        title: String,
    },

    // Pages
    AddPage {
        name: Option<String>,
        size: Size,
    },
    DeletePage {
        page: PageId,
    },
    ResizePage {
        page: PageId,
        size: Size,
    },

    // Layers
    AddLayer {
        page: PageId,
        name: String,
    },
    AddMasterLayer {
        name: String,
        scope: MasterScope,
    },
    SetLayerScope {
        layer: LayerId,
        scope: MasterScope,
    },
    SetLayerPrintable {
        layer: LayerId,
        printable: bool,
    },
    SetPageBackground {
        page: PageId,
        background: Option<Fill>,
    },
    SetMetadata {
        metadata: Metadata,
    },
    /// The CMYK profile the drawing carries (`None`: the default one).
    SetCmykProfile {
        profile: Option<crate::document::DocumentProfile>,
    },
    /// The node types chosen for curve `shape` (node element index to
    /// type), replacing the ones it had.
    SetNodeTypes {
        shape: ShapeId,
        types: std::collections::BTreeMap<usize, crate::nodes::NodeType>,
    },
    /// Store `shapes` as selection group `number` (0 to 9); no shapes
    /// removes the group.
    SetSelectionGroup {
        number: u8,
        shapes: Vec<ShapeId>,
    },
    /// Replace the Time Tracker records. This never becomes an undo step
    /// and does not mark the drawing changed (see [`Command::outside_history`]).
    SetTimeTracker {
        tracker: crate::document::TimeTracker,
    },
    /// Replace the review comments. Comments on pages that are gone are
    /// dropped; ids are kept, and repeated or missing (0) ones get a free
    /// number.
    SetComments {
        comments: Vec<crate::document::Comment>,
    },
    /// Replace the object styles. Styles without an id get a free one;
    /// objects linked to a style take its parts again, and links to a
    /// style that is gone are dropped.
    SetObjectStyles {
        styles: Vec<ObjectStyle>,
    },
    /// Link objects to an object style and give them its parts; `None`
    /// breaks their links and leaves their properties as they are.
    LinkObjectStyle {
        shapes: Vec<ShapeId>,
        style: Option<u32>,
    },
    /// Replace the fill and outline new objects get.
    SetObjectDefaults {
        defaults: crate::document::ObjectDefaults,
    },
    SetColorStyles {
        styles: Vec<ColorStyle>,
    },
    SetDocumentPalette {
        colors: Vec<Color>,
    },
    /// The drawing's spot inks, replacing the ones it had.
    SetInks {
        inks: Vec<crate::document::Ink>,
    },
    /// Draw clip frame `shape` as an opacity mask (`None`: a hard clip).
    SetOpacityMask {
        shape: ShapeId,
        mask: Option<crate::document::OpacityMask>,
    },
    /// Bitmap effects (and their picture) on a vector object; `None`
    /// removes them.
    SetRasterFx {
        shape: ShapeId,
        fx: Option<Box<crate::document::RasterFx>>,
    },
    AddSymbol {
        symbol: Symbol,
    },
    /// Move a page to another index (Page Sorter drag).
    MovePage {
        page: PageId,
        to: usize,
    },
    DeleteLayer {
        layer: LayerId,
    },
    SetLayerVisible {
        layer: LayerId,
        visible: bool,
    },
    SetLayerLocked {
        layer: LayerId,
        locked: bool,
    },

    // Shapes
    /// Insert a shape at the top of a layer. The shape's id must come from
    /// `Document::ids_mut().shape()` so it stays unique.
    AddShape {
        layer: LayerId,
        shape: Shape,
    },
    DeleteShapes {
        shapes: Vec<ShapeId>,
    },
    /// Pre-multiply the given transform onto each shape's transform
    /// (i.e. apply it in page space).
    TransformShapes {
        shapes: Vec<ShapeId>,
        transform: Affine,
    },
    SetFill {
        shapes: Vec<ShapeId>,
        fill: Fill,
    },
    SetStroke {
        shapes: Vec<ShapeId>,
        stroke: Option<Stroke>,
    },
    SetShapeKind {
        shape: ShapeId,
        kind: ShapeKind,
    },
    SetShapeName {
        shape: ShapeId,
        name: Option<String>,
    },
    /// A QR code object's settings (its children are set with
    /// [`Command::SetShapeKind`] in the same step).
    SetQrCode {
        shape: ShapeId,
        qr: Option<Box<crate::document::QrCode>>,
    },
    /// Move a shape to another layer and position (z-order).
    Reorder {
        shape: ShapeId,
        layer: LayerId,
        index: usize,
    },
    Group {
        shapes: Vec<ShapeId>,
    },
    Ungroup {
        group: ShapeId,
    },
    SetOpacity {
        shapes: Vec<ShapeId>,
        opacity: f64,
    },
    SetLocked {
        shapes: Vec<ShapeId>,
        locked: bool,
    },
    /// Hide or show objects (Object > Hide); hidden objects are not drawn
    /// and cannot be selected on the canvas, only in the Objects docker.
    SetVisible {
        shapes: Vec<ShapeId>,
        visible: bool,
    },
    /// Replace the live effect stack of a shape.
    SetEffects {
        shape: ShapeId,
        effects: Vec<crate::live::Effect>,
    },
    /// The whole effect stack of `shape`: the effects drawn and the ones
    /// switched off (see `Shape::effect_stack`).
    SetEffectStack {
        shape: ShapeId,
        effects: Vec<crate::live::Effect>,
        parked: Vec<crate::document::ParkedEffect>,
    },
    /// The further fills and outlines of `shapes`, replacing theirs.
    SetExtraPaint {
        shapes: Vec<ShapeId>,
        fills: Vec<Fill>,
        strokes: Vec<Stroke>,
    },
    SetOverprint {
        shapes: Vec<ShapeId>,
        fill: Option<bool>,
        outline: Option<bool>,
    },
    /// The uniform transparency of the fill alone (1.0 opaque, 0.0
    /// invisible); the outline and the object's own opacity stay.
    SetFillOpacity {
        shapes: Vec<ShapeId>,
        opacity: f64,
    },
    SetLink {
        shapes: Vec<ShapeId>,
        link: Option<String>,
    },
    /// Mark `shapes` as construction lines (shown, not printed or exported).
    SetConstruction {
        shapes: Vec<ShapeId>,
        construction: bool,
    },
    /// The ghost path a straightened text keeps (see `text_path`).
    SetTextPathMemory {
        shape: ShapeId,
        memory: Option<Box<crate::document::TextOnPath>>,
    },
    SetWrapText {
        shapes: Vec<ShapeId>,
        wrap: bool,
    },
    /// Turn wrapping on for `shapes` with this wrap style and offset.
    SetWrapStyle {
        shapes: Vec<ShapeId>,
        style: crate::text_flow::TextWrap,
    },
    SetObjectData {
        shape: ShapeId,
        data: Vec<(String, String)>,
    },
    /// Move `shapes` directly in front of (or behind) `reference` in its layer.
    OrderRelative {
        shapes: Vec<ShapeId>,
        reference: ShapeId,
        in_front: bool,
    },
    /// Reverse the stacking order of the given shapes among themselves.
    ReverseOrder {
        shapes: Vec<ShapeId>,
    },
    /// Merge several shapes into one curve (even-odd fill), like Ctrl+L.
    Combine {
        shapes: Vec<ShapeId>,
    },
    /// Split a curve into one object per subpath, like Ctrl+K.
    BreakApart {
        shape: ShapeId,
    },
    SetShadow {
        shapes: Vec<ShapeId>,
        shadow: Option<Shadow>,
    },
    /// Place objects inside a frame object (a clip frame). The frame keeps its
    /// id; the contents keep their page positions.
    PlaceInside {
        contents: Vec<ShapeId>,
        frame: ShapeId,
    },
    /// Take the contents out of a clip frame, leaving the frame as a plain object.
    ExtractContents {
        clip: ShapeId,
    },
    RenamePage {
        page: PageId,
        name: String,
    },
    /// Copy a page (layers, shapes with fresh ids, guides) right after it.
    DuplicatePage {
        page: PageId,
    },
    RenameLayer {
        layer: LayerId,
        name: String,
    },
    /// Move a layer to another index within its page, or a master layer
    /// within the master layers (z-order).
    ReorderLayer {
        layer: LayerId,
        index: usize,
    },
    AddGuide {
        page: PageId,
        guide: Guide,
    },
    MoveGuide {
        page: PageId,
        index: usize,
        guide: Guide,
    },
    DeleteGuide {
        page: PageId,
        index: usize,
    },
    /// Replace all guidelines of a page (several deleted, locked or
    /// restyled at once, presets applied).
    SetGuides {
        page: PageId,
        guides: Vec<Guide>,
    },
}

/// How many nodes a curve has (0 for other kinds).
fn node_count(kind: &ShapeKind) -> usize {
    match kind {
        ShapeKind::Path { path, .. } => crate::nodes::nodes(path).len(),
        _ => 0,
    }
}

impl Command {
    /// True for commands that change state kept outside the undo history
    /// (the Time Tracker's records): the engine applies them without an
    /// undo step, and undo and redo leave that state alone.
    pub fn outside_history(&self) -> bool {
        matches!(self, Command::SetTimeTracker { .. })
    }

    /// Short label for the History panel and the Edit menu ("Undo Move").
    pub fn label(&self) -> &'static str {
        match self {
            Command::SetTitle { .. } => "Rename Document",
            Command::AddPage { .. } => "Insert Page",
            Command::DeletePage { .. } => "Delete Page",
            Command::ResizePage { .. } => "Resize Page",
            Command::AddLayer { .. } => "New Layer",
            Command::AddMasterLayer { .. } => "New Master Layer",
            Command::SetLayerScope { .. } => "Master Layer Scope",
            Command::SetLayerPrintable { .. } => "Layer Printable",
            Command::SetPageBackground { .. } => "Page Background",
            Command::SetMetadata { .. } => "Document Properties",
            Command::SetCmykProfile { .. } => "Color Management",
            Command::SetTimeTracker { .. } => "Time Tracker",
            Command::SetComments { .. } => "Comments",
            Command::SetSelectionGroup { .. } => "Selection Group",
            Command::SetNodeTypes { .. } => "Node Type",
            Command::SetObjectStyles { .. } => "Object Styles",
            Command::LinkObjectStyle { style: Some(_), .. } => "Apply Style",
            Command::LinkObjectStyle { style: None, .. } => "Break Link to Style",
            Command::SetObjectDefaults { .. } => "Default Properties",
            Command::SetColorStyles { .. } => "Color Styles",
            Command::SetDocumentPalette { .. } => "Document Palette",
            Command::SetInks { .. } => "Spot Inks",
            Command::SetOpacityMask { .. } => "Opacity Mask",
            Command::SetRasterFx { .. } => "Effect",
            Command::AddSymbol { .. } => "New Symbol",
            Command::MovePage { .. } => "Move Page",
            Command::DeleteLayer { .. } => "Delete Layer",
            Command::SetLayerVisible { .. } => "Layer Visibility",
            Command::SetLayerLocked { .. } => "Lock Layer",
            Command::AddShape { .. } => "Create Object",
            Command::DeleteShapes { .. } => "Delete",
            Command::TransformShapes { .. } => "Transform",
            Command::SetFill { .. } => "Fill",
            Command::SetStroke { .. } => "Outline",
            Command::SetShapeKind { .. } => "Edit Object",
            Command::SetShapeName { .. } => "Rename Object",
            Command::SetQrCode { .. } => "QR Code",
            Command::Reorder { .. } => "Order",
            Command::Group { .. } => "Group",
            Command::Ungroup { .. } => "Ungroup",
            Command::SetOpacity { .. } => "Transparency",
            Command::SetLocked { locked: true, .. } => "Lock Object",
            Command::SetLocked { .. } => "Unlock Object",
            Command::SetVisible { visible: false, .. } => "Hide Object",
            Command::SetVisible { .. } => "Show Object",
            Command::SetEffects { effects, .. } if effects.is_empty() => "Clear Effects",
            Command::SetEffects { .. } => "Effect",
            Command::SetEffectStack { .. } => "Effects",
            Command::SetExtraPaint { .. } => "Fills and Outlines",
            Command::SetOverprint { .. } => "Overprint",
            Command::SetFillOpacity { .. } => "Fill Transparency",
            Command::SetLink { .. } => "Hyperlink",
            Command::SetConstruction { .. } => "Construction Line",
            Command::SetTextPathMemory { .. } => "Straighten Text",
            Command::SetWrapText { .. } => "Wrap Paragraph Text",
            Command::SetWrapStyle { .. } => "Wrap Style",
            Command::SetObjectData { .. } => "Object Data",
            Command::OrderRelative { in_front: true, .. } => "In Front Of",
            Command::OrderRelative { .. } => "Behind",
            Command::ReverseOrder { .. } => "Reverse Order",
            Command::Combine { .. } => "Combine",
            Command::BreakApart { .. } => "Break Apart",
            Command::PlaceInside { .. } => "Place Inside Frame",
            Command::ExtractContents { .. } => "Extract Contents",
            Command::RenamePage { .. } => "Rename Page",
            Command::DuplicatePage { .. } => "Duplicate Page",
            Command::RenameLayer { .. } => "Rename Layer",
            Command::ReorderLayer { .. } => "Reorder Layer",
            Command::SetShadow {
                shadow: Some(_), ..
            } => "Drop Shadow",
            Command::SetShadow { .. } => "Clear Drop Shadow",
            Command::AddGuide { .. } => "Add Guideline",
            Command::MoveGuide { .. } => "Move Guideline",
            Command::DeleteGuide { .. } => "Delete Guideline",
            Command::SetGuides { .. } => "Guidelines",
        }
    }

    /// Apply the command to a document. Fails without touching the document
    /// when it refers to objects that do not exist.
    pub fn apply(&self, doc: &mut Document) -> Result<()> {
        match self {
            Command::SetTitle { title } => doc.title = title.clone(),

            Command::AddPage { name, size } => {
                let n = doc.pages.len() + 1;
                let page_id = doc.ids_mut().page();
                let layer_id = doc.ids_mut().layer();
                doc.pages.push(Page {
                    id: page_id,
                    name: name.clone().unwrap_or_else(|| format!("Page {n}")),
                    size: *size,
                    layers: vec![Layer::new(layer_id, "Layer 1")],
                    guides: Vec::new(),
                    background: None,
                });
            }
            Command::DeletePage { page } => {
                if doc.pages.len() <= 1 {
                    return Err(Error::LastPage);
                }
                let i = doc
                    .pages
                    .iter()
                    .position(|p| p.id == *page)
                    .ok_or(Error::PageNotFound(*page))?;
                doc.pages.remove(i);
                doc.comments.retain(|c| c.page != *page);
            }
            Command::ResizePage { page, size } => doc.page_mut(*page)?.size = *size,

            Command::AddLayer { page, name } => {
                let id = doc.ids_mut().layer();
                doc.page_mut(*page)?
                    .layers
                    .push(Layer::new(id, name.clone()));
            }
            Command::AddMasterLayer { name, scope } => {
                let id = doc.ids_mut().layer();
                doc.master.push(Layer::master(id, name.clone(), *scope));
            }
            Command::SetLayerScope { layer, scope } => {
                doc.layer_mut(*layer)?.scope = *scope;
            }
            Command::SetLayerPrintable { layer, printable } => {
                doc.layer_mut(*layer)?.printable = *printable;
            }
            Command::SetPageBackground { page, background } => {
                doc.page_mut(*page)?.background = background.clone();
            }
            Command::SetMetadata { metadata } => {
                doc.metadata = metadata.clone();
            }
            Command::SetCmykProfile { profile } => {
                doc.cmyk_profile = profile.clone();
            }
            Command::SetSelectionGroup { number, shapes } => {
                if *number > 9 {
                    return Err(Error::SelectionGroup(*number));
                }
                if shapes.is_empty() {
                    doc.selection_groups.remove(number);
                } else {
                    doc.selection_groups.insert(*number, shapes.clone());
                }
            }
            Command::SetTimeTracker { tracker } => {
                doc.time_tracker = tracker.clone();
            }
            Command::SetComments { comments } => {
                let pages: Vec<PageId> = doc.pages.iter().map(|p| p.id).collect();
                let mut kept: Vec<crate::document::Comment> = comments
                    .iter()
                    .filter(|c| pages.contains(&c.page))
                    .cloned()
                    .collect();
                let mut next = kept.iter().map(|c| c.id).max().unwrap_or(0);
                let mut seen = std::collections::HashSet::new();
                for c in kept.iter_mut() {
                    if c.id == 0 || !seen.insert(c.id) {
                        next += 1;
                        c.id = next;
                        seen.insert(next);
                    }
                }
                doc.comments = kept;
            }
            Command::SetObjectStyles { styles } => {
                let mut styles = styles.clone();
                let mut next = styles.iter().map(|s| s.id).max().unwrap_or(0);
                for st in styles.iter_mut().filter(|s| s.id == 0) {
                    next += 1;
                    st.id = next;
                }
                doc.object_styles = styles;
                restyle(doc);
            }
            Command::LinkObjectStyle { shapes, style } => {
                let found = match style {
                    Some(id) => Some(
                        doc.object_styles
                            .iter()
                            .find(|s| s.id == *id)
                            .cloned()
                            .ok_or(Error::StyleNotFound(*id))?,
                    ),
                    None => None,
                };
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    let shape = any_mut(doc, *id)?;
                    shape.style = found.as_ref().map(|s| s.id);
                    if let Some(st) = &found {
                        st.apply_to(shape);
                    }
                }
            }
            Command::SetObjectDefaults { defaults } => {
                doc.defaults = defaults.clone();
            }
            Command::SetColorStyles { styles } => {
                doc.color_styles = styles.clone();
            }
            Command::SetDocumentPalette { colors } => {
                doc.palette = colors.clone();
            }
            Command::SetInks { inks } => {
                doc.inks = inks.clone();
            }
            Command::SetOpacityMask { shape, mask } => {
                let s = doc
                    .find_shape_mut(*shape)
                    .ok_or(Error::ShapeNotFound(*shape))?;
                if !matches!(s.kind, ShapeKind::ClipFrame { .. }) {
                    return Err(Error::NotClipFrame(*shape));
                }
                s.opacity_mask = *mask;
            }
            Command::SetRasterFx { shape, fx } => {
                let s = doc
                    .find_shape_mut(*shape)
                    .ok_or(Error::ShapeNotFound(*shape))?;
                s.raster_fx = fx.clone();
            }
            Command::AddSymbol { symbol } => {
                doc.symbols.push(symbol.clone());
            }
            Command::MovePage { page, to } => {
                let from = doc
                    .pages
                    .iter()
                    .position(|p| p.id == *page)
                    .ok_or(Error::PageNotFound(*page))?;
                let p = doc.pages.remove(from);
                let to = (*to).min(doc.pages.len());
                doc.pages.insert(to, p);
            }
            Command::DeleteLayer { layer } => {
                let (layers, master) = layer_list_mut(doc, *layer)?;
                if !master && layers.len() <= 1 {
                    return Err(Error::LastLayer);
                }
                layers.retain(|l| l.id != *layer);
            }
            Command::SetLayerVisible { layer, visible } => {
                doc.layer_mut(*layer)?.visible = *visible
            }
            Command::SetLayerLocked { layer, locked } => doc.layer_mut(*layer)?.locked = *locked,

            Command::AddShape { layer, shape } => {
                // Ids minted on a copy of the drawing (a script's) stay
                // ahead of the ones this drawing gives out next.
                fn highest(s: &Shape) -> u64 {
                    let inner = match &s.kind {
                        ShapeKind::Group { children } => {
                            children.iter().map(highest).max().unwrap_or(0)
                        }
                        ShapeKind::ClipFrame { frame, contents } => contents
                            .iter()
                            .map(highest)
                            .max()
                            .unwrap_or(0)
                            .max(highest(frame)),
                        _ => 0,
                    };
                    s.id.raw().max(inner)
                }
                let top = highest(shape);
                doc.layer_mut(*layer)?.shapes.push(shape.clone());
                doc.ids_mut().reserve(top);
            }
            Command::DeleteShapes { shapes } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for layer in doc.all_layers_mut() {
                    remove_shapes(&mut layer.shapes, shapes);
                }
            }
            Command::TransformShapes { shapes, transform } => {
                // The transform is in page space; an object inside a group
                // or a clip frame takes it in its parent's space.
                let mut local = Vec::with_capacity(shapes.len());
                for id in shapes {
                    any(doc, *id)?;
                    let t = if doc.shape(*id).is_ok() {
                        *transform
                    } else {
                        let parent = doc.parent_transform(*id).unwrap_or(Affine::IDENTITY);
                        let det = parent.determinant();
                        if !det.is_finite() || det.abs() < 1e-12 {
                            return Err(Error::ShapeNotFound(*id));
                        }
                        parent.inverse() * *transform * parent
                    };
                    local.push((*id, t));
                }
                for (id, t) in local {
                    let s = any_mut(doc, id)?;
                    s.transform = t * s.transform;
                }
            }
            Command::SetFill { shapes, fill } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    // A group passes the fill on to everything in it.
                    for_group_deep(any_mut(doc, *id)?, &mut |s| s.fill = fill.clone());
                }
            }
            Command::SetStroke { shapes, stroke } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    for_group_deep(any_mut(doc, *id)?, &mut |s| s.stroke = stroke.clone());
                }
            }
            Command::SetShapeKind { shape, kind } => {
                let target = any_mut(doc, *shape)?;
                // A clip frame given any other kind (a text style, a
                // conversion to curves) changes its frame object; the
                // contents stay inside.
                let into_frame = !matches!(kind, ShapeKind::ClipFrame { .. })
                    && matches!(target.kind, ShapeKind::ClipFrame { .. });
                let s: &mut Shape = if into_frame {
                    match &mut target.kind {
                        ShapeKind::ClipFrame { frame, .. } => frame,
                        _ => return Err(Error::NotClipFrame(*shape)),
                    }
                } else {
                    target
                };
                // Chosen node types follow node indices: a curve that
                // gains or loses nodes drops them.
                if !s.node_types.is_empty() && node_count(&s.kind) != node_count(kind) {
                    s.node_types.clear();
                }
                s.kind = kind.clone();
            }
            Command::SetNodeTypes { shape, types } => {
                any_mut(doc, *shape)?.node_types = types.clone()
            }
            Command::SetShapeName { shape, name } => any_mut(doc, *shape)?.name = name.clone(),
            Command::SetQrCode { shape, qr } => any_mut(doc, *shape)?.qr = qr.clone(),

            Command::Reorder {
                shape,
                layer,
                index,
            } => {
                let (from_layer, from_idx) = doc.locate(*shape)?;
                doc.layer(*layer)?;
                let s = doc.layer_mut(from_layer)?.shapes.remove(from_idx);
                let target = &mut doc.layer_mut(*layer)?.shapes;
                let index = (*index).min(target.len());
                target.insert(index, s);
            }

            Command::Group { shapes } => {
                if shapes.is_empty() {
                    return Ok(());
                }
                // The group lives in the layer of the topmost member, at the
                // position of the lowest member.
                let mut located: Vec<(ShapeId, LayerId, usize)> = shapes
                    .iter()
                    .map(|id| doc.locate(*id).map(|(l, i)| (*id, l, i)))
                    .collect::<Result<_>>()?;
                // Bottom to top: by layer (in drawing order), then by place
                // in the layer, so members from several layers keep the
                // order they were drawn in.
                let layer_rank = |l: LayerId| {
                    doc.pages
                        .iter()
                        .flat_map(|p| p.layers.iter())
                        .chain(doc.master.iter())
                        .position(|x| x.id == l)
                        .unwrap_or(usize::MAX)
                };
                located.sort_by_key(|(_, l, i)| (layer_rank(*l), *i));
                let layer_id = located[0].1;
                let insert_at = located[0].2;
                let mut children = Vec::with_capacity(shapes.len());
                for (id, l, _) in &located {
                    let layer = doc.layer_mut(*l)?;
                    let i = layer
                        .shapes
                        .iter()
                        .position(|s| s.id == *id)
                        .ok_or(Error::ShapeNotFound(*id))?;
                    children.push(layer.shapes.remove(i));
                }
                let gid = doc.ids_mut().shape();
                let mut group = Shape::new(gid, ShapeKind::Group { children });
                group.stroke = None;
                let target = &mut doc.layer_mut(layer_id)?.shapes;
                target.insert(insert_at.min(target.len()), group);
            }
            Command::SetOpacity { shapes, opacity } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.opacity = opacity.clamp(0.0, 1.0);
                }
            }
            Command::SetLocked { shapes, locked } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.locked = *locked;
                }
            }
            Command::SetVisible { shapes, visible } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.visible = *visible;
                }
            }
            Command::SetEffects { shape, effects } => {
                any_mut(doc, *shape)?.effects = effects.clone();
            }
            Command::SetEffectStack {
                shape,
                effects,
                parked,
            } => {
                let s = any_mut(doc, *shape)?;
                s.effects = effects.clone();
                s.parked_effects = parked.clone();
            }
            Command::SetExtraPaint {
                shapes,
                fills,
                strokes,
            } => {
                for id in shapes {
                    let s = any_mut(doc, *id)?;
                    s.extra_fills = fills.clone();
                    s.extra_strokes = strokes.clone();
                }
            }
            Command::SetOverprint {
                shapes,
                fill,
                outline,
            } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    let s = any_mut(doc, *id)?;
                    if let Some(f) = fill {
                        s.overprint_fill = *f;
                    }
                    if let Some(o) = outline {
                        s.overprint_outline = *o;
                    }
                }
            }
            Command::SetConstruction {
                shapes,
                construction,
            } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.construction = *construction;
                }
            }
            Command::SetTextPathMemory { shape, memory } => {
                any_mut(doc, *shape)?.text_path_memory = memory.clone();
            }
            Command::SetFillOpacity { shapes, opacity } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                // NaN (from a script) reads as opaque.
                let o = if opacity.is_nan() {
                    1.0
                } else {
                    opacity.clamp(0.0, 1.0)
                };
                for id in shapes {
                    // A group passes it on to everything in it, as the fill.
                    for_group_deep(any_mut(doc, *id)?, &mut |s| s.fill_opacity = o);
                }
            }
            Command::SetWrapText { shapes, wrap } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.wrap_text = *wrap;
                }
            }
            Command::SetWrapStyle { shapes, style } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                let mut style = *style;
                if !style.offset.is_finite() {
                    style.offset = 0.0;
                }
                style.offset = style.offset.clamp(0.0, 1000.0);
                for id in shapes {
                    let s = any_mut(doc, *id)?;
                    s.wrap_text = true;
                    s.wrap_style = style;
                }
            }
            Command::SetLink { shapes, link } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.link = link.clone();
                }
            }
            Command::SetObjectData { shape, data } => {
                any_mut(doc, *shape)?.data = data.clone();
            }
            Command::OrderRelative {
                shapes,
                reference,
                in_front,
            } => {
                let (layer, _) = doc.locate(*reference)?;
                let mut moved = Vec::new();
                for id in shapes {
                    if *id == *reference {
                        continue;
                    }
                    let (l, i) = doc.locate(*id)?;
                    if l != layer {
                        continue;
                    }
                    moved.push(doc.layer_mut(l)?.shapes.remove(i));
                }
                let (_, ri) = doc.locate(*reference)?;
                let at = if *in_front { ri + 1 } else { ri };
                let target = doc.layer_mut(layer)?;
                for (k, s) in moved.into_iter().enumerate() {
                    target.shapes.insert(at + k, s);
                }
            }
            Command::ReverseOrder { shapes } => {
                let mut slots: Vec<(LayerId, usize)> = Vec::new();
                for id in shapes {
                    slots.push(doc.locate(*id)?);
                }
                // Per layer, swap the shapes at the chosen indices in reverse.
                let mut by_layer: std::collections::BTreeMap<LayerId, Vec<usize>> =
                    Default::default();
                for (l, i) in slots {
                    by_layer.entry(l).or_default().push(i);
                }
                for (l, mut idx) in by_layer {
                    idx.sort_unstable();
                    let layer = doc.layer_mut(l)?;
                    let n = idx.len();
                    for k in 0..n / 2 {
                        layer.shapes.swap(idx[k], idx[n - 1 - k]);
                    }
                }
            }
            Command::Combine { shapes } => {
                if shapes.len() < 2 {
                    return Ok(());
                }
                let mut located: Vec<(ShapeId, LayerId, usize)> = shapes
                    .iter()
                    .map(|id| doc.locate(*id).map(|(l, i)| (*id, l, i)))
                    .collect::<Result<_>>()?;
                located.sort_by_key(|(_, _, i)| *i);
                // Result takes the attributes of the topmost shape.
                let top_id = located
                    .last()
                    .map(|(id, _, _)| *id)
                    .ok_or(Error::ShapeNotFound(shapes[0]))?;
                let top = doc.shape(top_id)?.1.clone();
                let layer_id = located[0].1;
                let insert_at = located[0].2;
                let mut path = BezPath::new();
                for (id, _, _) in &located {
                    let s = doc.shape(*id)?.1;
                    path.extend(s.page_path());
                }
                for (id, l, _) in &located {
                    let layer = doc.layer_mut(*l)?;
                    if let Some(i) = layer.shapes.iter().position(|s| s.id == *id) {
                        layer.shapes.remove(i);
                    }
                }
                let mut combined = Shape::new(top_id, ShapeKind::Path { path, closed: true });
                combined.fill = top.fill;
                combined.stroke = top.stroke;
                combined.opacity = top.opacity;
                let target = &mut doc.layer_mut(layer_id)?.shapes;
                target.insert(insert_at.min(target.len()), combined);
            }
            Command::BreakApart { shape } => {
                let (layer_id, idx) = doc.locate(*shape)?;
                let s = doc.shape(*shape)?.1.clone();
                let ShapeKind::Path { path, closed } = &s.kind else {
                    return Ok(());
                };
                let mut parts: Vec<BezPath> = Vec::new();
                let mut cur = BezPath::new();
                for el in path.elements() {
                    if let PathEl::MoveTo(_) = el {
                        if !cur.elements().is_empty() {
                            parts.push(std::mem::take(&mut cur));
                        }
                    }
                    cur.push(*el);
                }
                if !cur.elements().is_empty() {
                    parts.push(cur);
                }
                if parts.len() < 2 {
                    return Ok(());
                }
                let layer = doc.layer_mut(layer_id)?;
                layer.shapes.remove(idx);
                let n = parts.len();
                let mut ids: Vec<ShapeId> = (1..n).map(|_| doc.ids_mut().shape()).collect();
                ids.insert(0, *shape);
                let layer = doc.layer_mut(layer_id)?;
                for (k, (p, id)) in parts.into_iter().zip(ids).enumerate() {
                    let mut piece = Shape::new(
                        id,
                        ShapeKind::Path {
                            path: p,
                            closed: *closed,
                        },
                    );
                    piece.transform = s.transform;
                    piece.fill = s.fill.clone();
                    piece.stroke = s.stroke.clone();
                    piece.opacity = s.opacity;
                    layer.shapes.insert(idx + k, piece);
                }
            }
            Command::PlaceInside { contents, frame } => {
                if contents.contains(frame) {
                    return Ok(());
                }
                doc.shape(*frame)?;
                for id in contents {
                    doc.shape(*id)?;
                }
                let mut inner = Vec::new();
                for id in contents {
                    let (lid, i) = doc.locate(*id)?;
                    inner.push(doc.layer_mut(lid)?.shapes.remove(i));
                }
                let (flid, fi) = doc.locate(*frame)?;
                let frame_shape = doc.layer_mut(flid)?.shapes.remove(fi);
                let mut clip = Shape::new(
                    *frame,
                    ShapeKind::ClipFrame {
                        frame: Box::new(frame_shape.clone()),
                        contents: inner,
                    },
                );
                clip.name = frame_shape.name.clone();
                clip.opacity = frame_shape.opacity;
                let target = &mut doc.layer_mut(flid)?.shapes;
                target.insert(fi.min(target.len()), clip);
            }
            Command::ExtractContents { clip } => {
                let (lid, i) = doc.locate(*clip)?;
                let s = doc.layer_mut(lid)?.shapes.remove(i);
                match s.kind {
                    ShapeKind::ClipFrame { frame, contents } => {
                        // The clip frame's own transform (it was moved,
                        // scaled...) passes to what comes out, so nothing
                        // moves on the page.
                        let layer = doc.layer_mut(lid)?;
                        let mut f = *frame;
                        f.id = *clip;
                        f.absorb(s.transform);
                        layer.shapes.insert(i, f);
                        for (k, mut c) in contents.into_iter().enumerate() {
                            c.absorb(s.transform);
                            layer.shapes.insert(i + 1 + k, c);
                        }
                    }
                    other => doc
                        .layer_mut(lid)?
                        .shapes
                        .insert(i, Shape { kind: other, ..s }),
                }
            }
            Command::RenamePage { page, name } => doc.page_mut(*page)?.name = name.clone(),
            Command::DuplicatePage { page } => {
                let idx = doc
                    .pages
                    .iter()
                    .position(|p| p.id == *page)
                    .ok_or(Error::PageNotFound(*page))?;
                let src = doc.pages[idx].clone();
                let new_id = doc.ids_mut().page();
                let mut layers = Vec::with_capacity(src.layers.len());
                for l in &src.layers {
                    let lid = doc.ids_mut().layer();
                    let mut nl = Layer::new(lid, l.name.clone());
                    nl.visible = l.visible;
                    nl.printable = l.printable;
                    nl.locked = l.locked;
                    for s in &l.shapes {
                        nl.shapes.push(reid_shape(s, doc));
                    }
                    layers.push(nl);
                }
                let copy = Page {
                    id: new_id,
                    name: format!("{} (copy)", src.name),
                    size: src.size,
                    layers,
                    guides: src.guides.clone(),
                    background: src.background.clone(),
                };
                doc.pages.insert(idx + 1, copy);
            }
            Command::RenameLayer { layer, name } => doc.layer_mut(*layer)?.name = name.clone(),
            Command::ReorderLayer { layer, index } => {
                let (layers, _) = layer_list_mut(doc, *layer)?;
                let i = layers
                    .iter()
                    .position(|l| l.id == *layer)
                    .ok_or(Error::LayerNotFound(*layer))?;
                let l = layers.remove(i);
                let index = (*index).min(layers.len());
                layers.insert(index, l);
            }
            Command::SetShadow { shapes, shadow } => {
                for id in shapes {
                    any(doc, *id)?;
                }
                for id in shapes {
                    any_mut(doc, *id)?.shadow = *shadow;
                }
            }
            Command::AddGuide { page, guide } => doc.page_mut(*page)?.guides.push(*guide),
            Command::MoveGuide { page, index, guide } => {
                let p = doc.page_mut(*page)?;
                if let Some(g) = p.guides.get_mut(*index) {
                    *g = *guide;
                }
            }
            Command::DeleteGuide { page, index } => {
                let p = doc.page_mut(*page)?;
                if *index < p.guides.len() {
                    p.guides.remove(*index);
                }
            }
            Command::SetGuides { page, guides } => doc.page_mut(*page)?.guides = guides.clone(),
            Command::Ungroup { group } => {
                let (layer_id, idx) = doc.locate(*group)?;
                let layer = doc.layer_mut(layer_id)?;
                let g = layer.shapes.remove(idx);
                match g.kind {
                    ShapeKind::Group { children } => {
                        for (k, mut c) in children.into_iter().enumerate() {
                            c.absorb(g.transform);
                            layer.shapes.insert(idx + k, c);
                        }
                    }
                    other => {
                        // Not a group: put it back untouched.
                        layer.shapes.insert(idx, Shape { kind: other, ..g });
                    }
                }
            }
        }
        Ok(())
    }
}

/// Deep copy of a shape with fresh ids (groups included).
/// Any object, nested ones too.
fn any(doc: &Document, id: ShapeId) -> Result<&Shape> {
    doc.find_shape(id).ok_or(Error::ShapeNotFound(id))
}

/// Any object to change, nested ones too.
fn any_mut(doc: &mut Document, id: ShapeId) -> Result<&mut Shape> {
    doc.find_shape_mut(id).ok_or(Error::ShapeNotFound(id))
}

/// The list holding `layer`: the master layers (then true) or the layers
/// of its page.
fn layer_list_mut(doc: &mut Document, layer: LayerId) -> Result<(&mut Vec<Layer>, bool)> {
    if doc.master.iter().any(|l| l.id == layer) {
        return Ok((&mut doc.master, true));
    }
    doc.pages
        .iter_mut()
        .map(|p| &mut p.layers)
        .find(|layers| layers.iter().any(|l| l.id == layer))
        .map(|layers| (layers, false))
        .ok_or(Error::LayerNotFound(layer))
}

/// Take the objects `ids` out of `shapes`, inside groups and clip frames
/// too; a group left with nothing by it goes as well.
fn remove_shapes(shapes: &mut Vec<Shape>, ids: &[ShapeId]) {
    shapes.retain(|s| !ids.contains(&s.id));
    shapes.retain_mut(|s| match &mut s.kind {
        ShapeKind::Group { children } if !children.is_empty() => {
            remove_shapes(children, ids);
            !children.is_empty()
        }
        ShapeKind::ClipFrame { contents, .. } => {
            remove_shapes(contents, ids);
            true
        }
        _ => true,
    });
}

fn reid_shape(s: &Shape, doc: &mut Document) -> Shape {
    let mut c = s.clone();
    c.id = doc.ids_mut().shape();
    if let ShapeKind::Group { children } = &mut c.kind {
        let copies: Vec<Shape> = children.iter().map(|ch| reid_shape(ch, doc)).collect();
        *children = copies;
    }
    c
}

/// Give every object linked to an object style the style's parts again,
/// inside groups and clip frames too; links to missing styles go.
fn restyle(doc: &mut Document) {
    fn walk(shape: &mut Shape, styles: &[ObjectStyle]) {
        if let Some(id) = shape.style {
            match styles.iter().find(|s| s.id == id) {
                Some(st) => st.apply_to(shape),
                None => shape.style = None,
            }
        }
        match &mut shape.kind {
            ShapeKind::Group { children } => {
                for c in children {
                    walk(c, styles);
                }
            }
            ShapeKind::ClipFrame { frame, contents } => {
                walk(frame, styles);
                for c in contents {
                    walk(c, styles);
                }
            }
            _ => {}
        }
    }
    let styles = doc.object_styles.clone();
    for layer in doc.all_layers_mut() {
        for shape in &mut layer.shapes {
            walk(shape, &styles);
        }
    }
}

/// Run `f` on `shape` and, when it is a group, on everything inside it
/// (groups within groups too), as a fill or outline given to a group
/// reaches its objects. A clip frame passes it on to its frame object
/// (what is drawn as its fill and outline), not to its contents.
fn for_group_deep(shape: &mut Shape, f: &mut dyn FnMut(&mut Shape)) {
    f(shape);
    match &mut shape.kind {
        ShapeKind::Group { children } => {
            for c in children {
                for_group_deep(c, f);
            }
        }
        ShapeKind::ClipFrame { frame, .. } => f(frame),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn comments_get_free_ids_and_go_with_their_page() {
        use crate::document::{Comment, CommentMark};
        use crate::geometry::Point;
        let mut engine = crate::Engine::new(Document::new("t", Size::new(100.0, 100.0)));
        engine
            .run(&Command::AddPage {
                name: None,
                size: Size::new(100.0, 100.0),
            })
            .unwrap();
        let (p1, p2) = (engine.document().pages[0].id, engine.document().pages[1].id);
        let note = |id: u32, page: PageId, text: &str| Comment {
            id,
            page,
            mark: CommentMark::Pin {
                at: Point::new(10.0, 10.0),
            },
            author: "Ana".into(),
            time: 1_700_000_000,
            text: text.into(),
            replies: Vec::new(),
            resolved: false,
        };
        engine
            .run(&Command::SetComments {
                comments: vec![note(4, p1, "a"), note(4, p2, "b"), note(0, p2, "c")],
            })
            .unwrap();
        let ids: Vec<u32> = engine.document().comments.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![4, 5, 6]);
        assert_eq!(engine.undo_label(), Some("Comments"));
        // Deleting a page takes its comments with it; undo brings them back.
        engine.run(&Command::DeletePage { page: p2 }).unwrap();
        assert_eq!(engine.document().comments.len(), 1);
        engine.undo().unwrap();
        assert_eq!(engine.document().comments.len(), 3);
        // A comment on a page that does not exist is not kept.
        engine
            .run(&Command::SetComments {
                comments: vec![note(1, PageId(9999), "x")],
            })
            .unwrap();
        assert!(engine.document().comments.is_empty());
    }

    #[test]
    fn chosen_node_types_are_kept_until_the_node_count_changes() {
        use crate::nodes::NodeType;
        let mut doc = Document::default();
        let page = doc.pages[0].id;
        let layer = doc.page(page).expect("page").layers[0].id;
        let mut path = BezPath::new();
        path.move_to((0.0, 0.0));
        path.curve_to((5.0, 0.0), (10.0, 0.0), (15.0, 0.0));
        path.curve_to((20.0, 0.0), (25.0, 5.0), (30.0, 10.0));
        let id = doc.ids_mut().shape();
        let shape = Shape::new(
            id,
            ShapeKind::Path {
                path: path.clone(),
                closed: false,
            },
        );
        let mut eng = crate::Engine::new(doc);
        eng.run(&Command::AddShape { layer, shape }).expect("added");
        // The middle node has aligned handles of equal length: it reads
        // symmetrical, yet it is kept cusp once chosen so.
        let mid = crate::nodes::nodes(&path)[1].index;
        assert_eq!(crate::nodes::node_type(&path, mid), NodeType::Symmetrical);
        let mut types = std::collections::BTreeMap::new();
        types.insert(mid, NodeType::Cusp);
        eng.run(&Command::SetNodeTypes { shape: id, types })
            .expect("types");
        let s = eng.document().find_shape(id).expect("shape");
        assert_eq!(
            crate::nodes::node_type_with(&path, mid, &s.node_types),
            NodeType::Cusp
        );
        // Saved and read back.
        let json = serde_json::to_string(eng.document()).expect("saved");
        let back: Document = serde_json::from_str(&json).expect("read");
        assert_eq!(back.find_shape(id).expect("shape").node_types.len(), 1);
        // A node moved: the curve keeps its node count, and the type.
        let moved = crate::nodes::move_node(&path, mid, crate::geometry::Vec2::new(0.0, 2.0));
        eng.run(&Command::SetShapeKind {
            shape: id,
            kind: ShapeKind::Path {
                path: moved.clone(),
                closed: false,
            },
        })
        .expect("moved");
        assert_eq!(
            eng.document()
                .find_shape(id)
                .expect("shape")
                .node_types
                .len(),
            1
        );
        // A node more: the types no longer match their nodes and go.
        let added = crate::nodes::add_node(&moved, mid, 0.5);
        eng.run(&Command::SetShapeKind {
            shape: id,
            kind: ShapeKind::Path {
                path: added,
                closed: false,
            },
        })
        .expect("added node");
        assert!(eng
            .document()
            .find_shape(id)
            .expect("shape")
            .node_types
            .is_empty());
    }

    #[test]
    fn selection_groups_are_kept_saved_and_undone() {
        let mut eng = crate::Engine::new(Document::default());
        let a = crate::id::ShapeId(7);
        eng.run(&Command::SetSelectionGroup {
            number: 3,
            shapes: vec![a],
        })
        .expect("group 3");
        assert_eq!(eng.document().selection_groups.get(&3), Some(&vec![a]));
        // Saved with the drawing and read back.
        let json = serde_json::to_string(eng.document()).expect("saved");
        let back: Document = serde_json::from_str(&json).expect("read");
        assert_eq!(back.selection_groups.get(&3), Some(&vec![a]));
        // Undone like any change; no shapes removes a group; 10 is refused.
        eng.undo().expect("undo");
        assert!(eng.document().selection_groups.is_empty());
        eng.redo().expect("redo");
        eng.run(&Command::SetSelectionGroup {
            number: 3,
            shapes: Vec::new(),
        })
        .expect("cleared");
        assert!(eng.document().selection_groups.is_empty());
        assert!(eng
            .run(&Command::SetSelectionGroup {
                number: 10,
                shapes: vec![a],
            })
            .is_err());
    }

    use super::*;

    #[test]
    fn effects_switched_off_keep_their_place_in_the_stack() {
        use crate::live::Effect;
        let a = Effect::Perspective {
            corners: [crate::geometry::Point::ZERO; 4],
        };
        let b = Effect::BlockShadow {
            offset: crate::geometry::Vec2::new(1.0, 1.0),
            color: Color::BLACK,
            gap: 0.0,
        };
        let c = Effect::BlockShadow {
            offset: crate::geometry::Vec2::new(2.0, 2.0),
            color: Color::BLACK,
            gap: 0.0,
        };
        let stack = vec![(a.clone(), true), (b.clone(), false), (c.clone(), true)];
        let (effects, parked) = Shape::split_stack(stack.clone());
        assert_eq!(effects, vec![a.clone(), c.clone()]);
        assert_eq!(parked.len(), 1);
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let mut s = Shape::new(
            doc.ids_mut().shape(),
            ShapeKind::Rect {
                rect: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        let id = s.id;
        s.effects = vec![a.clone()];
        doc.layer_mut(layer).unwrap().shapes.push(s);
        Command::SetEffectStack {
            shape: id,
            effects,
            parked,
        }
        .apply(&mut doc)
        .unwrap();
        let s = doc.find_shape(id).unwrap();
        assert_eq!(s.effect_stack(), stack);
        assert_eq!(s.effects.len(), 2, "only the effects that are on are drawn");
        // A parked index past the end still comes back, at the end.
        let mut odd = s.clone();
        odd.parked_effects[0].index = 99;
        assert_eq!(odd.effect_stack().len(), 3);
        assert_eq!(odd.effect_stack()[2], (b, false));
    }

    #[test]
    fn only_clip_frames_take_an_opacity_mask() {
        use crate::document::OpacityMask;
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let r = Shape::new(
            doc.ids_mut().shape(),
            ShapeKind::Rect {
                rect: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        let rid = r.id;
        let clip = Shape::new(
            doc.ids_mut().shape(),
            ShapeKind::ClipFrame {
                frame: Box::new(r.clone()),
                contents: Vec::new(),
            },
        );
        let cid = clip.id;
        doc.layer_mut(layer).unwrap().shapes.extend([r, clip]);
        let mask = Some(OpacityMask { invert: true });
        assert!(Command::SetOpacityMask { shape: rid, mask }
            .apply(&mut doc)
            .is_err());
        Command::SetOpacityMask { shape: cid, mask }
            .apply(&mut doc)
            .unwrap();
        assert_eq!(doc.find_shape(cid).unwrap().opacity_mask, mask);
        let back = Document::from_json(&doc.to_json().unwrap()).unwrap();
        assert_eq!(back.find_shape(cid).unwrap().opacity_mask, mask);
        Command::SetOpacityMask {
            shape: cid,
            mask: None,
        }
        .apply(&mut doc)
        .unwrap();
        assert!(doc.find_shape(cid).unwrap().opacity_mask.is_none());
    }
    use crate::geometry::Rect;

    fn rect(doc: &mut Document, x: f64) -> Shape {
        let id = doc.ids_mut().shape();
        Shape::new(
            id,
            ShapeKind::Rect {
                rect: Rect::new(x, 0.0, x + 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        )
    }

    /// Objects inside a group take page-space transforms, fills and
    /// outlines, and can be deleted; a group left empty goes too.
    #[test]
    fn fills_and_outlines_given_to_a_group_reach_its_objects() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 20.0);
        let (ia, ib) = (a.id, b.id);
        for shape in [a, b] {
            Command::AddShape { layer, shape }.apply(&mut doc).unwrap();
        }
        Command::Group { shapes: vec![ia] }.apply(&mut doc).unwrap();
        let inner = doc.top_level_of(ia).unwrap();
        Command::Group {
            shapes: vec![inner, ib],
        }
        .apply(&mut doc)
        .unwrap();
        let outer = doc.top_level_of(ib).unwrap();
        let red = Fill::Solid(Color::rgb8(255, 0, 0));
        Command::SetFill {
            shapes: vec![outer],
            fill: red.clone(),
        }
        .apply(&mut doc)
        .unwrap();
        Command::SetStroke {
            shapes: vec![outer],
            stroke: None,
        }
        .apply(&mut doc)
        .unwrap();
        for id in [ia, ib] {
            let s = doc.find_shape(id).unwrap();
            assert_eq!(s.fill, red);
            assert!(s.stroke.is_none());
        }
    }

    #[test]
    fn objects_inside_groups_can_be_changed_and_deleted() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 20.0);
        let (ia, ib) = (a.id, b.id);
        for shape in [a, b] {
            Command::AddShape { layer, shape }.apply(&mut doc).unwrap();
        }
        Command::Group {
            shapes: vec![ia, ib],
        }
        .apply(&mut doc)
        .unwrap();
        let gid = doc.layer(layer).unwrap().shapes[0].id;
        // The group is turned and scaled, so the child's space is not the
        // page's.
        let g = Affine::translate((100.0, 50.0)) * Affine::rotate(0.5) * Affine::scale(2.0);
        Command::TransformShapes {
            shapes: vec![gid],
            transform: g,
        }
        .apply(&mut doc)
        .unwrap();
        assert!(doc.is_nested(ia) && !doc.is_nested(gid));
        assert_eq!(doc.top_level_of(ib), Some(gid));
        assert_eq!(doc.ancestors(ib), vec![gid]);
        assert!(doc.ancestors(gid).is_empty());
        let before = doc.shape_on_page(ia).unwrap().bounds();
        // A page-space move moves the child by exactly that on the page.
        Command::TransformShapes {
            shapes: vec![ia],
            transform: Affine::translate((7.0, -3.0)),
        }
        .apply(&mut doc)
        .unwrap();
        let after = doc.shape_on_page(ia).unwrap().bounds();
        assert!((after.x0 - before.x0 - 7.0).abs() < 1e-9);
        assert!((after.y0 - before.y0 + 3.0).abs() < 1e-9);
        assert!((after.width() - before.width()).abs() < 1e-9);
        // The other child stays where it was.
        let b_page = doc.shape_on_page(ib).unwrap().bounds();
        let expected = {
            use crate::geometry::Shape as _;
            (g * Rect::new(20.0, 0.0, 30.0, 10.0).to_path(0.01)).bounding_box()
        };
        assert!((b_page.x0 - expected.x0).abs() < 1e-6);
        // Fills and outlines reach the child.
        let red = Fill::Solid(Color::rgb8(255, 0, 0));
        Command::SetFill {
            shapes: vec![ia],
            fill: red.clone(),
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.find_shape(ia).unwrap().fill, red);
        Command::SetStroke {
            shapes: vec![ib],
            stroke: None,
        }
        .apply(&mut doc)
        .unwrap();
        assert!(doc.find_shape(ib).unwrap().stroke.is_none());
        // Deleting one child keeps the group; deleting the last one takes
        // the group away.
        Command::DeleteShapes { shapes: vec![ia] }
            .apply(&mut doc)
            .unwrap();
        assert!(doc.find_shape(ia).is_none());
        assert_eq!(doc.top_level_of(ib), Some(gid));
        Command::DeleteShapes { shapes: vec![ib] }
            .apply(&mut doc)
            .unwrap();
        assert!(doc.layer(layer).unwrap().shapes.is_empty());
        // Unknown objects are refused.
        assert!(Command::DeleteShapes { shapes: vec![ia] }
            .apply(&mut doc)
            .is_err());
    }

    #[test]
    fn group_and_ungroup_keep_page_positions() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 20.0);
        let (ia, ib) = (a.id, b.id);
        Command::AddShape { layer, shape: a }
            .apply(&mut doc)
            .unwrap();
        Command::AddShape { layer, shape: b }
            .apply(&mut doc)
            .unwrap();
        Command::Group {
            shapes: vec![ia, ib],
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.layer(layer).unwrap().shapes.len(), 1);
        let gid = doc.layer(layer).unwrap().shapes[0].id;
        Command::TransformShapes {
            shapes: vec![gid],
            transform: Affine::translate((5.0, 5.0)),
        }
        .apply(&mut doc)
        .unwrap();
        Command::Ungroup { group: gid }.apply(&mut doc).unwrap();
        let shapes = &doc.layer(layer).unwrap().shapes;
        assert_eq!(shapes.len(), 2);
        let bb = shapes[1].bounds();
        assert!((bb.x0 - 25.0).abs() < 1e-9 && (bb.y0 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn grouping_objects_of_several_layers_keeps_their_drawing_order() {
        let mut doc = Document::default();
        let page = doc.pages[0].id;
        let lower = doc.pages[0].layers[0].id;
        Command::AddLayer {
            page,
            name: "Upper".into(),
        }
        .apply(&mut doc)
        .unwrap();
        let upper = doc.pages[0].layers[1].id;
        // The lower layer holds two objects, the upper one one: the upper
        // object is drawn last although it is first in its layer.
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 20.0);
        let c = rect(&mut doc, 40.0);
        let (ia, ib, ic) = (a.id, b.id, c.id);
        for (layer, s) in [(lower, a), (lower, b), (upper, c)] {
            Command::AddShape { layer, shape: s }
                .apply(&mut doc)
                .unwrap();
        }
        Command::Group {
            shapes: vec![ic, ib, ia],
        }
        .apply(&mut doc)
        .unwrap();
        let g = &doc.layer(lower).unwrap().shapes[0];
        let ShapeKind::Group { children } = &g.kind else {
            panic!("a group");
        };
        let order: Vec<_> = children.iter().map(|s| s.id).collect();
        assert_eq!(order, vec![ia, ib, ic]);
    }

    #[test]
    fn combine_then_break_apart_round_trips() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 20.0);
        let (ia, ib) = (a.id, b.id);
        Command::AddShape { layer, shape: a }
            .apply(&mut doc)
            .unwrap();
        Command::AddShape { layer, shape: b }
            .apply(&mut doc)
            .unwrap();
        Command::Combine {
            shapes: vec![ia, ib],
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.layer(layer).unwrap().shapes.len(), 1);
        let combined = doc.layer(layer).unwrap().shapes[0].id;
        assert_eq!(combined, ib);
        Command::BreakApart { shape: combined }
            .apply(&mut doc)
            .unwrap();
        assert_eq!(doc.layer(layer).unwrap().shapes.len(), 2);
    }

    #[test]
    fn duplicate_page_copies_shapes_with_new_ids() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let page = doc.pages[0].id;
        let a = rect(&mut doc, 0.0);
        let ia = a.id;
        Command::AddShape { layer, shape: a }
            .apply(&mut doc)
            .unwrap();
        Command::DuplicatePage { page }.apply(&mut doc).unwrap();
        assert_eq!(doc.pages.len(), 2);
        let copy = &doc.pages[1];
        assert_eq!(copy.layers[0].shapes.len(), 1);
        assert_ne!(copy.layers[0].shapes[0].id, ia);
        assert_ne!(copy.layers[0].id, layer);
    }

    #[test]
    fn clip_frame_place_and_extract() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 5.0);
        let (ia, ib) = (a.id, b.id);
        Command::AddShape { layer, shape: a }
            .apply(&mut doc)
            .unwrap();
        Command::AddShape { layer, shape: b }
            .apply(&mut doc)
            .unwrap();
        Command::PlaceInside {
            contents: vec![ia],
            frame: ib,
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.layer(layer).unwrap().shapes.len(), 1);
        assert!(matches!(
            doc.shape(ib).unwrap().1.kind,
            ShapeKind::ClipFrame { .. }
        ));
        Command::ExtractContents { clip: ib }
            .apply(&mut doc)
            .unwrap();
        assert_eq!(doc.layer(layer).unwrap().shapes.len(), 2);
        assert!(doc.shape(ia).is_ok());
    }

    /// Extracting the contents of a clip frame that was moved leaves the
    /// contents and the frame where they were seen: nothing jumps back.
    #[test]
    fn extracting_from_a_moved_clip_frame_moves_nothing() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 5.0);
        let (ia, ib) = (a.id, b.id);
        for shape in [a, b] {
            Command::AddShape { layer, shape }.apply(&mut doc).unwrap();
        }
        Command::PlaceInside {
            contents: vec![ia],
            frame: ib,
        }
        .apply(&mut doc)
        .unwrap();
        Command::TransformShapes {
            shapes: vec![ib],
            transform: Affine::translate((30.0, 20.0)),
        }
        .apply(&mut doc)
        .unwrap();
        let seen_a = doc.shape_on_page(ia).unwrap().bounds();
        let seen_b = doc.shape(ib).unwrap().1.bounds();
        Command::ExtractContents { clip: ib }
            .apply(&mut doc)
            .unwrap();
        let close = |p: crate::geometry::Rect, q: crate::geometry::Rect| {
            (p.x0 - q.x0).abs() < 1e-9
                && (p.y0 - q.y0).abs() < 1e-9
                && (p.x1 - q.x1).abs() < 1e-9
                && (p.y1 - q.y1).abs() < 1e-9
        };
        assert!(close(doc.shape(ia).unwrap().1.bounds(), seen_a));
        assert!(close(doc.shape(ib).unwrap().1.bounds(), seen_b));
    }

    /// An outline or fill given to a clip frame reaches its frame object
    /// (what is drawn), not the contents.
    #[test]
    fn a_clip_frame_takes_an_outline_and_fill_on_its_frame() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 5.0);
        let (ia, ib) = (a.id, b.id);
        for shape in [a, b] {
            Command::AddShape { layer, shape }.apply(&mut doc).unwrap();
        }
        Command::PlaceInside {
            contents: vec![ia],
            frame: ib,
        }
        .apply(&mut doc)
        .unwrap();
        let before = doc.find_shape(ia).unwrap().stroke.clone();
        let stroke = crate::style::Stroke {
            width: 1.5,
            ..Default::default()
        };
        Command::SetStroke {
            shapes: vec![ib],
            stroke: Some(stroke.clone()),
        }
        .apply(&mut doc)
        .unwrap();
        Command::SetFill {
            shapes: vec![ib],
            fill: Fill::None,
        }
        .apply(&mut doc)
        .unwrap();
        let ShapeKind::ClipFrame { frame, contents } = &doc.shape(ib).unwrap().1.kind else {
            panic!("not a clip frame");
        };
        assert_eq!(frame.stroke, Some(stroke));
        assert_eq!(frame.fill, Fill::None);
        assert_eq!(contents[0].stroke, before);
        // Another kind (a text style, curves) changes the frame object and
        // keeps the contents.
        let ellipse = ShapeKind::Ellipse {
            rect: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
            arc: None,
        };
        Command::SetShapeKind {
            shape: ib,
            kind: ellipse.clone(),
        }
        .apply(&mut doc)
        .unwrap();
        let ShapeKind::ClipFrame { frame, contents } = &doc.shape(ib).unwrap().1.kind else {
            panic!("the clip frame was replaced");
        };
        assert_eq!(frame.kind, ellipse);
        assert_eq!(contents.len(), 1);
    }

    #[test]
    fn styled_objects_follow_their_style() {
        use crate::document::{ObjectStyle, StyleParts};
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = rect(&mut doc, 0.0);
        let b = rect(&mut doc, 20.0);
        let c = rect(&mut doc, 40.0);
        let (ia, ib, ic) = (a.id, b.id, c.id);
        for shape in [a, b, c] {
            Command::AddShape { layer, shape }.apply(&mut doc).unwrap();
        }
        let red = Fill::Solid(Color::rgb8(255, 0, 0));
        let mut style = ObjectStyle::from_shape("Red", doc.shape(ia).unwrap().1);
        style.fill = red.clone();
        style.parts = StyleParts {
            fill: true,
            outline: false,
            transparency: true,
        };
        style.opacity = 0.5;
        // A style without an id gets one.
        Command::SetObjectStyles {
            styles: vec![style.clone()],
        }
        .apply(&mut doc)
        .unwrap();
        let id = doc.object_styles[0].id;
        assert_eq!(id, 1);
        let stroke_before = doc.shape(ia).unwrap().1.stroke.clone();
        Command::LinkObjectStyle {
            shapes: vec![ia, ib],
            style: Some(id),
        }
        .apply(&mut doc)
        .unwrap();
        let a = doc.shape(ia).unwrap().1;
        assert_eq!(
            (a.fill.clone(), a.opacity, a.style),
            (red.clone(), 0.5, Some(id))
        );
        // Parts the style leaves out stay as they were.
        assert_eq!(a.stroke, stroke_before);
        // Grouped objects stay linked and follow too.
        Command::Group {
            shapes: vec![ib, ic],
        }
        .apply(&mut doc)
        .unwrap();
        let blue = Fill::Solid(Color::rgb8(0, 0, 255));
        let mut styles = doc.object_styles.clone();
        styles[0].fill = blue.clone();
        Command::SetObjectStyles { styles }.apply(&mut doc).unwrap();
        assert_eq!(doc.shape(ia).unwrap().1.fill, blue);
        assert_eq!(doc.find_shape(ib).unwrap().fill, blue);
        // The unlinked one keeps its own fill.
        assert_ne!(doc.find_shape(ic).unwrap().fill, blue);
        // Breaking the link keeps the properties; later changes pass it by.
        Command::LinkObjectStyle {
            shapes: vec![ia],
            style: None,
        }
        .apply(&mut doc)
        .unwrap();
        let mut styles = doc.object_styles.clone();
        styles[0].fill = red.clone();
        Command::SetObjectStyles { styles }.apply(&mut doc).unwrap();
        let a = doc.shape(ia).unwrap().1;
        assert_eq!((a.fill.clone(), a.style), (blue, None));
        assert_eq!(doc.find_shape(ib).unwrap().fill, red);
        // Deleting the style drops the links; a missing style is an error.
        Command::SetObjectStyles { styles: Vec::new() }
            .apply(&mut doc)
            .unwrap();
        assert_eq!(doc.find_shape(ib).unwrap().style, None);
        assert!(Command::LinkObjectStyle {
            shapes: vec![ia],
            style: Some(7),
        }
        .apply(&mut doc)
        .is_err());
        // Older files: styles without parts set fill and outline.
        let mut json = serde_json::to_value(&style).unwrap();
        for key in ["id", "parts", "opacity"] {
            json.as_object_mut().unwrap().remove(key);
        }
        let old: ObjectStyle = serde_json::from_value(json).unwrap();
        assert_eq!(old.parts, StyleParts::default());
        assert_eq!((old.id, old.opacity), (0, 1.0));
    }

    #[test]
    fn new_objects_take_the_defaults_of_their_kind() {
        use crate::document::{DefaultKind, ObjectDefaults};
        let mut doc = Document::default();
        let mut defaults = ObjectDefaults::default();
        defaults.paragraph_text.fill = Fill::Solid(Color::rgb8(0, 128, 0));
        Command::SetObjectDefaults {
            defaults: defaults.clone(),
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(doc.defaults, defaults);
        let mut s = rect(&mut doc, 0.0);
        s.fill = Fill::Solid(Color::BLACK);
        doc.defaults.apply_to(&mut s);
        assert_eq!(s.fill, Fill::None);
        assert_eq!(s.stroke.as_ref().map(|k| k.width), Some(0.2));
        let text = |frame| ShapeKind::Text {
            spans: Vec::new(),
            origin: crate::geometry::Point::ZERO,
            frame,
            align: Default::default(),
            para: Default::default(),
            on_path: None,
        };
        assert_eq!(DefaultKind::of(&text(None)), DefaultKind::ArtisticText);
        let mut p = Shape::new(crate::id::ShapeId(99), text(Some(Size::new(10.0, 10.0))));
        doc.defaults.apply_to(&mut p);
        assert_eq!(p.fill, Fill::Solid(Color::rgb8(0, 128, 0)));
        assert!(p.stroke.is_none());
        // Paragraph layout defaults reach paragraph text, not artistic text.
        doc.defaults.paragraph.columns = 3;
        doc.defaults.paragraph.tabs = vec![10.0];
        let mut p = Shape::new(crate::id::ShapeId(98), text(Some(Size::new(10.0, 10.0))));
        doc.defaults.apply_to(&mut p);
        let mut a = Shape::new(crate::id::ShapeId(97), text(None));
        doc.defaults.apply_to(&mut a);
        let cols = |s: &Shape| match &s.kind {
            ShapeKind::Text { para, .. } => (para.columns, para.tabs.clone()),
            _ => (0, Vec::new()),
        };
        assert_eq!(cols(&p), (3, vec![10.0]));
        assert_eq!(cols(&a), (1, Vec::new()));
        // Older files have none: the defaults of a new drawing.
        let mut json: serde_json::Value = serde_json::from_str(&doc.to_json().unwrap()).unwrap();
        json.as_object_mut().unwrap().remove("defaults");
        let old = Document::from_json(&json.to_string()).unwrap();
        assert_eq!(old.defaults, ObjectDefaults::default());
    }

    #[test]
    fn delete_unknown_shape_fails_cleanly() {
        let mut doc = Document::default();
        let err = Command::DeleteShapes {
            shapes: vec![ShapeId(999)],
        }
        .apply(&mut doc)
        .unwrap_err();
        assert!(matches!(err, Error::ShapeNotFound(_)));
    }

    #[test]
    fn master_layers_reorder_and_delete_down_to_none() {
        let mut doc = Document::default();
        for name in ["Back", "Front"] {
            Command::AddMasterLayer {
                name: name.into(),
                scope: MasterScope::All,
            }
            .apply(&mut doc)
            .unwrap();
        }
        let names =
            |doc: &Document| -> Vec<String> { doc.master.iter().map(|l| l.name.clone()).collect() };
        let back = doc.master[0].id;
        Command::ReorderLayer {
            layer: back,
            index: 1,
        }
        .apply(&mut doc)
        .unwrap();
        assert_eq!(names(&doc), ["Front", "Back"]);
        for l in doc.master.iter().map(|l| l.id).collect::<Vec<_>>() {
            Command::DeleteLayer { layer: l }.apply(&mut doc).unwrap();
        }
        assert!(doc.master.is_empty());
        // A page keeps its last layer.
        assert_eq!(doc.pages[0].layers.len(), 1);
        let last = doc.pages[0].layers[0].id;
        let err = Command::DeleteLayer { layer: last }
            .apply(&mut doc)
            .unwrap_err();
        assert!(matches!(err, Error::LastLayer));
        let err = Command::DeleteLayer { layer: back }
            .apply(&mut doc)
            .unwrap_err();
        assert!(matches!(err, Error::LayerNotFound(_)));
    }

    #[test]
    fn fill_opacity_is_set_undone_and_saved_only_when_not_opaque() {
        use crate::engine::Engine;
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let mut r = Shape::new(
            doc.ids_mut().shape(),
            ShapeKind::Rect {
                rect: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        r.fill = Fill::Solid(crate::Color::rgb8(255, 0, 0));
        let id = r.id;
        // Opaque: nothing written, and older files read as opaque.
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("fill_opacity"), "{json}");
        let back: Shape = serde_json::from_str(&json).unwrap();
        assert_eq!(back.fill_opacity, 1.0);
        doc.layer_mut(layer).unwrap().shapes.push(r);
        let mut e = Engine::new(doc);
        let cmd = Command::SetFillOpacity {
            shapes: vec![id],
            opacity: 0.25,
        };
        assert_eq!(cmd.label(), "Fill Transparency");
        e.run(&cmd).unwrap();
        let s = e.document().find_shape(id).unwrap();
        assert_eq!(s.fill_opacity, 0.25);
        // The object's own opacity and the outline stay.
        assert_eq!(s.opacity, 1.0);
        assert!(s.stroke.is_some());
        let saved = e.document().to_json().unwrap();
        assert!(saved.contains("fill_opacity"));
        let reread = Document::from_json(&saved).unwrap();
        assert_eq!(reread.find_shape(id).unwrap().fill_opacity, 0.25);
        e.undo().unwrap();
        assert_eq!(e.document().find_shape(id).unwrap().fill_opacity, 1.0);
        e.redo().unwrap();
        assert_eq!(e.document().find_shape(id).unwrap().fill_opacity, 0.25);
        // Out of range and NaN clamp; an unknown object is an error.
        e.run(&Command::SetFillOpacity {
            shapes: vec![id],
            opacity: 7.0,
        })
        .unwrap();
        assert_eq!(e.document().find_shape(id).unwrap().fill_opacity, 1.0);
        e.run(&Command::SetFillOpacity {
            shapes: vec![id],
            opacity: 0.5,
        })
        .unwrap();
        e.run(&Command::SetFillOpacity {
            shapes: vec![id],
            opacity: f64::NAN,
        })
        .unwrap();
        assert_eq!(e.document().find_shape(id).unwrap().fill_opacity, 1.0);
        assert!(e
            .run(&Command::SetFillOpacity {
                shapes: vec![ShapeId(999_999)],
                opacity: 0.5,
            })
            .is_err());
    }

    #[test]
    fn fill_opacity_reaches_the_shapes_of_a_group_and_not_extra_fills() {
        let mut doc = Document::default();
        let layer = doc.pages[0].layers[0].id;
        let a = Shape::new(
            doc.ids_mut().shape(),
            ShapeKind::Rect {
                rect: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        let aid = a.id;
        let g = Shape::new(doc.ids_mut().shape(), ShapeKind::Group { children: vec![a] });
        let gid = g.id;
        doc.layer_mut(layer).unwrap().shapes.push(g);
        Command::SetFillOpacity {
            shapes: vec![gid],
            opacity: 0.5,
        }
        .apply(&mut doc)
        .unwrap();
        let mut layered = doc.find_shape(aid).unwrap().clone();
        assert_eq!(layered.fill_opacity, 0.5);
        layered.extra_fills = vec![Fill::Solid(crate::Color::rgb8(0, 0, 255))];
        let parts = layered.layers().unwrap();
        let fills: Vec<f64> = parts
            .iter()
            .filter(|p| p.stroke.is_none())
            .map(|p| p.fill_opacity)
            .collect();
        assert_eq!(fills, vec![0.5, 1.0]);
    }

    #[test]
    fn an_object_style_carries_the_fill_transparency_with_the_fill() {
        let mut s = Shape::new(
            ShapeId(1),
            ShapeKind::Rect {
                rect: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill_opacity = 0.4;
        let style = ObjectStyle::from_shape("s", &s);
        assert_eq!(style.fill_opacity, 0.4);
        let mut t = Shape::new(ShapeId(2), s.kind.clone());
        style.apply_to(&mut t);
        assert_eq!(t.fill_opacity, 0.4);
        // Styles saved before keep an opaque fill.
        let mut json = serde_json::to_value(&style).unwrap();
        json.as_object_mut().unwrap().remove("fill_opacity");
        let old: ObjectStyle = serde_json::from_value(json).unwrap();
        assert_eq!(old.fill_opacity, 1.0);
    }
}
