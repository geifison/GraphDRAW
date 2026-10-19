//! Print Merge: the records, their fields and the merge itself (see
//! `docs/behavior/print-merge.md`). The dialog edits a
//! [`PrintMergeState`]; [`perform`] turns the current page into one page
//! per record.

use crate::app::App;
use crate::i18n::trf;
use crate::ui::dialogs::PrintMergeState;
use tracedraw_core::geometry::{Rect, Shape as _};
use tracedraw_core::{Color, Command, Fill, LayerId, Shape, ShapeKind, TextSpan};

/// What a field holds.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FieldKind {
    /// Text put in place of `<Field>` in every text object.
    #[default]
    Text,
    /// A counter: `start`, then `step` more each record, padded with zeros
    /// to `digits` digits; its cells are not typed in.
    Number { start: i64, step: i64, digits: u32 },
    /// The path of a picture (relative paths from the data file's folder);
    /// a text object holding only `<Field>` becomes the picture, fitted in
    /// its box.
    Image,
    /// A QR code of the cell's text in place of a text object holding only
    /// `<Field>`, as large as the box allows.
    Qr,
}

/// The kind of field `i`, text when the state has none for it.
pub fn kind_of(st: &PrintMergeState, i: usize) -> FieldKind {
    st.kinds.get(i).cloned().unwrap_or_default()
}

/// The value of every cell as the merge uses it: counters worked out,
/// short records padded with empty cells.
pub fn values(st: &PrintMergeState) -> Vec<Vec<String>> {
    st.rows
        .iter()
        .enumerate()
        .map(|(r, row)| {
            (0..st.headers.len())
                .map(|c| match kind_of(st, c) {
                    FieldKind::Number {
                        start,
                        step,
                        digits,
                    } => {
                        let n = start.saturating_add(step.saturating_mul(r as i64));
                        let width = digits.min(30) as usize;
                        if n < 0 {
                            format!("-{:0width$}", n.unsigned_abs())
                        } else {
                            format!("{n:0width$}")
                        }
                    }
                    _ => row.get(c).cloned().unwrap_or_default(),
                })
                .collect()
        })
        .collect()
}

/// Add a field named `name` (text) to every record.
pub fn add_field(st: &mut PrintMergeState, name: &str) {
    st.headers.push(name.to_string());
    st.kinds.resize(st.headers.len() - 1, FieldKind::Text);
    st.kinds.push(FieldKind::Text);
    for row in st.rows.iter_mut() {
        row.resize(st.headers.len(), String::new());
    }
}

/// Remove field `i` and its cells.
pub fn remove_field(st: &mut PrintMergeState, i: usize) {
    if i >= st.headers.len() {
        return;
    }
    st.headers.remove(i);
    if i < st.kinds.len() {
        st.kinds.remove(i);
    }
    for row in st.rows.iter_mut() {
        if i < row.len() {
            row.remove(i);
        }
    }
}

/// Add an empty record at the end.
pub fn add_record(st: &mut PrintMergeState) {
    st.rows.push(vec![String::new(); st.headers.len()]);
}

/// The records as CSV text, counters written out, with a header line.
pub fn to_csv(st: &PrintMergeState) -> String {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    let _ = w.write_record(&st.headers);
    for row in values(st) {
        let _ = w.write_record(&row);
    }
    w.into_inner()
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_default()
}

/// The text of a text object, all its runs together.
fn text_of(s: &Shape) -> Option<String> {
    match &s.kind {
        ShapeKind::Text { spans, .. } => Some(spans.iter().map(|sp| sp.text.as_str()).collect()),
        _ => None,
    }
}

/// The box a field object stands for, in its own coordinates: the frame of
/// paragraph text, or what artistic text covers.
fn local_box(s: &Shape) -> Rect {
    let page = s.bounds();
    if s.transform.determinant().abs() < 1e-12 {
        return page;
    }
    (s.transform.inverse() * page.to_path(0.01)).bounding_box()
}

/// The largest rectangle of aspect `w : h` centred in `r`.
fn fit(r: Rect, w: f64, h: f64) -> Rect {
    let k = (r.width() / w.max(1e-9)).min(r.height() / h.max(1e-9));
    let (fw, fh) = (w * k, h * k);
    let c = r.center();
    Rect::new(
        c.x - fw / 2.0,
        c.y - fh / 2.0,
        c.x + fw / 2.0,
        c.y + fh / 2.0,
    )
}

