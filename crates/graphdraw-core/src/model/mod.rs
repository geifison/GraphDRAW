//! Modelo de documento do GraphDraw (doc 02). Unidade interna: µm (GD-DOC-01).
//! Eixo Y para cima, origem no canto inferior esquerdo da página (GD-DOC-02).

pub mod color;
pub mod document;
pub mod geom;
pub mod object;
pub mod paint;

pub use color::Color;
pub use document::{
    ColorsResource, Document, DocumentData, Guide, Layer, MasterPage, Metadata, NamedColor, Page,
    Settings,
};
pub use geom::{FillRule, Node, NodeKind, Subpath, SubpathBuilder, Transform};
pub use object::{
    Corner, EllipseData, FontSpec, GroupData, ImageData, Object, ObjectKind, Paragraph, PathData,
    PolygonData, RectData, TextData, TextSpan, new_id,
};
pub use paint::{Fill, Gradient, GradientStop, Stroke};
