//! The private clipboard format: the copied objects with every property
//! and the object styles they link to, as versioned JSON. Only GraphDraw
//! reads it, so it keeps what the public formats (SVG, PNG) lose.
// Used by the clipboard backend (next tasks).
#![allow(dead_code)]

use graphdraw_core::{ObjectStyle, Shape, ShapeKind, Symbol};
use serde::{Deserialize, Serialize};

/// Name of the clipboard format as registered with the system.
pub const FORMAT_NAME: &str = "GraphDraw Objects";
/// Highest payload version this build writes and understands.
pub const VERSION: u32 = 1;
/// Largest payload accepted on decode, in bytes.
pub const MAX_BYTES: usize = 256 * 1024 * 1024;
/// Value of the `format` field that identifies the payload.
const TAG: &str = "graphdraw-objects";
/// Deepest symbol nesting that is expanded before giving up.
const MAX_DEPTH: usize = 32;

/// The decoded clipboard payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Payload {
    /// Always the format tag.
    pub format: String,
    /// Payload version.
    pub version: u32,
    /// The copied objects.
    pub shapes: Vec<Shape>,
    /// The object styles the shapes link to.
    #[serde(default)]
    pub styles: Vec<ObjectStyle>,
}

/// Replaces symbol instances by groups holding a copy of the symbol's
/// shape, so the payload never refers to another drawing's symbol table.
fn flatten(shape: &Shape, symbols: &[Symbol], depth: usize) -> Shape {
    let mut out = shape.clone();
    if depth > MAX_DEPTH {
        return out;
    }
    match &shape.kind {
        ShapeKind::SymbolInstance { .. } => {
            let children = shape
                .expand(symbols)
                .iter()
                .map(|c| flatten(c, symbols, depth + 1))
                .collect();
            out.kind = ShapeKind::Group { children };
        }
        ShapeKind::Group { children } => {
            out.kind = ShapeKind::Group {
                children: children
                    .iter()
                    .map(|c| flatten(c, symbols, depth + 1))
                    .collect(),
            };
        }
        ShapeKind::ClipFrame { frame, contents } => {
            out.kind = ShapeKind::ClipFrame {
                frame: Box::new(flatten(frame, symbols, depth + 1)),
                contents: contents
                    .iter()
                    .map(|c| flatten(c, symbols, depth + 1))
                    .collect(),
            };
        }
        _ => {}
    }
    out
}

/// Serializes the shapes and styles, expanding symbol instances first.
pub fn encode(
    shapes: &[Shape],
    styles: &[ObjectStyle],
    symbols: &[Symbol],
) -> Result<Vec<u8>, String> {
    let payload = Payload {
        format: TAG.to_string(),
        version: VERSION,
        shapes: shapes.iter().map(|s| flatten(s, symbols, 0)).collect(),
        styles: styles.to_vec(),
    };
    serde_json::to_vec(&payload).map_err(|e| e.to_string())
}

/// Parses a payload, rejecting oversized input, other formats and
/// versions newer than [`VERSION`].
pub fn decode(bytes: &[u8]) -> Result<Payload, String> {
    if bytes.len() > MAX_BYTES {
        return Err("payload too large".to_string());
    }
    let payload: Payload = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if payload.format != TAG {
        return Err("not a GraphDraw objects payload".to_string());
    }
    if payload.version > VERSION {
        return Err(format!("unsupported payload version {}", payload.version));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::{Rect, ShapeId};

    fn rect_shape(id: u64) -> Shape {
        Shape::new(
            ShapeId(id),
            ShapeKind::Rect {
                rect: Rect::new(0.0, 0.0, 10.0, 20.0),
                radius: 0.0,
                corners: None,
            },
        )
    }

    fn has_instance(s: &Shape) -> bool {
        match &s.kind {
            ShapeKind::SymbolInstance { .. } => true,
            ShapeKind::Group { children } => children.iter().any(has_instance),
            ShapeKind::ClipFrame { frame, contents } => {
                has_instance(frame) || contents.iter().any(has_instance)
            }
            _ => false,
        }
    }

    #[test]
    fn round_trip_keeps_every_field() {
        let mut s = rect_shape(7);
        s.name = Some("kept".to_string());
        let shapes = vec![s];
        let bytes = encode(&shapes, &[], &[]).expect("encode");
        let back = decode(&bytes).expect("decode");
        assert_eq!(back.shapes, shapes);
        assert!(back.styles.is_empty());
        assert_eq!(back.version, VERSION);
    }

    #[test]
    fn rejects_other_formats_newer_versions_and_garbage() {
        assert!(decode(b"not json").is_err());
        assert!(decode(br#"{"format":"other","version":1,"shapes":[],"styles":[]}"#).is_err());
        assert!(
            decode(br#"{"format":"graphdraw-objects","version":99,"shapes":[],"styles":[]}"#)
                .is_err()
        );
        assert!(decode(&[0xff; 16]).is_err());
    }

    #[test]
    fn rejects_payloads_over_the_limit() {
        let big = vec![b' '; MAX_BYTES + 1];
        assert!(decode(&big).is_err());
    }

    #[test]
    fn symbol_instances_are_expanded_even_inside_groups() {
        let symbols = vec![Symbol {
            name: "sym".to_string(),
            shape: rect_shape(1),
        }];
        let inst = Shape::new(ShapeId(2), ShapeKind::SymbolInstance { index: 0 });
        let group = Shape::new(
            ShapeId(3),
            ShapeKind::Group {
                children: vec![inst.clone()],
            },
        );
        let bytes = encode(&[inst, group], &[], &symbols).expect("encode");
        let back = decode(&bytes).expect("decode");
        assert_eq!(back.shapes.len(), 2);
        assert!(back.shapes.iter().all(|s| !has_instance(s)));
        assert!(matches!(back.shapes[0].kind, ShapeKind::Group { .. }));
    }
}