/// What takes the place of a picture or QR field object, or `None` when
/// the value gives nothing (an empty cell, a file that does not load).
fn replacement(
    kind: &FieldKind,
    value: &str,
    area: Rect,
    base: Option<&std::path::Path>,
) -> Option<(ShapeKind, Fill)> {
    if value.trim().is_empty() {
        return None;
    }
    match kind {
        FieldKind::Image => {
            let mut path = std::path::PathBuf::from(value.trim());
            if path.is_relative() {
                if let Some(dir) = base.and_then(|b| b.parent()) {
                    path = dir.join(path);
                }
            }
            let bytes = crate::files::read(&path).ok()?;
            let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
            let png = crate::bitmap_fx::encode(&img)?;
            Some((
                ShapeKind::Bitmap {
                    rect: fit(area, img.width() as f64, img.height() as f64),
                    width_px: img.width(),
                    height_px: img.height(),
                    png,
                    fx: None,
                },
                Fill::None,
            ))
        }
        FieldKind::Qr => {
            let side = area.width().min(area.height());
            let path = crate::ui::dialogs::qr_path(value, side).ok()?;
            let r = fit(area, 1.0, 1.0);
            let path = tracedraw_core::geometry::Affine::translate((r.x0, r.y0)) * path;
            Some((
                ShapeKind::Path { path, closed: true },
                Fill::Solid(Color::Cmyk {
                    c: 0.0,
                    m: 0.0,
                    y: 0.0,
                    k: 1.0,
                }),
            ))
        }
        _ => None,
    }
}

/// The commands that fill one page's field objects with one record.
fn fill_page(
    st: &PrintMergeState,
    page: &tracedraw_core::document::Page,
    row: &[String],
) -> Vec<Command> {
    let mut cmds = Vec::new();
    let layers: Vec<(LayerId, Vec<Shape>)> = page
        .layers
        .iter()
        .map(|l| (l.id, l.shapes.clone()))
        .collect();
    for (_, shapes) in layers {
        for s in shapes {
            let Some(text) = text_of(&s) else {
                continue;
            };
            // A picture or QR field: the object holds only its placeholder.
            let whole = st.headers.iter().enumerate().find(|(i, h)| {
                matches!(kind_of(st, *i), FieldKind::Image | FieldKind::Qr)
                    && text.trim() == format!("<{h}>")
            });
            if let Some((i, _)) = whole {
                let value = row.get(i).map(String::as_str).unwrap_or("");
                match replacement(&kind_of(st, i), value, local_box(&s), st.path.as_deref()) {
                    Some((kind, fill)) => {
                        cmds.push(Command::SetShapeKind { shape: s.id, kind });
                        cmds.push(Command::SetFill {
                            shapes: vec![s.id],
                            fill,
                        });
                        cmds.push(Command::SetStroke {
                            shapes: vec![s.id],
                            stroke: None,
                        });
                    }
                    None => cmds.push(Command::DeleteShapes { shapes: vec![s.id] }),
                }
                continue;
            }
            let ShapeKind::Text {
                spans,
                origin,
                frame,
                align,
                para,
                on_path,
            } = &s.kind
            else {
                continue;
            };
            let mut changed = false;
            let new_spans: Vec<TextSpan> = spans
                .iter()
                .map(|sp| {
                    let mut t = sp.text.clone();
                    for (i, (h, v)) in st.headers.iter().zip(row).enumerate() {
                        if matches!(kind_of(st, i), FieldKind::Image | FieldKind::Qr) {
                            continue;
                        }
                        let ph = format!("<{h}>");
                        if t.contains(&ph) {
                            t = t.replace(&ph, v);
                            changed = true;
                        }
                    }
                    TextSpan {
                        text: t,
                        ..sp.clone()
                    }
                })
                .collect();
            if changed {
                cmds.push(Command::SetShapeKind {
                    shape: s.id,
                    kind: ShapeKind::Text {
                        spans: new_spans,
                        origin: *origin,
                        frame: *frame,
                        align: *align,
                        para: para.clone(),
                        on_path: on_path.clone(),
                    },
                });
            }
        }
    }
    cmds
}

/// Duplicate the current page once per record and fill the copies'
/// fields, as one undo step per stage.
pub fn perform(app: &mut App, st: &PrintMergeState) {
    let rows = values(st);
    if rows.is_empty() {
        return;
    }
    let page = app.page;
    let template = match app.doc().page(page) {
        Ok(p) => p.clone(),
        Err(_) => return,
    };
    let cmds: Vec<Command> = rows
        .iter()
        .map(|_| Command::DuplicatePage { page })
        .collect();
    if let Err(e) = app.engine.run_batch("Print Merge", &cmds) {
        app.status = e.to_string();
        return;
    }
    // The copies follow the template; record k goes on the k-th of them,
    // so the pages read in the records' order.
    let idx = app
        .doc()
        .pages
        .iter()
        .position(|p| p.id == page)
        .unwrap_or(0);
    let new_pages: Vec<tracedraw_core::PageId> = app
        .doc()
        .pages
        .iter()
        .skip(idx + 1)
        .take(rows.len())
        .map(|p| p.id)
        .collect();
    let mut cmds = Vec::new();
    for (k, pid) in new_pages.iter().enumerate() {
        let (Some(row), Ok(p)) = (rows.get(k), app.doc().page(*pid)) else {
            continue;
        };
        cmds.extend(fill_page(st, p, row));
        cmds.push(Command::RenamePage {
            page: *pid,
            name: format!("{} {}", template.name, k + 1),
        });
    }
    if let Err(e) = app.engine.run_batch("Print Merge", &cmds) {
        app.status = e.to_string();
        return;
    }
    app.status = trf("status.merge_done", &[("n", &rows.len().to_string())]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> PrintMergeState {
        PrintMergeState {
            headers: vec!["Name".into(), "No".into(), "Link".into()],
            rows: vec![
                vec!["Ana".into(), String::new(), "https://a.example".into()],
                vec!["Bob".into()],
            ],
            kinds: vec![
                FieldKind::Text,
                FieldKind::Number {
                    start: 7,
                    step: 3,
                    digits: 3,
                },
                FieldKind::Qr,
            ],
            ..Default::default()
        }
    }

    #[test]
    fn counters_and_short_records_are_filled_in() {
        let v = values(&state());
        assert_eq!(v[0], vec!["Ana", "007", "https://a.example"]);
        assert_eq!(v[1], vec!["Bob", "010", ""]);
        let mut st = state();
        st.kinds[1] = FieldKind::Number {
            start: -2,
            step: 1,
            digits: 2,
        };
        assert_eq!(values(&st)[0][1], "-02");
    }

    #[test]
    fn fields_and_records_are_added_and_removed() {
        let mut st = state();
        add_field(&mut st, "City");
        assert_eq!(st.headers.len(), 4);
        assert!(st.rows.iter().all(|r| r.len() == 4));
        add_record(&mut st);
        assert_eq!(st.rows.len(), 3);
        remove_field(&mut st, 1);
        assert_eq!(st.headers, vec!["Name", "Link", "City"]);
        assert_eq!(st.kinds[1], FieldKind::Qr);
        assert!(st.rows.iter().all(|r| r.len() == 3));
    }

    #[test]
    fn csv_round_trips_with_counters_written_out() {
        let text = to_csv(&state());
        let (h, r) = crate::export::parse_csv(&text);
        assert_eq!(h, vec!["Name", "No", "Link"]);
        assert_eq!(r[1], vec!["Bob", "010", ""]);
    }

    #[test]
    fn merge_fills_text_qr_and_counters() {
        let mut app = App::headless();
        let mut add = |text: &str, frame: bool| {
            app.new_shape(ShapeKind::Text {
                spans: vec![TextSpan::new(text, "Arial", 12.0)],
                origin: tracedraw_core::geometry::Point::new(10.0, 10.0),
                frame: frame.then(|| tracedraw_core::geometry::Size::new(30.0, 30.0)),
                align: Default::default(),
                para: Default::default(),
                on_path: None,
            })
            .expect("added");
        };
        add("Hello <Name> no <No>", false);
        add("<Link>", true);
        let pages_before = app.doc().pages.len();
        perform(&mut app, &state());
        let doc = app.doc();
        assert_eq!(doc.pages.len(), pages_before + 2);
        let first = &doc.pages[1];
        let shapes: Vec<&Shape> = first.layers.iter().flat_map(|l| &l.shapes).collect();
        let texts: Vec<String> = shapes.iter().filter_map(|s| text_of(s)).collect();
        assert!(texts.contains(&"Hello Ana no 007".to_string()), "{texts:?}");
        // The QR field became a path inside the 30 mm frame.
        let qr = shapes
            .iter()
            .find(|s| matches!(s.kind, ShapeKind::Path { .. }))
            .expect("qr code");
        let b = qr.bounds();
        assert!(b.width() > 25.0 && b.width() <= 30.0 + 1e-6, "{b:?}");
        // The second record has no link: its field object is gone.
        let second = &doc.pages[2];
        let n: usize = second.layers.iter().map(|l| l.shapes.len()).sum();
        assert_eq!(n, 1);
    }
}
